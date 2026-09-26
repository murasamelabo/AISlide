import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile, readdir, symlink, truncate } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory.js';
import { guidedExamples } from './guided-demo.mjs';
import { registerHooks } from 'node:module';
import { randomUUID } from 'node:crypto';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { z } from 'zod';

const coreEnvironment = process.env.AISLIDE_CORE_BINARY ? { AISLIDE_CORE_BINARY: process.env.AISLIDE_CORE_BINARY } : undefined;

function resolveSchemaRef(root, value) {
  const visited = new Set();
  while (typeof value?.$ref === 'string') {
    const ref = value.$ref;
    assert.ok(ref.startsWith('#/') && !visited.has(ref), `Invalid schema reference: ${ref}`);
    visited.add(ref);
    value = ref.slice(2).split('/').reduce((current, key) => current?.[key.replaceAll('~1', '/').replaceAll('~0', '~')], root);
    assert.ok(value, `Unresolved schema reference: ${ref}`);
  }
  return value;
}

async function feedbackMcpFixture(run, args = ['--tool-profile', 'full']) {
  const registrations = new Map();
  const resources = new Map();
  const prompts = new Map();
  const calls = [];
  const key = `aislide-feedback-${randomUUID()}`;
  const entry = new URL(`./mcp.mjs?${key}`, import.meta.url).href;
  const fixture = {
    McpServer: class extends McpServer {
      constructor(...args) { super(...args); fixture.instance = this; }
      registerTool(name, config, callback) { assert.equal(registrations.has(name), false, `Duplicate tool: ${name}`); registrations.set(name, { config, callback }); return super.registerTool(name, config, callback); }
      registerResource(name, uri, config, callback) { resources.set(uri, { config, callback }); return super.registerResource(name, uri, config, callback); }
      registerPrompt(name, config, callback) { prompts.set(name, { config, callback }); return super.registerPrompt(name, config, callback); }
      async connect() {}
    },
    StdioServerTransport: class {},
    MAX_REQUEST_BYTES: 100663296,
    requestCore: async (request, options) => {
      calls.push({ request: structuredClone(request), signal: options?.signal });
      if (fixture.onRequest) return fixture.onRequest(request, options);
      if (request.op === 'create_presentation') return { version: 1, id: request.id, revision: 0, hash: 'a'.repeat(64), sources: [], bindings: [], parts: [], deck: { version: 1, title: request.title, width: 1280, height: 720, slides: [{ id: 'slide-1', title: request.title, background: 'FFFFFF', elements: [], notes: '' }] } };
      if (request.op === 'apply_operations' || request.op === 'import_slides') {
        assert.equal(request.expected_hash, request.document.hash);
        return { document: { ...request.document, revision: request.document.revision + 1, hash: 'b'.repeat(64) }, receipt: { inverse: [] } };
      }
      if (request.op === 'create_object') return { type: 'text', id: request.id, x: 0, y: 0, width: 200, height: 80, text: 'Synthetic', font_size: 24, color: '@dk1', bold: false };
      return { ready: true, op: request.op };
    },
  };
  globalThis[key] = fixture;
  const replacements = new Map([
    ['@modelcontextprotocol/sdk/server/mcp.js', ['McpServer']],
    ['@modelcontextprotocol/sdk/server/stdio.js', ['StdioServerTransport']],
    ['./core-client.mjs', ['requestCore', 'MAX_REQUEST_BYTES']],
  ]);
  const hooks = registerHooks({ resolve(specifier, context, nextResolve) {
    const names = context.parentURL === entry ? replacements.get(specifier) : undefined;
    if (!names) return nextResolve(specifier, context);
    const source = names.map(name => `export const ${name} = globalThis[${JSON.stringify(key)}].${name};`).join('\n');
    return { url: `data:text/javascript,${encodeURIComponent(source)}`, shortCircuit: true };
  } });
  const previousArgv = process.argv;
  try {
    process.argv = [...previousArgv.slice(0, 2), ...args];
    try { await import(entry); } finally { process.argv = previousArgv; }
    const call = async (name, input = {}, signal = new AbortController().signal) => {
      const tool = registrations.get(name);
      assert.ok(tool, `Missing tool: ${name}`);
      const parsed = tool.config.inputSchema.parse(input);
      const result = await tool.callback(parsed, { signal });
      assert.ok(!result.isError, JSON.stringify(result.content));
      return JSON.parse(result.content[0].text);
    };
    await run({ registrations, resources, prompts, calls, call, fixture });
  } finally { hooks.deregister(); delete globalThis[key]; }
}

test('lightweight MCP publishes deduplicated local schema references without weakening validation', async context => {
  await feedbackMcpFixture(async ({ fixture, registrations, calls }) => {
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const client = new Client({ name: 'schema-size-regression', version: '1.0.0' });
    try {
      await McpServer.prototype.connect.call(fixture.instance, serverTransport);
      await client.connect(clientTransport);
      const listed = await client.listTools();
      const schema = listed.tools.find(tool => tool.name === 'add_elements').inputSchema;
      const original = z.toJSONSchema(registrations.get('add_elements').config.inputSchema, { target: 'draft-7', io: 'input' });
      const originalBytes = Buffer.byteLength(JSON.stringify(original));
      const publishedBytes = Buffer.byteLength(JSON.stringify(schema));
      const expandedList = { tools: [...registrations].map(([name, { config }]) => ({ name, description: config.description, inputSchema: z.toJSONSchema(config.inputSchema, { target: 'draft-7', io: 'input' }), annotations: config.annotations })) };
      context.diagnostic(JSON.stringify({ tool: 'add_elements', expanded_bytes: originalBytes, compact_bytes: publishedBytes, expanded_full_list_bytes: Buffer.byteLength(JSON.stringify(expandedList)), full_list_bytes: Buffer.byteLength(JSON.stringify(listed)), tool_count: listed.tools.length }));
      assert.ok(publishedBytes < originalBytes * 0.6, `Published schema ${publishedBytes} bytes versus expanded ${originalBytes}`);
      const refs = [];
      const visit = value => {
        if (!value || typeof value !== 'object') return;
        if (typeof value.$ref === 'string') refs.push(value.$ref);
        for (const item of Object.values(value)) visit(item);
      };
      visit(schema);
      assert.ok(refs.length > 0);
      for (const ref of refs) {
        assert.ok(ref.startsWith('#/'), ref);
        let value = schema;
        for (const segment of ref.slice(2).split('/')) value = value?.[segment.replaceAll('~1', '/').replaceAll('~0', '~')];
        assert.ok(value, `Unresolved local schema reference: ${ref}`);
      }
      const before = calls.length;
      const invalid = await client.callTool({ name: 'add_elements', arguments: { deck_id: randomUUID(), expected_revision: 0, expected_hash: 'a'.repeat(64), slide_id: 'slide-1', elements: [{ type: 'text', id: 'invalid', x: 0, y: 0, width: 20, height: 20, text: 'Synthetic', font_size: 20, color: 'not-a-color', bold: false }] } });
      assert.equal(invalid.isError, true);
      assert.equal(calls.length, before);
    } finally { await client.close(); }
  });
});

test('lightweight MCP defaults to a bounded tool surface with on-demand schemas', async context => {
  await feedbackMcpFixture(async ({ fixture, calls }) => {
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const client = new Client({ name: 'compact-tools-regression', version: '1.0.0' });
    try {
      await McpServer.prototype.connect.call(fixture.instance, serverTransport);
      await client.connect(clientTransport);
      const initial = await client.listTools();
      assert.ok(initial.tools.length <= 10, `Unexpected initial tool count: ${initial.tools.length}`);
      assert.ok(Buffer.byteLength(JSON.stringify(initial)) < 65536);
      for (const name of ['discover_tools', 'get_tool_schema', 'create_presentation', 'edit_slides', 'apply_operations', 'preview_presentation', 'finalize_presentation']) assert.ok(initial.tools.some(tool => tool.name === name), name);
      assert.equal(initial.tools.some(tool => tool.name === 'get_document'), false);
      const search = await client.callTool({ name: 'discover_tools', arguments: { query: 'graph', limit: 4 } });
      const catalog = JSON.parse(search.content[0].text);
      assert.equal(search.isError, undefined);
      assert.equal(catalog.tools.length, 4);
      assert.ok(catalog.total > 4);
      assert.equal(catalog.tools.some(tool => Object.hasOwn(tool, 'inputSchema')), false);
      assert.ok(Buffer.byteLength(JSON.stringify(search)) < 4096);
      const detail = await client.callTool({ name: 'get_tool_schema', arguments: { name: 'add_graph' } });
      const definition = JSON.parse(detail.content[0].text);
      assert.equal(definition.name, 'add_graph');
      assert.equal(definition.inputSchema.type, 'object');
      assert.ok((await client.listTools()).tools.some(tool => tool.name === 'add_graph'));
      for (const name of ['get_document', 'undo', 'redo', 'get_graph', 'apply_theme']) {
        const result = await client.callTool({ name: 'get_tool_schema', arguments: { name } });
        assert.equal(result.isError, undefined);
      }
      assert.ok((await client.listTools()).tools.length <= initial.tools.length + 4);
      const invalid = await client.callTool({ name: 'get_tool_schema', arguments: { name: 'does_not_exist' } });
      assert.equal(invalid.isError, true);
      assert.equal(calls.length, 0);
      context.diagnostic(JSON.stringify({ default_tool_count: initial.tools.length, default_list_bytes: Buffer.byteLength(JSON.stringify(initial)) }));
    } finally { await client.close(); }
  }, []);
});

test('lightweight MCP recovers current handles and operation state without document or preview reads', async () => {
  await feedbackMcpFixture(async ({ fixture, registrations, call, calls }) => {
    assert.deepEqual((await call('list_decks')).decks, []);
    const create = registrations.get('create_presentation');
    const response = await create.callback(create.config.inputSchema.parse({ title: 'Synthetic resumable deck' }), { signal: new AbortController().signal });
    const created = JSON.parse(response.content[0].text);
    assert.equal(response.structuredContent, undefined);
    assert.equal(created.hash, 'a'.repeat(64));
    assert.equal(created.can_undo, false);
    const { deck_id } = created;
    const before = calls.length;
    const inventory = await call('list_decks');
    assert.equal(inventory.decks[0].deck_id, deck_id);
    assert.equal(inventory.decks[0].last_operation.name, 'create_presentation');
    assert.equal(inventory.persisted, false);
    const summary = await call('get_deck_summary', { deck_id });
    assert.equal(summary.slides[0].id, 'slide-1');
    assert.equal(summary.hash, created.hash);
    assert.equal(calls.length, before);
    const changed = await call('apply_operations', { deck_id, expected_revision: 0, expected_hash: created.hash, operations: [{ op: 'set_slide_background', slide_id: 'slide-1', color: 'FFFFFF' }] });
    assert.equal(changed.revision, 1);
    const resumed = await call('list_decks');
    assert.equal(resumed.decks[0].revision, changed.revision);
    assert.equal(resumed.decks[0].hash, changed.hash);
    assert.equal(resumed.decks[0].last_operation.name, 'apply_operations');
    assert.equal(resumed.decks[0].can_undo, true);
    const mutate = registrations.get('apply_operations');
    const stale = await mutate.callback(mutate.config.inputSchema.parse({ deck_id, expected_revision: 0, expected_hash: created.hash, operations: [{ op: 'set_slide_background', slide_id: 'slide-1', color: '000000' }] }), { signal: new AbortController().signal });
    assert.equal(stale.isError, true);
    assert.equal(calls.length, before + 1);
    assert.deepEqual((await call('list_decks')).decks[0], resumed.decks[0]);
    await call('close_deck', { deck_id });
    assert.deepEqual((await call('list_decks')).decks, []);
    assert.ok(Buffer.byteLength(JSON.stringify(resumed)) < 2048);
    assert.equal(fixture.instance.isConnected(), false);
  }, []);
});

test('lightweight MCP reuses bounded approved local assets without round-tripping base64', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-assets-'));
  const outside = await mkdtemp(join(tmpdir(), 'aislide-assets-outside-'));
  const image = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACklEQVR4nGMAAQAABQABDQottAAAAABJRU5ErkJggg==', 'base64');
  try {
    await writeFile(join(directory, 'image.png'), image);
    await writeFile(join(outside, 'outside.png'), image);
    await symlink(outside, join(directory, 'outside-link'), process.platform === 'win32' ? 'junction' : 'dir');
    await writeFile(join(directory, 'large.png'), '');
    await truncate(join(directory, 'large.png'), 1048577);
    await feedbackMcpFixture(async ({ call, calls, registrations }) => {
      const asset = await call('register_asset', { path: 'image.png' });
      assert.equal(asset.byte_length, image.length);
      assert.equal(asset.mime_type, 'image/png');
      assert.equal(asset.base64, undefined);
      assert.equal(calls.length, 0);
      assert.equal((await call('register_asset', { path: 'image.png' })).asset_id, asset.asset_id);
      assert.equal((await call('list_assets')).assets.length, 1);
      const created = await call('create_presentation', { title: 'Synthetic asset reuse' });
      await writeFile(join(directory, 'image.png'), Buffer.from('changed after registration'));
      const added = await call('apply_operations', { deck_id: created.deck_id, expected_revision: 0, expected_hash: created.hash, operations: [{ op: 'add_picture', slide_id: 'slide-1', id: 'picture', asset_id: asset.asset_id, alt: 'Synthetic image' }] });
      assert.equal(calls.length, 2);
      assert.equal(calls.at(-1).request.operations[0].base64, image.toString('base64'));
      assert.equal(calls.at(-1).request.operations[0].mime_type, 'image/png');
      assert.equal(calls.at(-1).request.operations[0].asset_id, undefined);
      const spec = { version: 1, title: '', nodes: [{ id: 'node', label: 'Synthetic', x: 0, y: 88, icon: { asset_id: asset.asset_id } }] };
      await call('create_graph', { id: 'graph', spec });
      assert.deepEqual(calls.at(-1).request.spec.nodes[0].icon, { base64: image.toString('base64'), mime_type: 'image/png' });
      const assetTool = registrations.get('register_asset');
      for (const input of [{ path: '../outside.png' }, { path: '..\\outside.png' }, { path: join(outside, 'outside.png') }, { path: 'https://example.com/image.png' }, { path: 'image.png:alternate' }, { path: 'outside-link/outside.png' }, { path: 'large.png' }, { path: 'image.png', root: 1 }]) {
        const result = await assetTool.callback(assetTool.config.inputSchema.parse(input), { signal: new AbortController().signal });
        assert.equal(result.isError, true, JSON.stringify(input));
      }
      const picture = registrations.get('add_picture');
      const invalidBoth = picture.config.inputSchema.safeParse({ deck_id: created.deck_id, expected_revision: added.revision, slide_id: 'slide-1', id: 'invalid', alt: '', base64: image.toString('base64'), mime_type: 'image/png', asset_id: asset.asset_id });
      assert.equal(invalidBoth.success, false);
      await call('close_asset', { asset_id: asset.asset_id });
      const before = calls.length;
      const missing = await picture.callback(picture.config.inputSchema.parse({ deck_id: created.deck_id, expected_revision: added.revision, slide_id: 'slide-1', id: 'missing', alt: '', asset_id: asset.asset_id }), { signal: new AbortController().signal });
      assert.equal(missing.isError, true);
      assert.equal(calls.length, before);
      assert.deepEqual((await call('list_assets')).assets, []);
      await call('close_deck', { deck_id: created.deck_id });
    }, ['--asset-dir', directory]);
    await feedbackMcpFixture(async ({ registrations }) => {
      const tool = registrations.get('register_asset');
      const result = await tool.callback(tool.config.inputSchema.parse({ path: 'image.png' }), { signal: new AbortController().signal });
      assert.equal(result.isError, true);
      assert.match(result.content[0].text, /asset-dir/);
    }, []);
  } finally {
    await rm(directory, { recursive: true, force: true });
    await rm(outside, { recursive: true, force: true });
  }
});

test('lightweight asset cache retains batches atomically at its capacity boundary', async () => {
  const { McpAssets } = await import('./mcp-assets.mjs');
  const assets = await McpAssets.create([]);
  const inputs = Array.from({ length: 31 }, (_, index) => ({ bytes: Buffer.from(`Synthetic asset ${index}`), format: 'png', name: `asset-${index}` }));
  const saved = assets.retainMany(inputs);
  const before = assets.list();
  assert.throws(() => assets.retainMany([{ bytes: Buffer.from('New one'), format: 'png', name: 'new-one' }, { bytes: Buffer.from('New two'), format: 'png', name: 'new-two' }]), /limit/);
  assert.deepEqual(assets.list(), before);
  const reused = assets.retainMany([inputs[0], { bytes: Buffer.from('New one'), format: 'png', name: 'new-one' }]);
  assert.equal(reused[0].asset_id, saved[0].asset_id);
  assert.equal(assets.list().assets.length, 32);
  assets.close(saved[0].asset_id);
  assert.equal(assets.list().assets.length, 31);
  assert.throws(() => assets.retain(Buffer.alloc(1048577), 'png', 'oversize'), /oversized/);
  assert.equal(assets.list().assets.length, 31);
});

test('lightweight MCP bounds default previews and catalogs while allowing explicit full detail', async () => {
  await feedbackMcpFixture(async ({ call, calls, fixture, registrations }) => {
    const image = { base64: 'aW1hZ2U=', mime_type: 'image/png', width: 10, height: 10, alt: 'Synthetic' };
    const presets = Array.from({ length: 30 }, (_, index) => ({ id: `list/${index}`, category: 'list', name: `Synthetic ${index}`, example: { version: 1, preset: `list/${index}` }, recommended: index === 0, use_when: 'Equal-status topics' }));
    fixture.onRequest = async request => {
      if (request.op === 'create_presentation') return { version: 1, id: request.id, revision: 0, hash: 'a'.repeat(64), sources: [], bindings: [], parts: [], deck: { version: 1, title: request.title, width: 1280, height: 720, slides: Array.from({ length: 14 }, (_, index) => ({ id: `slide-${index + 1}`, title: `Synthetic ${index}`, background: 'FFFFFF', elements: [], notes: '' })) } };
      if (request.op === 'preview_presentation') return { revision: 0, hash: request.document.hash, pages: (request.options.page_indices ?? [0]).map(page_index => ({ page_index, slide_id: `slide-${page_index + 1}`, image_index: 0 })), images: [{ ...image, mime_type: `image/${request.options.format ?? 'png'}` }], warnings: [], office_visual_parity: false };
      if (request.op === 'part_catalog') return { version: 1, presets, schema: { marker: 'FULL_SCHEMA' }, style: 'Synthetic', default_bounds: {} };
      if (request.op === 'architecture_icons') return { version: 1, release: 'synthetic', configured: false, message: 'Not installed', providers: [], icons: Array.from({ length: 100 }, (_, index) => ({ id: `vendor/${index}`, name: `Synthetic ${index}`, provider: 'aws', categories: ['compute'], aliases: [] })) };
      if (request.op === 'create_graph_icon') return { base64: image.base64, mime_type: image.mime_type, alt: image.alt };
      if (request.op === 'architecture_icon_assets') return { icons: [{ id: 'vendor/1', ...image }] };
      throw new Error(`Unexpected request: ${request.op}`);
    };
    const { deck_id } = await call('create_presentation', { title: 'Synthetic compact preview' });
    const tool = registrations.get('preview_presentation');
    const response = await tool.callback(tool.config.inputSchema.parse({ deck_id }), { signal: new AbortController().signal });
    assert.equal(response.isError, undefined);
    assert.equal(response.structuredContent, undefined);
    assert.equal(response.content[1].mimeType, 'image/jpeg');
    assert.deepEqual(calls.at(-1).request.options, { max_dimension: 640, layout: 'contact_sheet', format: 'jpeg', max_output_bytes: 393216, page_indices: [0, 1, 2, 3, 4, 5, 6, 7] });
    const preview = JSON.parse(response.content[0].text);
    assert.deepEqual(preview.page_scope, { total: 14, selected: 8, unselected: 6 });
    assert.ok(Buffer.byteLength(response.content[0].text) < 4096);
    await call('preview_presentation', { deck_id, detail: 'full', options: { page_indices: [0], max_dimension: 1600 } });
    assert.deepEqual(calls.at(-1).request.options, { page_indices: [0], max_dimension: 1600 });
    const catalog = await call('part_catalog');
    assert.equal(catalog.presets.length, 12);
    assert.equal(catalog.total, 30);
    assert.equal(catalog.next_offset, 12);
    assert.equal(catalog.schema, undefined);
    assert.equal(catalog.presets[0].example, undefined);
    assert.equal(catalog.presets[0].recommended, true);
    const selected = await call('part_catalog', { preset_id: 'list/0' });
    assert.deepEqual(selected.presets[0].example, presets[0].example);
    assert.equal((await call('part_catalog', { detail: 'full' })).presets.length, 30);
    const icons = await call('architecture_icons', { provider: 'aws', limit: 5, offset: 95 });
    assert.equal(icons.icons.length, 5);
    assert.equal(icons.next_offset, null);
    const prepared = await call('create_graph_icon', { base64: image.base64, mime_type: image.mime_type });
    assert.equal(prepared.base64, undefined);
    assert.equal(typeof prepared.asset_id, 'string');
    assert.deepEqual(await call('create_graph_icon', { base64: image.base64, mime_type: image.mime_type, detail: 'full' }), { base64: image.base64, mime_type: image.mime_type, alt: image.alt });
    const iconAssets = await call('architecture_icon_assets', { ids: ['vendor/1'] });
    assert.equal(iconAssets.icons[0].asset_id, prepared.asset_id);
    assert.equal(iconAssets.icons[0].base64, undefined);
    await call('close_deck', { deck_id });
  }, []);
});

test('lightweight MCP live 14-slide authoring resumes and exports a complete native presentation', { timeout: 180000 }, async context => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-compact-live-'));
  const sharp = (await import('sharp')).default;
  const image = await sharp({ create: { width: 32, height: 32, channels: 4, background: { r: 25, g: 190, b: 115, alpha: 1 } } }).png().toBuffer();
  await writeFile(join(directory, 'synthetic.png'), image);
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory, '--asset-dir', directory], stderr: 'pipe', env: coreEnvironment });
  const client = new Client({ name: 'compact-authoring-proof', version: '1.0.0' });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args }, undefined, { timeout: 120000 });
    assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content)}`);
    assert.equal(result.structuredContent, undefined);
    return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const listed = await client.listTools();
    assert.equal(listed.tools.length, 10);
    assert.ok(Buffer.byteLength(JSON.stringify(listed)) < 65536);
    const created = await call('create_presentation', { title: 'Synthetic compact fourteen-slide proof' });
    const blank = await call('edit_slides', { deck_id: created.deck_id, expected_revision: created.revision, operations: Array.from({ length: 13 }, (_, index) => ({ op: 'insert', id: `slide-${index + 2}`, after: `slide-${index + 1}`, title: `Synthetic ${index + 2}` })) });
    const asset = await call('register_asset', { path: 'synthetic.png' });
    const operations = Array.from({ length: 14 }, (_, index) => ({ op: 'add_elements', slide_id: `slide-${index + 1}`, elements: [{ type: 'text', id: `heading-${index + 1}`, x: 80, y: 100, width: 1050, height: 90, text: `Synthetic slide ${index + 1}`, font_size: 32, color: '@dk1', bold: true }] }));
    operations.push({ op: 'add_picture', slide_id: 'slide-1', id: 'synthetic-picture', asset_id: asset.asset_id, alt: 'Synthetic square', frame: { x: 900, y: 400, width: 64, height: 64 } });
    const authored = await call('apply_operations', { deck_id: blank.deck_id, expected_revision: blank.revision, expected_hash: blank.hash, operations });
    const inventory = await call('list_decks');
    const resumed = inventory.decks.find(deck => deck.title === 'Synthetic compact fourteen-slide proof');
    assert.equal(resumed.deck_id, authored.deck_id);
    assert.equal(resumed.hash, authored.hash);
    assert.equal(resumed.last_operation.name, 'apply_operations');
    const summary = await call('get_deck_summary', { deck_id: resumed.deck_id });
    assert.equal(summary.slides.length, 14);
    assert.ok(summary.slides.every(slide => slide.element_count >= 1));
    assert.ok(Buffer.byteLength(JSON.stringify(summary)) < 8192);
    const preview = await client.callTool({ name: 'preview_presentation', arguments: { deck_id: resumed.deck_id } }, undefined, { timeout: 120000 });
    assert.ok(!preview.isError, JSON.stringify(preview.content));
    const previewMetadata = JSON.parse(preview.content[0].text);
    assert.deepEqual(previewMetadata.page_scope, { total: 14, selected: 8, unselected: 6 });
    const previewImage = preview.content.find(item => item.type === 'image');
    assert.equal(previewImage.mimeType, 'image/jpeg');
    const previewBytes = Buffer.from(previewImage.data, 'base64');
    assert.equal((await sharp(previewBytes).metadata()).format, 'jpeg');
    assert.ok(previewBytes.length <= 393216);
    const delivery = await call('finalize_presentation', { deck_id: resumed.deck_id, expected_revision: resumed.revision, expected_hash: resumed.hash, name: 'synthetic-compact', include_images: false });
    const presentation = delivery.files.find(file => file.filename.endsWith('.pptx'));
    assert.ok(presentation);
    const bytes = await readFile(presentation.path);
    assert.equal(bytes.subarray(0, 2).toString(), 'PK');
    assert.equal((await call('list_decks')).decks[0].last_export.path, presentation.path);
    const registered = await call('register_asset', { path: presentation.filename });
    const reopened = await call('open_pptx', { asset_id: registered.asset_id });
    assert.equal(reopened.slides, 14);
    const reopenedSummary = await call('get_deck_summary', { deck_id: reopened.deck_id });
    assert.ok(reopenedSummary.slides.every(slide => slide.element_count >= 1));
    assert.deepEqual(await readFile(presentation.path), bytes);
    context.diagnostic(JSON.stringify({ slides: 14, default_list_bytes: Buffer.byteLength(JSON.stringify(listed)), summary_bytes: Buffer.byteLength(JSON.stringify(summary)), preview_image_bytes: previewBytes.length, delivery_response_bytes: Buffer.byteLength(JSON.stringify(delivery)), pptx_bytes: bytes.length, original_unchanged: true }));
    await call('close_deck', { deck_id: reopened.deck_id });
    await call('close_deck', { deck_id: resumed.deck_id });
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});

test('graph feedback MCP schemas and committed diagnostics stay bounded and single-request', async () => {
  await feedbackMcpFixture(async ({ call, calls, registrations, fixture }) => {
    const { deck_id } = await call('create_presentation', { title: 'Graph feedback' });
    const node = { id: 'node', label: 'Synthetic label', x: 0, y: 88, label_fit: 'shrink' };
    const group = { id: 'group', label: 'Boundary', x: 0, y: 88, width: 500, height: 300, header_color: '@accent2' };
    const spec = { version: 1, title: '', nodes: [node], groups: [group] };
    const part = { version: 1, preset: 'diagram/custom', title: '', data: { kind: 'diagram', graph: spec } };
    const element = { type: 'group', id: 'graph', children: [] };
    const diagnostics = { status: 'complete', findings: [{ code: 'GRAPH_LABEL_BORDER_OVERLAP', severity: 'warning', graph_id: 'graph', entity_id: 'edge', element_ids: ['label'], bounds: [10, 20, 80, 28], message: 'Border overlap.', slide_id: 'slide-1' }] };
    let supplied = diagnostics;
    let noOp = false;
    fixture.onRequest = async request => {
      if (request.op === 'create_graph') return request.include_diagnostics ? { element, diagnostics } : element;
      assert.ok(['insert_graph', 'update_graph', 'apply_graph', 'insert_part', 'update_part', 'apply_operations'].includes(request.op), `Unexpected post-commit request: ${request.op}`);
      return { document: noOp ? request.document : { ...request.document, revision: request.document.revision + 1, hash: 'b'.repeat(64) }, receipt: noOp ? null : { inverse: [] }, ...(supplied === undefined ? {} : { diagnostics: supplied }) };
    };
    assert.deepEqual(await call('create_graph', { id: 'graph', spec }), element);
    assert.equal(Object.hasOwn(calls.at(-1).request, 'include_diagnostics'), false);
    assert.deepEqual(await call('create_graph', { id: 'graph', spec, include_diagnostics: false }), element);
    assert.deepEqual(await call('create_graph', { id: 'graph', spec, include_diagnostics: true }), { element, diagnostics });
    const schema = registrations.get('create_graph').config.inputSchema;
    for (const value of [null, 'true', 1]) assert.equal(schema.safeParse({ id: 'graph', spec, include_diagnostics: value }).success, false);
    for (const label_fit of ['wrap', 'shrink']) assert.ok(schema.safeParse({ id: 'graph', spec: { ...spec, nodes: [{ ...node, label_fit }] } }).success);
    for (const label_fit of [null, 'truncate', true]) assert.equal(schema.safeParse({ id: 'graph', spec: { ...spec, nodes: [{ ...node, label_fit }] } }).success, false);
    for (const header_color of ['123ABC', '@dk1', null]) assert.ok(schema.safeParse({ id: 'graph', spec: { ...spec, groups: [{ ...group, header_color }] } }).success);
    for (const header_color of ['red', '@bad', 123]) assert.equal(schema.safeParse({ id: 'graph', spec: { ...spec, groups: [{ ...group, header_color }] } }).success, false);
    assert.ok(registrations.get('transform_graph').config.inputSchema.safeParse({ spec, operations: [{ op: 'put_node', node }, { op: 'put_group', group }] }).success);
    assert.ok(registrations.get('create_part').config.inputSchema.safeParse({ id: 'part', spec: part }).success);
    assert.ok(registrations.get('preview_slide_revision').config.inputSchema.safeParse({ deck_id, expected_revision: 0, expected_hash: 'a'.repeat(64), slide_id: 'slide-1', edits: [{ op: 'update_graph', id: 'graph', spec }, { op: 'update_part', id: 'part', spec: part }] }).success);
    let revision = 0;
    for (const name of ['add_graph', 'update_graph', 'apply_graph', 'add_part', 'update_part', 'apply_operations']) {
      const input = name === 'apply_operations' ? { expected_hash: 'b'.repeat(64), operations: [
        { op: 'add_graph', slide_id: 'slide-1', id: 'graph', spec }, { op: 'update_graph', slide_id: 'slide-1', id: 'graph', spec },
        { op: 'add_part', slide_id: 'slide-1', id: 'part', spec: part }, { op: 'update_part', slide_id: 'slide-1', id: 'part', spec: part },
      ] } : { slide_id: 'slide-1', id: 'graph', ...(name === 'apply_graph' ? { operations: [{ op: 'put_node', node }, { op: 'put_group', group }] } : { spec: name.endsWith('part') ? part : spec }) };
      const count = calls.length;
      const result = await call(name, { deck_id, expected_revision: revision, ...input });
      revision += 1;
      assert.equal(calls.length, count + 1, name);
      assert.deepEqual(result.graphDiagnostics, { revision, hash: 'b'.repeat(64), ...diagnostics }, name);
      assert.equal(result.revision, revision);
      assert.ok(Buffer.byteLength(JSON.stringify(result)) < 40000);
    }
    noOp = true;
    supplied = { status: 'partial', findings: [] };
    const count = calls.length;
    const unchanged = await call('apply_graph', { deck_id, expected_revision: revision, slide_id: 'slide-1', id: 'graph', operations: [{ op: 'move', ids: ['node'], dx: 0, dy: 0 }] });
    assert.equal(calls.length, count + 1);
    assert.equal(unchanged.revision, revision);
    assert.deepEqual(unchanged.graphDiagnostics, { revision, hash: 'b'.repeat(64), ...supplied });
    noOp = false;
    supplied = { status: 'complete', findings: Array(65).fill(diagnostics.findings[0]) };
    const malformed = await call('update_graph', { deck_id, expected_revision: revision++, slide_id: 'slide-1', id: 'graph', spec });
    assert.deepEqual(malformed.graphDiagnostics, { revision, hash: 'b'.repeat(64), status: 'unavailable', findings: [] });
    supplied = undefined;
    const legacy = await call('update_part', { deck_id, expected_revision: revision++, slide_id: 'slide-1', id: 'part', spec: part });
    assert.equal(Object.hasOwn(legacy, 'graphDiagnostics'), false);
    const recovery = await call('get_session_recovery', { deck_id });
    assert.equal(JSON.stringify(recovery).includes('diagnostics'), false);
    assert.equal(recovery.document.revision, revision);
    assert.match(registrations.get('create_graph').config.description, /include_diagnostics.*64.*32 KiB/);
    assert.match(registrations.get('add_graph').config.description, /graphDiagnostics/);
  });
});

test('graph authoring MCP preserves manual routes and waypoints', async () => {
  await feedbackMcpFixture(async ({ call, calls, registrations }) => {
    const spec = { version: 1, title: 'Synthetic route', show_title: false, nodes: [
      { id: 'source', label: 'Source', x: 40, y: 120 },
      { id: 'target', label: 'Target', x: 700, y: 120 },
    ], edges: [{ id: 'edge', source: 'source', target: 'target', route: 'manual', waypoints: [[400, 300]], stroke_width: 4, label: 'Request', label_color: '@accent2', label_font_size: 20, source_offset: -0.25, target_offset: 0.25, label_placement: { position: 0.3, side: 'below', offset: 12 }, badge: { number: 7, position: 0.7, size: 28, font_size: 14, fill: '@lt1', color: '@dk1' } }], groups: [{ id: 'group', label: 'Group', x: 0, y: 0, width: 1152, height: 512, padding: 16, header_height: 64, header_font_size: 24 }] };
    await call('create_graph', { id: 'graph', spec });
    assert.deepEqual(calls.at(-1).request.spec, spec);
    const edge = spec.edges[0], group = spec.groups[0];
    await call('transform_graph', { spec, operations: [{ op: 'put_edge', edge }, { op: 'put_group', group }] });
    assert.deepEqual(calls.at(-1).request.operations, [{ op: 'put_edge', edge }, { op: 'put_group', group }]);
    const schema = registrations.get('create_graph').config.inputSchema;
    const json = z.toJSONSchema(schema);
    assert.deepEqual(json.properties.spec.properties.edges.items.properties.route.enum, ['straight', 'elbow', 'manual']);
    assert.equal(json.properties.spec.properties.edges.items.properties.waypoints.maxItems, 16);
    for (const on_overlap of ['warn', 'error']) {
      const strict = structuredClone(spec); strict.edges[0].label_placement.on_overlap = on_overlap;
      await call('create_graph', { id: 'collision-policy', spec: strict });
      assert.equal(calls.at(-1).request.spec.edges[0].label_placement.on_overlap, on_overlap);
    }
    assert.equal(schema.safeParse({ id: 'graph', spec: { ...spec, edges: [{ ...edge, label_placement: { position: 0.5, on_overlap: 'ignore' } }] } }).success, false);
    for (const patch of [{ stroke_width: 0.49 }, { stroke_width: 12.1 }, { label_font_size: 41 }, { label_color: 'red' }, { source_offset: -0.51 }, { target_offset: 0.51 }, { waypoints: null }, { waypoints: Array(17).fill([1, 100]) }, { waypoints: [[1153, 100]] }, { waypoints: [[100, -1]] }, { label_placement: { position: 1.1 } }, { label_placement: { position: 0.5, offset: 129 } }, { label_placement: { position: 0.5, side: 'left' } }, { badge: { number: 0 } }, { badge: { number: 100 } }, { badge: { number: 1.5 } }, { badge: { number: 1, size: 65 } }, { badge: { number: 1, font_size: 33 } }, { badge: { number: 1, unknown: true } }]) {
      assert.equal(schema.safeParse({ id: 'graph', spec: { ...spec, edges: [{ ...edge, ...patch }] } }).success, false, JSON.stringify(patch));
    }
    for (const patch of [{ padding: 65 }, { header_height: 19 }, { header_font_size: 33 }, { unknown: true }]) {
      assert.equal(schema.safeParse({ id: 'graph', spec: { ...spec, groups: [{ ...group, ...patch }] } }).success, false, JSON.stringify(patch));
    }
    assert.ok(schema.safeParse({ id: 'graph', spec: { ...spec, edges: [{ ...edge, stroke_width: null, label_color: null, label_font_size: null, source_offset: null, target_offset: null, label_placement: null, badge: null }], groups: [{ ...group, padding: null, header_height: null, header_font_size: null }] } }).success);
    const nativeSchema = registrations.get('add_elements').config.inputSchema;
    const input = { deck_id: randomUUID(), expected_revision: 0, slide_id: 'slide-1', elements: [{ type: 'connector', id: 'edge', x: 0, y: 0, width: 100, height: 100, color: '@dk1', stroke_width: 4, arrow: true, routing: { custom: true, points: Array.from({ length: 18 }, (_, index) => [index / 17, index % 2]) }, visual: { connection_sites: [{ x: 0.25, y: 1, angle: 90 }] } }] };
    assert.deepEqual(nativeSchema.parse(input), input);
    for (const patch of [{ custom: false }, { custom: undefined }, { custom: null }, { points: Array(19).fill([0, 0]) }]) {
      const invalid = structuredClone(input);
      Object.assign(invalid.elements[0].routing, patch);
      assert.equal(nativeSchema.safeParse(invalid).success, false);
    }
    for (const sites of [null, Array(129).fill({ x: 0, y: 0, angle: 0 }), [{ x: -0.1, y: 0, angle: 0 }], [{ x: 0, y: 0, angle: 361 }], [{ x: 0, y: 0, angle: 0, unknown: true }]]) {
      const invalid = structuredClone(input);
      invalid.elements[0].visual.connection_sites = sites;
      assert.equal(nativeSchema.safeParse(invalid).success, false);
    }
  });
});

test('MCP preview accepts bounded JPEG and overflow policy without losing metadata', async () => {
  await feedbackMcpFixture(async ({ registrations, call, calls, fixture }) => {
    const { deck_id } = await call('create_presentation', { title: 'Synthetic preview policy' });
    const tool = registrations.get('preview_presentation');
    for (const format of ['png', 'jpeg']) {
      fixture.onRequest = async request => ({ revision: 0, hash: request.document.hash, requested_max_dimension: 1280, actual_max_dimension: 960, quality_reduced: true, pages: [], warnings: [{ code: 'PREVIEW_DOWNSCALED', page_index: 0, element_id: '', message: 'Synthetic reduced preview' }], images: [{ base64: 'aW1hZ2U=', mime_type: `image/${format}`, width: 960, height: 540, byte_length: 5, sha256: 'a'.repeat(64) }], office_visual_parity: false });
      const input = { deck_id, options: { format, overflow: 'shrink', page_indices: [0] } };
      const result = await tool.callback(tool.config.inputSchema.parse(input), { signal: new AbortController().signal });
      assert.ok(!result.isError, JSON.stringify(result));
      assert.deepEqual(calls.at(-1).request.options, input.options);
      const metadata = JSON.parse(result.content[0].text);
      assert.equal(metadata.quality_reduced, true); assert.equal(metadata.actual_max_dimension, 960);
      assert.equal(result.content[1].mimeType, `image/${format}`);
      assert.ok(tool.config.inputSchema.safeParse({ ...input, options: { ...input.options, overflow: 'error' } }).success);
    }
    for (const options of [{ format: 'pdf' }, { overflow: 'unlimited' }, { format: null }, { overflow: null }, { max_output_bytes: 2097153 }]) assert.equal(tool.config.inputSchema.safeParse({ deck_id, options }).success, false);
  });
});

function assertFeedbackWorkflow(prompt, resource) {
  const text = prompt.messages[0].content.text;
  assert.equal(resource.contents[0].text, text);
  for (const pattern of [
    /Default compact mode exposes ten common tools/, /discover_tools.*get_tool_schema/, /list_decks and get_deck_summary/,
    /No manual progress file or full-document read/, /last successful compact mutation response or get_deck_summary/,
    /Pass asset_id instead of base64/, /page_scope reports unselected pages/,
    /typed_authoring/, /freeform.*high-volume/, /apply_operations.*complete add_elements/,
    /1\.\.128.*one Undo/, /expected_revision and expected_hash/, /set_frame\/set_frames change geometry only/,
    /set_text_style is a partial style update.*apply_format copies/, /Do not force a preview_slide_revision candidate for every frame/,
    /Finish with preview_presentation and preflight_presentation/, /sentence headlines and a 32-slide limit/,
    /headline_style="keyword"/, /slide_limit=39.*32\.\.128/, /Evidence support and numeric declarations still apply/,
    /Consulting issues are required only for the consulting-decision profile/, /PartSpec\.layout.*show_title=false.*body box/,
    /GraphSpec\.show_title=false.*node\.detail, text_align and heading_bold/, /12px text floor.*reject/,
    /import_slides.*source_deck_id.*reusing matching masters/, /same canvas/,
    /does not support arbitrary native cross-package/, /compile_report.*fixed structured-layout shortcut.*not the best path for exact recreation/,
    /prefer apply_operations with add_part\/add_graph and explicit layouts/, /create_part\/create_graph plus add_elements ONLY as an explicit unmanaged choice/,
    /never automatically fall back.*timeout/, /128 metadata entries.*capacity limits/, /Reduce chunk size for progress and cancellation/,
    /batch update_graph has no layout field and preserves the existing PartLayout/, /corner_label.*48 Unicode scalars.*default empty/,
    /Set theme before add_part\/add_graph/, /no automatic theme-driven regeneration/,
    /set_accessibility after the target exists in a separate revision/, /at least 65 seconds.*size-aware.*180 seconds.*opt-in MCP progress/,
    /All three venn variants support PartSpec\.layout\.show_title=false/,
    /Keep card text at least 8 slide pixels/, /CONTAINER_CORNER_OVERFLOW and CONTAINER_PADDING.*require preview review/,
    /CONNECTOR_BADGE_OVERLAP is info.*not a visual approval/, /propose guarded edits and inspect before\/after previews/,
    /Choose the information relationship before the layout/, /list\/rows.*list-horizontal\/columns.*list-enumeration\/grid/,
    /recommended.*use_when.*avoid_when/, /Do not assign a different accent color to every item/,
    /do not randomize layouts/, /Legacy preset IDs keep their existing rendering/,
    /node\.label_fit defaults to wrap/, /Group header_color.*@dk1/, /include_diagnostics=true.*bare native Element/,
    /graphDiagnostics bound to the accepted revision\/hash without another core request/, /64 findings within 32 KiB/,
    /GRAPH_LABEL_BORDER_OVERLAP.*GRAPH_LABEL_OVERLAP.*GRAPH_NODE_LABEL_WRAPPED.*GRAPH_NODE_LABEL_SHRINK_LIMIT/,
    /unavailable does not mean no issues/, /on_overlap=error remains fatal/,
  ]) assert.match(text, pattern);
}

test('managed batch MCP progress is opt-in monotonic and stops on completion or cancellation', async context => {
  context.mock.timers.enable({ apis: ['setInterval'] });
  try {
    await feedbackMcpFixture(async ({ registrations, call, fixture }) => {
      const created = await call('create_presentation', { title: 'Synthetic progress' });
      const before = await call('get_session_recovery', { deck_id: created.deck_id });
      const tool = registrations.get('apply_operations');
      const input = tool.config.inputSchema.parse({ deck_id: created.deck_id, expected_revision: 0, expected_hash: before.document.hash, operations: [
        { op: 'add_part', slide_id: 'slide-1', id: 'steps', spec: { version: 1, preset: 'list-horizontal/balanced', title: 'Synthetic', data: { kind: 'items', items: [{ label: 'First' }, { label: 'Next' }] } } },
      ] });
      for (const mode of ['success', 'cancel', 'failure', 'notification-failure', 'no-token']) {
        const controller = new AbortController();
        const started = Promise.withResolvers();
        const pending = Promise.withResolvers();
        const notifications = [];
        fixture.onRequest = async (request, options) => {
          started.resolve();
          options.signal.addEventListener('abort', () => pending.reject(new Error('Operation cancelled')), { once: true });
          await pending.promise;
          return { document: request.document, receipt: null, changes: [] };
        };
        const result = tool.callback(input, {
          signal: controller.signal,
          ...(mode === 'no-token' ? {} : { _meta: { progressToken: 0 } }),
          sendNotification: async notification => {
            notifications.push(structuredClone(notification));
            if (mode === 'notification-failure') throw new Error('Disconnected progress channel');
          },
        });
        await started.promise;
        await new Promise(resolve => setImmediate(resolve));
        context.mock.timers.tick(5000);
        await new Promise(resolve => setImmediate(resolve));
        if (mode === 'no-token') assert.equal(notifications.length, 0);
        else {
          assert.ok(notifications.length >= 2, mode);
          for (const notification of notifications) {
            assert.equal(notification.method, 'notifications/progress');
            assert.equal(notification.params.progressToken, 0);
            assert.equal(notification.params.total, undefined);
            assert.match(notification.params.message, /elapsed|not a completion percentage/i);
            assert.ok(!notification.params.message.includes('Synthetic'));
          }
          assert.ok(notifications[1].params.progress > notifications[0].params.progress);
        }
        if (mode === 'cancel') controller.abort();
        else if (mode === 'failure') pending.reject(new Error('Core operation timed out'));
        else pending.resolve();
        const response = await result;
        assert.equal(Boolean(response.isError), ['cancel', 'failure'].includes(mode));
        const count = notifications.length;
        context.mock.timers.tick(15000);
        await new Promise(resolve => setImmediate(resolve));
        assert.equal(notifications.length, count, 'notifications stop after the request ends');
        fixture.onRequest = undefined;
        assert.deepEqual(await call('get_session_recovery', { deck_id: created.deck_id }), before);
      }
    });
  } finally { context.mock.timers.reset(); }
});

test('accessibility MCP progress stops on cancellation without changing the document', async context => {
  context.mock.timers.enable({ apis: ['setInterval'] });
  try {
    await feedbackMcpFixture(async ({ registrations, call, fixture }) => {
      const { deck_id } = await call('create_presentation', { title: 'Synthetic accessibility' });
      const before = await call('get_session_recovery', { deck_id });
      const started = Promise.withResolvers();
      const pending = Promise.withResolvers();
      const notifications = [];
      const controller = new AbortController();
      fixture.onRequest = async (_request, { signal }) => {
        signal.addEventListener('abort', () => pending.reject(new Error('Operation cancelled')), { once: true });
        started.resolve();
        return pending.promise;
      };
      const tool = registrations.get('set_accessibility');
      const input = tool.config.inputSchema.parse({ deck_id, expected_revision: 0, slide_id: 'slide-1', element_id: 'diagram', metadata: { description: 'Synthetic diagram' } });
      const result = tool.callback(input, { signal: controller.signal, _meta: { progressToken: 0 }, sendNotification: async notification => notifications.push(notification) });
      await started.promise;
      await new Promise(resolve => setImmediate(resolve));
      context.mock.timers.tick(5000);
      await new Promise(resolve => setImmediate(resolve));
      const beforeCancel = notifications.length;
      controller.abort();
      assert.equal((await result).isError, true);
      context.mock.timers.tick(10000);
      await new Promise(resolve => setImmediate(resolve));
      assert.ok(beforeCancel >= 2, 'Accessibility authoring must emit opted-in progress');
      assert.equal(notifications.length, beforeCancel);
      assert.ok(notifications.every(notification => notification.params.progressToken === 0 && notification.params.total === undefined));
      fixture.onRequest = undefined;
      assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
    });
  } finally { context.mock.timers.reset(); }
});

test('ordinary writes imports and previews emit requested progress and stop after cancellation', async context => {
  context.mock.timers.enable({ apis: ['setInterval'] });
  try {
    await feedbackMcpFixture(async ({ registrations, call, fixture }) => {
      const { deck_id } = await call('create_presentation', { title: 'Private document title' });
      const before = await call('get_session_recovery', { deck_id });
      const cases = [
        ['apply_operations', { deck_id, expected_revision: 0, expected_hash: before.document.hash, operations: [{ op: 'set_slide_background', slide_id: 'slide-1', color: 'FFFFFF' }] }],
        ['apply_transaction', { deck_id, expected_revision: 0, operations: [{ op: 'replace', path: '/deck/title', value: 'Private title' }] }],
        ['update_notes', { deck_id, expected_revision: 0, slide_id: 'slide-1', notes: 'Private notes' }],
        ['set_frame', { deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'shape', frame: { x: 20, y: 20, width: 100, height: 80 } }],
        ['add_elements', { deck_id, expected_revision: 0, slide_id: 'slide-1', elements: [{ type: 'rect', id: 'shape', x: 20, y: 20, width: 100, height: 80, fill: 'FFFFFF' }] }],
        ['open_pptx', { base64: 'UEsDBA==' }],
        ['preview_presentation', { deck_id, options: { page_indices: [0] } }],
      ];
      for (const [name, parameters] of cases) {
        const controller = new AbortController(), started = Promise.withResolvers(), pending = Promise.withResolvers(), notifications = [];
        fixture.onRequest = async (_request, { signal }) => {
          signal.addEventListener('abort', () => pending.reject(new Error('Operation cancelled')), { once: true });
          started.resolve(); return pending.promise;
        };
        const tool = registrations.get(name);
        const response = tool.callback(tool.config.inputSchema.parse(parameters), { signal: controller.signal, _meta: { progressToken: 0 }, sendNotification: async notification => notifications.push(notification) });
        await started.promise;
        await new Promise(resolve => setImmediate(resolve));
        context.mock.timers.tick(5000);
        await new Promise(resolve => setImmediate(resolve));
        const count = notifications.length;
        controller.abort();
        assert.equal((await response).isError, true, name);
        assert.ok(count >= 2, `${name} must report opted-in progress`);
        assert.ok(notifications.every(notification => notification.params.progressToken === 0 && notification.params.total === undefined && !JSON.stringify(notification).includes('Private')));
        context.mock.timers.tick(15000);
        await new Promise(resolve => setImmediate(resolve));
        assert.equal(notifications.length, count, `${name} must stop progress`);
        fixture.onRequest = undefined;
        assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
      }
    });
  } finally { context.mock.timers.reset(); }
});

test('managed batch MCP progress send failures do not discard a changed commit', async () => {
  await feedbackMcpFixture(async ({ registrations, call, fixture }) => {
    for (const synchronous of [false, true]) {
      const { deck_id } = await call('create_presentation', { title: 'Synthetic notification failure' });
      const before = await call('get_session_recovery', { deck_id });
      const notificationAttempt = Promise.withResolvers();
      let sent = 0;
      fixture.onRequest = async request => {
        await notificationAttempt.promise;
        const document = structuredClone(request.document);
        document.revision += 1;
        document.hash = 'b'.repeat(64);
        document.deck.slides[0].notes = 'Synthetic committed change';
        return { document, receipt: { document_id: document.id, after_hash: document.hash, inverse: [] } };
      };
      const tool = registrations.get('apply_operations');
      const input = tool.config.inputSchema.parse({ deck_id, expected_revision: 0, expected_hash: before.document.hash, operations: [
        { op: 'add_part', slide_id: 'slide-1', id: 'part', spec: { version: 1, preset: 'list-horizontal/balanced', title: 'Synthetic', data: { kind: 'items', items: [{ label: 'Check' }, { label: 'Act' }] } } },
      ] });
      const response = await tool.callback(input, { signal: new AbortController().signal, _meta: { progressToken: 'changed' }, sendNotification: () => {
        sent += 1; notificationAttempt.resolve();
        if (synchronous) throw new Error('Synchronous notification failure');
        return Promise.reject(new Error('Rejected notification'));
      } });
      assert.equal(Boolean(response.isError), false);
      assert.equal(sent, 1);
      fixture.onRequest = undefined;
      const after = await call('get_session_recovery', { deck_id });
      assert.equal(after.document.revision, 1);
      assert.equal(after.document.deck.slides[0].notes, 'Synthetic committed change');
      assert.equal(after.past.length, 1);
    }
  });
});

function assertGuidedFeedbackSchema(schema) {
  const input = schema.properties.input;
  const authoring = input.properties.authoring.anyOf.find(branch => branch.type === 'object');
  const limit = authoring.properties.slide_limit.anyOf.find(branch => branch.type === 'integer');
  const headline = authoring.properties.headline_style.anyOf.find(branch => branch.type === 'string');
  assert.equal(input.properties.slides.maxItems, 128);
  assert.equal(limit.minimum, 32);
  assert.equal(limit.maximum, 128);
  assert.deepEqual(headline.enum, ['sentence', 'keyword']);
}

test('feedback MCP stub discovery shares reasoned authoring choices without core calls', async () => {
  await feedbackMcpFixture(async ({ registrations, resources, prompts, calls }) => {
    assertFeedbackWorkflow(await prompts.get('author_presentation').callback(), await resources.get('aislide://authoring/workflow').callback());
    assert.match(registrations.get('create_guided_presentation').config.description, /32 slides.*headline_style="keyword".*32\.\.128.*Evidence and numeric checks/);
    assert.match(registrations.get('compile_report').config.description, /fixed structured layouts.*exact recreation/);
    assert.equal(calls.length, 0);
  });
});

test('feedback MCP stub schemas expose bounded typed authoring and source handles', async () => {
  await feedbackMcpFixture(async ({ registrations, calls, call }) => {
    for (const name of ['apply_operations', 'add_elements', 'set_frames', 'set_text_style', 'set_slide_background', 'set_connector', 'set_picture_crop', 'set_hyperlink', 'set_shape_adjustment', 'import_slides', 'create_object']) {
      const tool = registrations.get(name);
      assert.ok(tool, name);
      assert.equal(tool.config.annotations.openWorldHint, false);
      assert.equal(tool.config.annotations.readOnlyHint, name === 'create_object');
      assert.equal(z.toJSONSchema(tool.config.inputSchema, { io: 'input' }).additionalProperties, false);
    }
    const { deck_id } = await call('create_presentation', { title: 'Synthetic target' });
    const source = await call('create_presentation', { title: 'Synthetic source' });
    const frame = { x: 0, y: 0, width: 200, height: 100 };
    const text = { type: 'text', id: 'text', ...frame, text: 'Synthetic', font_size: 24, color: '@dk1', bold: false };
    const operations = [
      { op: 'add_elements', slide_id: 'slide-1', elements: [text] },
      { op: 'set_frame', slide_id: 'slide-1', id: 'text', frame },
      { op: 'set_text_style', slide_id: 'slide-1', ids: ['text'], style: { bold: false } },
      { op: 'set_slide_background', slide_id: 'slide-1', color: 'FFFFFF' },
      { op: 'set_connector', slide_id: 'slide-1', id: 'edge', connector: { color: '@dk1', stroke_width: 2, arrow: true } },
      { op: 'set_picture_crop', slide_id: 'slide-1', id: 'image', crop: { left: 0.1 } },
      { op: 'set_hyperlink', slide_id: 'slide-1', id: 'text', link: null },
      { op: 'set_shape_adjustment', slide_id: 'slide-1', id: 'shape', adjustment: { name: 'adj', value: 25000 } },
      { op: 'add_picture', slide_id: 'slide-1', id: 'image', base64: 'c3ludGhldGlj', mime_type: 'image/png', alt: 'Synthetic', frame },
    ];
    const guarded = { deck_id, expected_revision: 0, expected_hash: 'a'.repeat(64), operations };
    const batch = registrations.get('apply_operations').config.inputSchema;
    assert.deepEqual(batch.parse(guarded), guarded);
    for (const change of [{ operations: [] }, { operations: Array(129).fill(operations[3]) }, { expected_revision: -1 }, { expected_hash: 'invalid' }, { shell: 'no' }, { operations: [{ op: 'replace', path: '/deck', value: {} }] }]) {
      assert.equal(batch.safeParse({ ...guarded, ...change }).success, false, JSON.stringify(change).slice(0, 100));
    }
    for (const operation of [
      { ...operations[0], elements: [{ ...text, unknown: true }] },
      { ...operations[0], elements: [] },
      { ...operations[1], frame: { ...frame, width: 0 } },
      { ...operations[1], frame: { ...frame, x: Infinity } },
      { ...operations[2], ids: ['text', 'text'] },
      { ...operations[2], style: {} },
      { ...operations[2], style: { bold: null } },
      { ...operations[4], connector: { ...operations[4].connector, path: 'no' } },
      { ...operations[5], crop: { left: 2 } },
      { ...operations[6], link: undefined },
      { ...operations[7], adjustment: { name: 'unsupported', value: 2 } },
      { ...operations[8], mime_type: 'image/svg+xml' },
    ]) assert.equal(batch.safeParse({ ...guarded, operations: [operation] }).success, false, operation.op);
    const beforeCalls = calls.length;
    const updated = await call('apply_operations', guarded);
    assert.equal(updated.revision, 1);
    assert.equal(calls.length, beforeCalls + 1);
    assert.deepEqual(calls.at(-1).request.operations, operations);
    assert.equal(calls.at(-1).request.op, 'apply_operations');
    assert.equal((await call('get_session_recovery', { deck_id })).past.length, 1);
    const sourceBefore = await call('get_session_recovery', { deck_id: source.deck_id });
    const importInput = { deck_id, expected_revision: 1, expected_hash: 'b'.repeat(64), source_deck_id: source.deck_id, source_slide_ids: ['slide-1'], prefix: 'copy', after: null };
    const importSchema = registrations.get('import_slides').config.inputSchema;
    for (const change of [{ source: sourceBefore.document }, { path: 'source.pptx' }, { prefix: '../copy' }, { prefix: 'x'.repeat(25) }, { source_slide_ids: [] }, { source_slide_ids: ['slide-1', 'slide-1'] }, { source_deck_id: 'invalid' }]) assert.equal(importSchema.safeParse({ ...importInput, ...change }).success, false);
    await call('import_slides', importInput);
    assert.deepEqual(calls.at(-1).request.source, sourceBefore.document);
    assert.equal('source_deck_id' in calls.at(-1).request, false);
    assert.deepEqual(await call('get_session_recovery', { deck_id: source.deck_id }), sourceBefore);
    const unknown = { ...importInput, expected_revision: 2, source_deck_id: randomUUID() };
    const count = calls.length;
    await assert.rejects(() => call('import_slides', unknown), /Unknown deck handle/);
    assert.equal(calls.length, count);
    await call('create_object', { id: 'draft', kind: 'text' });
    assert.equal(calls.at(-1).request.op, 'create_object');
  });
});

test('managed batch MCP stub accepts strict parts and graphs with one guarded request', async () => {
  await feedbackMcpFixture(async ({ registrations, calls, call }) => {
    const { deck_id } = await call('create_presentation', { title: 'Synthetic managed batch' });
    const layout = { x: 64, y: 120, width: 1152, height: 512, show_title: false };
    const part = { version: 1, preset: 'matrix-basic', title: 'Synthetic', data: { kind: 'matrix', corner_label: 'Criterion', rows: ['First', 'Second'], columns: ['A', 'B'], cells: [['a', 'b'], ['c', 'd']] }, layout };
    const graph = { version: 1, title: 'Synthetic', show_title: false, nodes: [{ id: 'node', label: 'Heading', detail: 'Detail', x: 0, y: 0 }] };
    const operations = [
      { op: 'add_part', slide_id: 'slide-1', id: 'part', spec: part },
      { op: 'update_part', slide_id: 'slide-1', id: 'part', spec: part },
      { op: 'add_graph', slide_id: 'slide-1', id: 'graph', spec: graph, layout },
      { op: 'update_graph', slide_id: 'slide-1', id: 'graph', spec: graph },
    ];
    const guarded = { deck_id, expected_revision: 0, expected_hash: 'a'.repeat(64), operations };
    const schema = registrations.get('apply_operations').config.inputSchema;
    assert.deepEqual(schema.parse(guarded), guarded);
    const published = z.toJSONSchema(schema, { io: 'input' });
    assert.equal(published.properties.operations.items.oneOf.length, 13);
    for (const operation of operations) {
      assert.equal(schema.safeParse({ ...guarded, operations: [operation] }).success, true);
      assert.equal(schema.safeParse({ ...guarded, operations: [{ ...operation, unknown: true }] }).success, false);
      assert.equal(schema.safeParse({ ...guarded, operations: [{ ...operation, spec: { ...operation.spec, unknown: true } }] }).success, false);
      assert.equal(schema.safeParse({ ...guarded, operations: [{ ...operation, id: 'x'.repeat(41) }] }).success, false);
    }
    assert.equal(schema.safeParse({ ...guarded, operations: Array(128).fill(operations[0]) }).success, true);
    for (const invalid of [[], Array(129).fill(operations[0]),
      [{ ...operations[0], layout }],
      [{ ...operations[0], spec: { ...part, data: { ...part.data, unknown: true } } }],
      [{ ...operations[0], spec: { ...part, data: { ...part.data, corner_label: null } } }],
      [{ ...operations[0], spec: { ...part, data: { ...part.data, corner_label: 'x'.repeat(49) } } }],
      [{ ...operations[2], layout: { ...layout, width: 0 } }],
      [{ ...operations[2], layout: { ...layout, unknown: true } }],
      [{ ...operations[2], spec: { ...graph, nodes: [{ ...graph.nodes[0], unknown: true }] } }],
      [{ ...operations[2], spec: { ...graph, nodes: [{ ...graph.nodes[0], icon: { base64: 'c3ludGhldGlj', mime_type: 'image/png', unknown: true } }] } }],
      [{ ...operations[2], spec: { ...graph, edges: [{ id: 'edge', source: 'node', target: 'node', unknown: true }] } }],
      [{ ...operations[2], spec: { ...graph, groups: [{ id: 'region', label: 'Region', x: 0, y: 0, width: 500, height: 300, unknown: true }] } }],
      [{ ...operations[3], layout }],
    ]) assert.equal(schema.safeParse({ ...guarded, operations: invalid }).success, false);
    for (const value of [undefined, null]) {
      const operation = { ...operations[2], layout: value };
      assert.deepEqual(schema.parse({ ...guarded, operations: [operation] }).operations, [operation]);
    }
    const count = calls.length;
    const changed = await call('apply_operations', guarded);
    assert.equal(changed.revision, 1);
    assert.equal(calls.length, count + 1);
    assert.deepEqual(calls.at(-1).request.operations, operations);
    assert.equal(calls.at(-1).request.op, 'apply_operations');
    assert.equal((await call('get_session_recovery', { deck_id })).past.length, 1);
  });
});

test('managed batch MCP stub failures cancellation busy and no-op retain state without fallback', { timeout: 10000 }, async () => {
  await feedbackMcpFixture(async ({ calls, call, fixture }) => {
    const { deck_id } = await call('create_presentation', { title: 'Synthetic guarded batch' });
    const before = await call('get_session_recovery', { deck_id });
    const part = { version: 1, preset: 'list-horizontal/balanced', title: 'Synthetic', data: { kind: 'items', items: [{ label: 'First' }, { label: 'Second' }] } };
    const operations = [
      { op: 'add_part', slide_id: 'slide-1', id: 'part', spec: part },
      { op: 'add_graph', slide_id: 'slide-1', id: 'graph', spec: { version: 1, title: 'Synthetic', nodes: [{ id: 'node', label: 'Node', x: 0, y: 88 }] } },
    ];
    const input = { deck_id, expected_revision: before.document.revision, expected_hash: before.document.hash, operations };
    for (const mode of ['failure', 'timeout', 'late-cancel', 'early-cancel']) {
      const controller = new AbortController();
      const count = calls.length;
      fixture.onRequest = (request, options) => {
        assert.equal(request.op, 'apply_operations');
        assert.deepEqual(request.operations, operations);
        assert.equal(options.signal, controller.signal);
        if (mode === 'failure') throw new Error('Core rejected stale metadata');
        if (mode === 'timeout') throw new Error('Core request timed out');
        controller.abort();
        return { document: { ...request.document, revision: 1, hash: 'b'.repeat(64), parts: [{ spec: part }] }, receipt: { inverse: [] } };
      };
      if (mode === 'early-cancel') controller.abort();
      await assert.rejects(() => call('apply_operations', input, controller.signal), /cancelled|stale|timed out/i);
      assert.equal(calls.length, count + (mode === 'early-cancel' ? 0 : 1));
      assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
    }
    let release;
    let started;
    const entered = new Promise(resolve => { started = resolve; });
    fixture.onRequest = request => new Promise(resolve => { release = () => resolve({ document: request.document, receipt: null }); started(); });
    const pending = call('apply_operations', input);
    try {
      await entered;
      const count = calls.length;
      await assert.rejects(() => call('apply_operations', input), /in progress|busy/i);
      assert.equal(calls.length, count);
      assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
    } finally { release(); await pending; }
    assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
    fixture.onRequest = request => ({ document: request.document, receipt: null });
    const noop = await call('apply_operations', { ...input, operations: [{ op: 'set_slide_background', slide_id: 'slide-1', color: 'FFFFFF' }] });
    assert.equal(noop.can_undo, false);
    assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
  });
});

test('managed batch MCP stub matrix corner labels share Unicode bounds and defaults across tools', async () => {
  await feedbackMcpFixture(async ({ registrations, calls, call }) => {
    const { deck_id } = await call('create_presentation', { title: 'Synthetic matrix contracts' });
    const base = { version: 1, preset: 'matrix/balanced', title: 'Synthetic', data: { kind: 'matrix', rows: ['First', 'Second'], columns: ['A', 'B'], cells: [['a', 'b'], ['c', 'd']] } };
    const published = z.toJSONSchema(registrations.get('create_part').config.inputSchema, { io: 'input' });
    const matrixSchema = published.properties.spec.properties.data.oneOf.find(variant => variant.properties.kind.const === 'matrix');
    assert.equal(matrixSchema.properties.corner_label.maxLength, 48);
    assert.equal(matrixSchema.required.includes('corner_label'), false);
    const guided = structuredClone(guidedExamples()[0]);
    const count = calls.length;
    for (const category of ['matrix', 'contrast']) for (const variant of ['balanced', 'focus', 'labeled']) {
      const spec = { ...base, preset: `${category}/${variant}` };
      const inputs = [
        ['create_part', part => ({ id: 'part', spec: part })],
        ...['add_part', 'update_part'].map(name => [name, part => ({ deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'part', spec: part })]),
        ...['add_part', 'update_part'].map(op => ['apply_operations', part => ({ deck_id, expected_revision: 0, expected_hash: 'a'.repeat(64), operations: [{ op, slide_id: 'slide-1', id: 'part', spec: part }] })]),
        ...['validate_guided_presentation', 'create_guided_presentation'].map(name => [name, part => ({ input: { ...guided, slides: [{ ...guided.slides[0], part }] } })]),
      ];
      for (const [name, input] of inputs) {
        const schema = registrations.get(name).config.inputSchema;
        assert.deepEqual(schema.parse(input(spec)), input(spec), `${name}: omitted defaults must reach core unchanged`);
        for (const corner_label of ['', 'Criterion', 'x'.repeat(48), '\u{20000}'.repeat(48)]) {
          const value = input({ ...spec, data: { ...spec.data, corner_label } });
          assert.deepEqual(schema.parse(value), value, name);
        }
        for (const corner_label of ['x'.repeat(49), '\u{20000}'.repeat(49), null, 3]) {
          assert.equal(schema.safeParse(input({ ...spec, data: { ...spec.data, corner_label } })).success, false, name);
        }
      }
    }
    assert.equal(calls.length, count);
    for (const name of ['add_graph', 'update_graph']) {
      const schema = registrations.get(name).config.inputSchema;
      const spec = { version: 1, title: 'Legacy', nodes: [{ id: 'node', label: 'Node', x: 0, y: 88 }] };
      const input = { deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'graph', spec };
      assert.deepEqual(schema.parse(input), input);
      assert.equal(schema.safeParse({ ...input, layout: null }).success, false, name);
      assert.equal(schema.safeParse({ ...input, expected_revision: undefined }).success, false, name);
    }
  });
});

test('feedback MCP stub guided graph and part schemas preserve optional field semantics', async () => {
  await feedbackMcpFixture(async ({ registrations, calls, call }) => {
    const graph = { version: 1, title: 'Synthetic', show_title: false, nodes: [{ id: 'node', label: 'Heading', label_fit: 'shrink', detail: '\u{20000}'.repeat(240), detail_font_size: 12, text_align: 'left', heading_bold: false, font_size: 18, x: 0, y: 0, height: 512 }], groups: [{ id: 'region', label: 'Region', x: 0, y: 0, width: 1152, height: 512, header_color: '@accent2' }] };
    await call('create_graph', { id: 'graph', spec: graph });
    assert.deepEqual(calls.at(-1).request.spec, graph);
    const graphSchema = registrations.get('create_graph').config.inputSchema;
    for (const changes of [{ detail: ' ' }, { detail: 'x'.repeat(241) }, { detail_font_size: 41 }, { text_align: 'justify' }, { unknown: true }]) {
      assert.equal(graphSchema.safeParse({ id: 'graph', spec: { ...graph, nodes: [{ ...graph.nodes[0], ...changes }] } }).success, false);
    }
    await call('transform_graph', { spec: graph, operations: [{ op: 'put_node', node: graph.nodes[0] }, { op: 'put_group', group: { id: 'region', label: 'Region', x: 0, y: 0, width: 1152, height: 512 } }] });
    assert.equal(calls.at(-1).request.operations[1].group.height, 512);
    const legacy = { version: 1, title: 'Legacy', nodes: [{ id: 'node', label: 'Legacy', x: 0, y: 88 }] };
    await call('create_graph', { id: 'legacy', spec: legacy });
    assert.deepEqual(calls.at(-1).request.spec, legacy);
    const part = { version: 1, preset: 'process-basic', title: 'Synthetic', data: { kind: 'items', items: [{ label: 'First' }, { label: 'Second' }] }, layout: { x: 20, y: 30, width: 600, height: 300, show_title: false } };
    await call('create_part', { id: 'part', spec: part });
    assert.deepEqual(calls.at(-1).request.spec, part);
    const partSchema = registrations.get('create_part').config.inputSchema;
    for (const layout of [{ ...part.layout, width: 0 }, { ...part.layout, x: 4097 }, { ...part.layout, font_size: 12 }]) assert.equal(partSchema.safeParse({ id: 'part', spec: { ...part, layout } }).success, false);
    const input = structuredClone(guidedExamples()[0]);
    input.slides[0].part = { version: 1, preset: 'diagram/custom', title: '', data: { kind: 'diagram', graph } };
    const evidenceIds = Array.from({ length: 16 }, (_, index) => `evidence-${index}`);
    input.slides[0].support = [{ clause: 'Synthetic', body_paths: ['/data/items/0'], evidence_ids: evidenceIds }];
    input.authoring = { headline_style: 'keyword', slide_limit: 128 };
    input.slides = Array.from({ length: 128 }, (_, index) => ({ ...input.slides[0], id: `slide-${index}` }));
    await call('validate_guided_presentation', { input });
    assert.deepEqual(calls.at(-1).request.input.slides[0].part.data.graph, graph);
    assert.deepEqual(calls.at(-1).request.input.authoring, { headline_style: 'keyword', slide_limit: 128 });
    const guidedSchema = registrations.get('validate_guided_presentation').config.inputSchema;
    for (const name of ['validate_guided_presentation', 'create_guided_presentation']) {
      assert.ok(registrations.get(name).config.inputSchema.safeParse({ input }).success);
      assertGuidedFeedbackSchema(z.toJSONSchema(registrations.get(name).config.inputSchema, { io: 'input' }));
    }
    for (const authoring of [{ slide_limit: 31 }, { slide_limit: 129 }, { slide_limit: 32.5 }, { headline_style: 'freeform' }, { headline_style: 'keyword', unknown: true }]) assert.equal(guidedSchema.safeParse({ input: { ...input, authoring } }).success, false);
    assert.equal(guidedSchema.safeParse({ input: { ...input, slides: [...input.slides, input.slides[0]] } }).success, false);
  });
});

test('feedback MCP stub complete elements and ergonomic tools retain strict nested types', async () => {
  await feedbackMcpFixture(async ({ registrations, calls, call }) => {
    const { deck_id } = await call('create_presentation', { title: 'Synthetic elements' });
    const frame = { x: 20, y: 20, width: 300, height: 150 };
    const text = { id: 'text', type: 'text', ...frame, text: 'Synthetic', font_size: 24, color: '@dk1', bold: false, format: { paragraphs: [{ runs: [{ text: 'Synthetic', style: { italic: true, color: '@accent1' } }], alignment: 'right' }] } };
    const elements = [
      text,
      { id: 'rect', type: 'rect', ...frame, fill: '@accent1', visual: { opacity: 0.5 } },
      { id: 'polygon', type: 'polygon', ...frame, points: [[0, 0], [1, 0], [0, 1]], fill: 'none', stroke: '@dk1', stroke_width: 1 },
      { ...text, id: 'shape', type: 'shape', preset: 'roundRect', fill: '@lt1', stroke: '@dk1', stroke_width: 1, rotation: 0, visual: { adjustments: [{ name: 'adj', value: 25000 }] } },
      { id: 'table', type: 'table', ...frame, rows: [['Synthetic']], font_size: 20, format: { cells: [{ row: 0, column: 0, style: { fill: '@lt1', text_style: { bold: true } } }] } },
      { id: 'chart', type: 'chart', ...frame, kind: 'column', categories: ['Synthetic'], series: [{ name: 'Synthetic', values: [1], color: '@accent1' }] },
      { id: 'picture', type: 'picture', ...frame, base64: 'c3ludGhldGlj', mime_type: 'image/png', alt: 'Synthetic', crop: { left: 0 } },
      { id: 'edge', type: 'connector', ...frame, color: '@dk1', stroke_width: 1, arrow: true, start: { element_id: 'rect', site: 0 }, routing: { points: [[0, 0], [1, 1]] } },
      { id: 'group', type: 'group', ...frame, view_width: 300, view_height: 150, children: [{ ...text, id: 'child' }] },
    ];
    const schema = registrations.get('add_elements').config.inputSchema;
    const guarded = { deck_id, expected_revision: 0, slide_id: 'slide-1', elements };
    assert.deepEqual(schema.parse(guarded), guarded);
    const invalid = structuredClone(guarded);
    invalid.elements[8].children[0].format.paragraphs[0].runs[0].style.execute = 'no';
    assert.equal(schema.safeParse(invalid).success, false);
    assert.equal(schema.safeParse({ ...guarded, elements: Array(129).fill(text) }).success, false);
    const operations = [
      ['add_elements', { elements }],
      ['set_frame', { id: 'rect', frame }],
      ['set_text_style', { ids: ['text'], style: { italic: false } }],
      ['set_slide_background', { color: 'FFFFFF' }],
      ['set_connector', { id: 'edge', connector: { color: '@dk1', stroke_width: 1, arrow: false, start: null, end: null, routing: null } }],
      ['set_picture_crop', { id: 'picture', crop: { top: 0.1 } }],
      ['set_hyperlink', { id: 'text', link: 'https://example.com' }],
      ['set_shape_adjustment', { id: 'shape', adjustment: { name: 'adj', value: 10000 } }],
      ['add_picture', { id: 'new-picture', base64: 'c3ludGhldGlj', mime_type: 'image/png', alt: 'Synthetic', frame, crop: { top: 0 } }],
    ];
    for (const [index, [name, input]] of operations.entries()) {
      const count = calls.length;
      await call(name, { deck_id, expected_revision: index, slide_id: 'slide-1', ...input });
      assert.equal(calls.length, count + 1, name);
      assert.equal(calls.at(-1).request.op, 'apply_operations');
      assert.deepEqual(calls.at(-1).request.operations, [{ ...input, op: name, slide_id: 'slide-1' }]);
    }
    await call('set_frames', { deck_id, expected_revision: 9, slide_id: 'slide-1', frames: [{ id: 'rect', frame }, { id: 'text', frame }] });
    assert.deepEqual(calls.at(-1).request.operations, ['rect', 'text'].map(id => ({ op: 'set_frame', slide_id: 'slide-1', id, frame })));
    await call('add_picture', { deck_id, slide_id: 'slide-1', id: 'legacy', base64: 'c3ludGhldGlj', mime_type: 'image/png', alt: 'Synthetic' });
    assert.equal(calls.at(-1).request.expected_revision, 10);
    assert.equal(calls.at(-1).request.operations[0].frame, undefined);
    const count = calls.length;
    await assert.rejects(() => call('set_slide_background', { deck_id, expected_revision: 0, slide_id: 'slide-1', color: 'FFFFFF' }), /Revision conflict/);
    assert.equal(calls.length, count);
  });
});

test('managed batch MCP live 39 slides and 21 managed roots retain metadata through native updates and Undo', { timeout: 360000 }, async context => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-managed-batch-mcp-'));
  const client = new Client({ name: 'managed-batch-transport-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full', '--output-dir', directory], stderr: 'pipe', env: coreEnvironment });
  const requests = [];
  const progressEvents = [];
  const request = async (name, args = {}) => {
    requests.push(name);
    return client.callTool({ name, arguments: args }, undefined, { signal: context.signal, timeout: 320000, resetTimeoutOnProgress: true, maxTotalTimeout: 320000, onprogress: progress => progressEvents.push({ tool: name, ...progress }) });
  };
  const call = async (name, args = {}) => {
    const result = await request(name, args);
    assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content).slice(0, 2000)}`);
    return JSON.parse(result.content.find(content => content.type === 'text').text);
  };
  const guard = (deck_id, document) => ({ deck_id, expected_revision: document.revision, expected_hash: document.hash });
  const frameOf = ({ x, y, width, height }) => ({ x, y, width, height });
  const elementsOf = elements => elements.flatMap(element => [element, ...(element.type === 'group' ? elementsOf(element.children) : [])]);
  const assertManaged = document => {
    assert.equal(document.deck.slides.length, 39);
    assert.equal(new Set(document.deck.slides.map(slide => slide.id)).size, 39);
    assert.equal(document.parts.length, 21);
    assert.ok(document.parts.every(part => part.stale === false));
    assert.equal(new Set(document.parts.map(part => `${part.slide_id}/${part.element_id}`)).size, 21);
    const elements = document.deck.slides.flatMap(slide => elementsOf(slide.elements));
    assert.equal(new Set(elements.map(element => element.id)).size, elements.length);
    for (const part of document.parts) {
      const root = document.deck.slides.find(slide => slide.id === part.slide_id).elements.find(element => element.id === part.element_id);
      assert.ok(root, part.element_id);
      assert.equal(root.type, 'group');
      assert.ok(root.x >= 0 && root.y >= 0 && root.x + root.width <= 1280 && root.y + root.height <= 720);
      assert.deepEqual(frameOf(root), frameOf(part.spec.layout));
      if (part.spec.data.kind === 'matrix') {
        assert.equal(part.spec.data.corner_label, 'Criterion');
        assert.ok(elementsOf([root]).some(element => element.text === 'Criterion' || element.rows?.[0]?.[0] === 'Criterion'));
      }
    }
  };
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    assert.equal(new Set(tools.map(tool => tool.name)).size, tools.length);
    const batchSchema = tools.find(tool => tool.name === 'apply_operations').inputSchema;
    const items = resolveSchemaRef(batchSchema, resolveSchemaRef(batchSchema, batchSchema.properties.operations).items);
    const variants = items.oneOf ?? items.anyOf;
    const operations = variants.map(variant => resolveSchemaRef(batchSchema, resolveSchemaRef(batchSchema, variant).properties.op).const).sort();
    assert.equal(operations.length, 13);
    const capabilities = await call('authoring_capabilities');
    assert.deepEqual([...capabilities.typed_authoring.operations].sort(), operations);
    assert.equal(capabilities.typed_authoring.batch_limit, 128);
    assert.equal(capabilities.typed_authoring.one_undo, true);
    const { deck_id } = await call('create_presentation', { title: 'Synthetic managed assembly' });
    const initial = await call('get_document', { deck_id });
    await call('edit_slides', { deck_id, expected_revision: initial.revision, operations: Array.from({ length: 38 }, (_, index) => ({ op: 'insert', id: `managed-slide-${index + 2}`, title: `Synthetic ${index + 2}` })) });
    const blank = await call('get_document', { deck_id });
    assert.equal(blank.deck.slides.length, 39);
    assert.ok(blank.deck.slides.every(slide => slide.elements.length === 0));
    const sourceBytes = Buffer.from('Synthetic evidence for the managed-batch test.');
    const ingested = await call('ingest_source', { name: 'synthetic.txt', format: 'text', base64: sourceBytes.toString('base64') });
    await call('apply_transaction', { deck_id, expected_revision: blank.revision, operations: [{ op: 'add', path: '/sources/-', value: ingested.source }] });
    await call('close_source', { source_id: ingested.source_id });
    const before = await call('get_session_recovery', { deck_id });
    const catalog = await call('part_catalog');
    const presetIds = [
      ...['matrix', 'contrast'].flatMap(category => ['balanced', 'focus', 'labeled'].map(variant => `${category}/${variant}`)),
      ...['list-horizontal', 'vertical-bar-graph', 'horizontal-bar-graph', 'line-graph', 'pie-chart', 'tree', 'flow', 'vertical-flow', 'cycle', 'before-after', 'layer'].map(category => `${category}/balanced`),
    ];
    assert.equal(presetIds.length, 17);
    const layout = { x: 64, y: 144, width: 1152, height: 512, show_title: false };
    const batch = presetIds.map((preset, index) => {
      const entry = catalog.presets.find(entry => entry.id === preset);
      assert.ok(entry, preset);
      const spec = { ...structuredClone(entry.example), layout: { ...layout, show_title: entry.example.data.kind === 'chart' } };
      if (spec.data.kind === 'matrix') spec.data.corner_label = 'Criterion';
      return { op: 'add_part', slide_id: before.document.deck.slides[index].id, id: `managed-part-${index}`, spec };
    });
    for (let index = 0; index < 4; index += 1) batch.push({
      op: 'add_graph', slide_id: before.document.deck.slides[index + 17].id, id: `managed-graph-${index}`, layout,
      spec: { version: 1, title: `Synthetic graph ${index}`, show_title: false, nodes: [
        { id: 'source', label: 'Source', detail: 'Synthetic input', x: 40, y: 60, width: 280, height: 120 },
        { id: 'target', label: 'Target', detail: 'Synthetic output', x: 600, y: 60, width: 280, height: 120 },
      ], edges: [{ id: 'flow', source: 'source', target: 'target', source_port: 'right', target_port: 'left', route: 'straight', arrow: true }] },
    });
    assert.equal(batch.length, 21);
    assert.ok(Buffer.byteLength(JSON.stringify(batch)) < 64 * 1024);
    const requestCount = requests.length;
    const progressCount = progressEvents.length;
    const added = await call('apply_operations', { ...guard(deck_id, before.document), operations: batch });
    assert.deepEqual(requests.slice(requestCount), ['apply_operations']);
    const batchProgress = progressEvents.slice(progressCount);
    assert.ok(batchProgress.length > 0, 'actual stdio client receives requested progress');
    assert.ok(batchProgress.every(event => event.tool === 'apply_operations' && event.total === undefined && /elapsed/.test(event.message)));
    assert.ok(batchProgress.every((event, index) => index === 0 || event.progress > batchProgress[index - 1].progress));
    const populated = await call('get_session_recovery', { deck_id });
    assert.equal(added.revision, before.document.revision + 1);
    assert.equal(populated.document.revision, added.revision);
    assert.equal(populated.past.length, before.past.length + 1);
    assertManaged(populated.document);
    assert.deepEqual(populated.document.sources, before.document.sources);
    assert.deepEqual(populated.document.bindings, before.document.bindings);
    await call('undo', { deck_id });
    const removed = await call('get_session_recovery', { deck_id });
    assert.equal(removed.document.hash, before.document.hash);
    assert.deepEqual(removed.document.deck, before.document.deck);
    assert.deepEqual(removed.document.parts, before.document.parts);
    assert.deepEqual(removed.document.sources, before.document.sources);
    assert.equal(removed.past.length, before.past.length);
    assert.equal(removed.future.length, 1);
    await call('redo', { deck_id });
    const restored = await call('get_document', { deck_id });
    assert.equal(restored.hash, populated.document.hash);
    assertManaged(restored);
    const exported = await call('export_pptx', { deck_id, filename: 'managed-original.pptx' });
    const originalBytes = await readFile(exported.path);
    const opened = await call('open_pptx', { base64: originalBytes.toString('base64') });
    const native = await call('get_session_recovery', { deck_id: opened.deck_id });
    assertManaged(native.document);
    assert.deepEqual(Buffer.from(native.document.origin.base64, 'base64'), originalBytes);
    assert.deepEqual(native.document.sources, before.document.sources);
    assert.deepEqual(native.document.bindings, before.document.bindings);
    const noop = await call('export_pptx', { deck_id: opened.deck_id, filename: 'managed-noop.pptx' });
    assert.deepEqual(await readFile(noop.path), originalBytes);
    const part = native.document.parts.find(part => part.element_id === 'managed-part-0');
    const graph = native.document.parts.find(part => part.element_id === 'managed-graph-0');
    const updatedPart = structuredClone(part.spec);
    updatedPart.data.cells[0][0] = 'Updated';
    const updatedGraph = structuredClone(graph.spec.data.graph);
    updatedGraph.nodes[0].label = 'Changed source';
    const updates = [
      { op: 'update_part', slide_id: part.slide_id, id: part.element_id, spec: updatedPart },
      { op: 'update_graph', slide_id: graph.slide_id, id: graph.element_id, spec: updatedGraph },
    ];
    await call('apply_operations', { ...guard(opened.deck_id, native.document), operations: updates });
    const changed = await call('get_session_recovery', { deck_id: opened.deck_id });
    assert.equal(changed.document.revision, native.document.revision + 1);
    assert.equal(changed.past.length, native.past.length + 1);
    assertManaged(changed.document);
    assert.equal(changed.document.parts.find(entry => entry.element_id === part.element_id).spec.data.cells[0][0], 'Updated');
    assert.equal(changed.document.parts.find(entry => entry.element_id === graph.element_id).spec.data.graph.nodes[0].label, 'Changed source');
    assert.deepEqual(changed.document.parts.find(entry => entry.element_id === graph.element_id).spec.layout, graph.spec.layout);
    assert.deepEqual(changed.document.origin, native.document.origin);
    assert.deepEqual(changed.document.sources, native.document.sources);
    assert.deepEqual(changed.document.bindings, native.document.bindings);
    for (const entry of native.document.parts.filter(entry => ![part.element_id, graph.element_id].includes(entry.element_id))) {
      assert.deepEqual(changed.document.parts.find(candidate => candidate.element_id === entry.element_id), entry);
    }
    await call('apply_operations', { ...guard(opened.deck_id, changed.document), operations: updates });
    assert.deepEqual(await call('get_session_recovery', { deck_id: opened.deck_id }), changed);
    const root = changed.document.deck.slides.find(slide => slide.id === part.slide_id).elements.find(element => element.id === part.element_id);
    const child = elementsOf(root.children).find(element => element.type === 'text' && element.text);
    assert.ok(child);
    const failed = await request('apply_operations', { ...guard(opened.deck_id, changed.document), operations: [
      { op: 'set_text_style', slide_id: part.slide_id, ids: [child.id], style: { color: 'CC0011' } },
      updates[0],
    ] });
    assert.equal(failed.isError, true);
    assert.match(failed.content[0].text, /stale|modified/i);
    assert.deepEqual(await call('get_session_recovery', { deck_id: opened.deck_id }), changed);
    const changedExport = await call('export_pptx', { deck_id: opened.deck_id, filename: 'managed-changed.pptx' });
    const changedBytes = await readFile(changedExport.path);
    const changedOpened = await call('open_pptx', { base64: changedBytes.toString('base64') });
    const persisted = await call('get_document', { deck_id: changedOpened.deck_id });
    assertManaged(persisted);
    assert.equal(persisted.parts.find(entry => entry.element_id === part.element_id).spec.data.cells[0][0], 'Updated');
    assert.equal(persisted.parts.find(entry => entry.element_id === graph.element_id).spec.data.graph.nodes[0].label, 'Changed source');
    assert.deepEqual(persisted.parts.find(entry => entry.element_id === graph.element_id).spec.layout, graph.spec.layout);
    assert.deepEqual(persisted.sources, native.document.sources);
    await call('undo', { deck_id: opened.deck_id });
    const undone = await call('get_session_recovery', { deck_id: opened.deck_id });
    assert.equal(undone.document.hash, native.document.hash);
    for (const field of ['deck', 'parts', 'sources', 'bindings', 'origin']) assert.deepEqual(undone.document[field], native.document[field]);
    assert.equal(undone.past.length, native.past.length);
    assert.equal(undone.future.length, 1);
    const undoneExport = await call('export_pptx', { deck_id: opened.deck_id, filename: 'managed-undone.pptx' });
    assert.deepEqual(await readFile(undoneExport.path), originalBytes);
    assert.deepEqual(await readFile(exported.path), originalBytes);
    assert.deepEqual(Buffer.from(ingested.source.text), sourceBytes);
  } finally {
    try { await client.close(); } finally { await rm(directory, { recursive: true, force: true }); }
  }
});

test('feedback MCP live typed batches and authored slide import preserve content and atomic history', { timeout: 120000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-feedback-mcp-'));
  const client = new Client({ name: 'feedback-batch-transport-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full', '--output-dir', directory], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content)}`);
    return JSON.parse(result.content[0].text);
  };
  const guard = (deck_id, document) => ({ deck_id, expected_revision: document.revision, expected_hash: document.hash });
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    for (const name of ['apply_operations', 'add_elements', 'set_frame', 'set_frames', 'set_text_style', 'set_slide_background', 'set_connector', 'set_picture_crop', 'set_hyperlink', 'set_shape_adjustment', 'import_slides', 'create_object']) {
      const tool = tools.find(tool => tool.name === name);
      assert.ok(tool, name);
      assert.equal(tool.annotations.openWorldHint, false, name);
      assert.equal(tool.annotations.readOnlyHint, name === 'create_object', name);
      assert.equal(tool.inputSchema.additionalProperties, false, name);
    }
    for (const [name, property] of [['apply_operations', 'operations'], ['add_elements', 'elements'], ['import_slides', 'source_slide_ids']]) {
      const schema = tools.find(tool => tool.name === name).inputSchema;
      assert.equal(schema.properties[property].minItems, 1);
      assert.equal(schema.properties[property].maxItems, 128);
    }
    const capabilities = await call('authoring_capabilities');
    assert.equal(capabilities.typed_authoring.batch_limit, 128);
    assert.equal(capabilities.typed_authoring.scale_fonts, false);
    assert.equal(capabilities.typed_authoring.one_undo, true);
    assert.equal(capabilities.slide_import.source, 'authored_document_only');
    assert.equal(capabilities.slide_import.office_visual_parity, false);
    assertFeedbackWorkflow(await client.getPrompt({ name: 'author_presentation' }), await client.readResource({ uri: 'aislide://authoring/workflow' }));
    const source = await call('create_presentation', { title: 'Synthetic batch source' });
    const target = await call('create_presentation', { title: 'Synthetic merge target' });
    const sourceInitial = await call('get_session_recovery', { deck_id: source.deck_id });
    const targetInitial = await call('get_session_recovery', { deck_id: target.deck_id });
    for (const initial of [sourceInitial, targetInitial]) {
      assert.equal(initial.document.deck.slides.length, 1);
      assert.deepEqual(initial.document.deck.slides[0].elements, []);
      assert.deepEqual(initial.past, []);
    }
    const slide_id = sourceInitial.document.deck.slides[0].id;
    const picture = await call('create_asset', { id: 'pixel', mime_type: 'image/svg+xml', size: 40, alt: 'Synthetic square', base64: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="red"/></svg>').toString('base64') });
    assert.equal(Buffer.from(picture.base64, 'base64').subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
    const elements = [
      { type: 'text', id: 'text', x: 40, y: 40, width: 600, height: 140, text: 'One\nTwo', font_size: 28, color: '202525', bold: false, format: { paragraphs: [
        { alignment: 'right', space_after: { kind: 'points', value: 600 }, runs: [{ text: 'One', style: { italic: true } }] },
        { bullet: 'bullet', bullet_character: '-', runs: [{ text: 'Two', style: { color: 'AA0000' } }] },
      ] } },
      { type: 'shape', id: 'shape', x: 760, y: 40, width: 240, height: 140, preset: 'roundRect', fill: '087F73', stroke: '202525', stroke_width: 3, text: 'Label', font_size: 28, color: 'FFFFFF', bold: true, format: { alignment: 'center', vertical: 'middle' }, visual: { opacity: 0.7, adjustments: [{ name: 'adj', value: 10000 }] } },
      { type: 'connector', id: 'edge', x: 640, y: 110, width: 120, height: 1, color: '087F73', stroke_width: 2, arrow: true, start: { element_id: 'text', site: 3 }, end: { element_id: 'shape', site: 1 } },
      { type: 'polygon', id: 'polygon', x: 40, y: 280, width: 200, height: 100, points: [[0, 0], [1, 0], [0.5, 1]], fill: '087F73', stroke: '202525', stroke_width: 2 },
    ];
    const frame = { x: 300, y: 280, width: 240, height: 120 };
    const crop = { left: 0.1, right: 0.2, top: 0.05, bottom: 0.15 };
    const added = await call('apply_operations', { ...guard(source.deck_id, sourceInitial.document), operations: [
      { op: 'add_elements', slide_id, elements },
      { op: 'add_picture', slide_id, id: 'image', base64: picture.base64, mime_type: 'image/png', alt: 'Synthetic square', frame, crop },
    ] });
    const populated = await call('get_session_recovery', { deck_id: source.deck_id });
    assert.equal(added.revision, 1);
    assert.equal(added.hash, populated.document.hash);
    assert.equal(added.can_undo, true);
    assert.equal(populated.past.length, 1);
    assert.equal(populated.document.deck.slides[0].elements.length, 5);
    const find = (document, id) => document.deck.slides[0].elements.find(element => element.id === id);
    for (const expected of elements) {
      const actual = find(populated.document, expected.id);
      for (const [key, value] of Object.entries(expected)) {
        if (key !== 'format' && key !== 'visual') assert.deepEqual(actual[key], value, `${expected.id}.${key}`);
      }
    }
    const image = find(populated.document, 'image');
    assert.deepEqual(image, { ...image, ...frame, crop, base64: picture.base64, alt: 'Synthetic square' });
    const paragraphs = find(populated.document, 'text').format.paragraphs;
    assert.equal(paragraphs[0].alignment, 'right');
    assert.deepEqual(paragraphs[0].space_after, elements[0].format.paragraphs[0].space_after);
    assert.equal(paragraphs[0].runs[0].style.italic, true);
    assert.equal(paragraphs[1].bullet, 'bullet');
    assert.equal(paragraphs[1].bullet_character, '-');
    assert.equal(paragraphs[1].runs[0].style.color, 'AA0000');
    const shape = find(populated.document, 'shape');
    assert.equal(shape.format.alignment, 'center');
    assert.equal(shape.format.vertical, 'middle');
    assert.equal(shape.visual.opacity, 0.7);
    assert.deepEqual(shape.visual.adjustments, elements[1].visual.adjustments);
    const textFrame = { x: 80, y: 60, width: 420, height: 120 };
    const shapeFrame = { x: 760, y: 60, width: 300, height: 160 };
    await call('apply_operations', { ...guard(source.deck_id, populated.document), operations: [
      { op: 'set_frame', slide_id, id: 'text', frame: textFrame },
      { op: 'set_frame', slide_id, id: 'shape', frame: shapeFrame },
      { op: 'set_text_style', slide_id, ids: ['text'], style: { font_size: 20, bold: true } },
      { op: 'set_slide_background', slide_id, color: 'F3F5F7' },
      { op: 'set_picture_crop', slide_id, id: 'image', crop: { left: 0.2 } },
    ] });
    const changed = await call('get_session_recovery', { deck_id: source.deck_id });
    assert.equal(changed.document.revision, 2);
    assert.equal(changed.past.length, 2);
    assert.deepEqual(find(changed.document, 'shape'), { ...shape, ...shapeFrame });
    const expectedText = { ...structuredClone(find(populated.document, 'text')), ...textFrame, font_size: 20, bold: true };
    for (const paragraph of expectedText.format.paragraphs) for (const run of paragraph.runs) Object.assign(run.style, { font_size: 20, bold: true });
    assert.deepEqual(find(changed.document, 'text'), expectedText);
    assert.equal(changed.document.deck.slides[0].background, 'F3F5F7');
    assert.equal(changed.document.deck.slides[0].inherit_background ?? false, false);
    assert.deepEqual(find(changed.document, 'image'), { ...image, crop: { left: 0.2, right: 0, top: 0, bottom: 0 } });
    for (const id of ['edge', 'polygon']) assert.deepEqual(find(changed.document, id), find(populated.document, id));
    await call('undo', { deck_id: source.deck_id });
    const undone = await call('get_session_recovery', { deck_id: source.deck_id });
    assert.equal(undone.document.hash, populated.document.hash);
    assert.deepEqual(undone.document.deck, populated.document.deck);
    assert.equal(undone.past.length, 1);
    assert.equal(undone.future.length, 1);
    const validOperation = { op: 'set_slide_background', slide_id, color: 'ABCDEF' };
    const guarded = guard(source.deck_id, undone.document);
    for (const input of [
      { ...guarded, operations: [validOperation, { op: 'set_frame', slide_id, id: 'missing', frame: textFrame }] },
      { ...guarded, operations: [validOperation, { op: 'set_frame', slide_id, id: 'text', frame: { ...textFrame, unknown: true } }] },
      { ...guarded, expected_revision: 0, operations: [validOperation] },
      { ...guarded, expected_hash: '0'.repeat(64), operations: [validOperation] },
    ]) {
      const result = await client.callTool({ name: 'apply_operations', arguments: input });
      assert.equal(result.isError, true, JSON.stringify(result.content));
      assert.deepEqual(await call('get_session_recovery', { deck_id: source.deck_id }), undone);
    }
    await call('redo', { deck_id: source.deck_id });
    const sourceBefore = await call('get_session_recovery', { deck_id: source.deck_id });
    assert.equal(sourceBefore.document.hash, changed.document.hash);
    assert.deepEqual(sourceBefore.document.deck, changed.document.deck);
    assert.equal(sourceBefore.past.length, 2);
    assert.deepEqual(sourceBefore.future, []);
    const merged = await call('import_slides', { ...guard(target.deck_id, targetInitial.document), source_deck_id: source.deck_id, source_slide_ids: [slide_id], prefix: 'copy', after: targetInitial.document.deck.slides[0].id });
    const targetBefore = await call('get_session_recovery', { deck_id: target.deck_id });
    assert.equal(merged.revision, 1);
    assert.equal(merged.slides, 2);
    assert.equal(merged.hash, targetBefore.document.hash);
    assert.equal(targetBefore.past.length, 1);
    assert.deepEqual(targetBefore.document.deck.design, targetInitial.document.deck.design);
    assert.deepEqual(targetBefore.document.deck.slides[0], targetInitial.document.deck.slides[0]);
    const imported = targetBefore.document.deck.slides[1];
    assert.notEqual(imported.id, slide_id);
    assert.deepEqual(imported.elements, sourceBefore.document.deck.slides[0].elements);
    assert.equal(imported.background, 'F3F5F7');
    assert.equal(imported.layout_id, targetInitial.document.deck.slides[0].layout_id);
    assert.deepEqual(await call('get_session_recovery', { deck_id: source.deck_id }), sourceBefore);
    const exported = await call('export_pptx', { deck_id: target.deck_id, filename: 'merged.pptx' });
    const bytes = await readFile(exported.path);
    const native = await call('open_pptx', { base64: bytes.toString('base64') });
    const nativeBefore = await call('get_session_recovery', { deck_id: native.deck_id });
    assert.ok(nativeBefore.document.origin);
    assert.equal(nativeBefore.document.deck.slides[1].elements.find(element => element.id === 'image').base64, picture.base64);
    assert.equal(nativeBefore.document.deck.slides[1].elements.find(element => element.id === 'text').text, 'One\nTwo');
    const rejected = await client.callTool({ name: 'import_slides', arguments: { ...guard(target.deck_id, targetBefore.document), source_deck_id: native.deck_id, source_slide_ids: [nativeBefore.document.deck.slides[1].id], prefix: 'native' } });
    assert.equal(rejected.isError, true);
    assert.match(rejected.content[0].text, /authored|native/i);
    assert.deepEqual(await call('get_session_recovery', { deck_id: target.deck_id }), targetBefore);
    assert.deepEqual(await call('get_session_recovery', { deck_id: native.deck_id }), nativeBefore);
    const unchanged = await call('export_pptx', { deck_id: native.deck_id, filename: 'native-noop.pptx' });
    assert.deepEqual(await readFile(unchanged.path), bytes);
    await call('undo', { deck_id: target.deck_id });
    const restored = await call('get_session_recovery', { deck_id: target.deck_id });
    assert.equal(restored.document.hash, targetInitial.document.hash);
    assert.deepEqual(restored.document.deck, targetInitial.document.deck);
    assert.deepEqual(restored.past, []);
    assert.equal(restored.future.length, 1);
    assert.deepEqual(await call('get_session_recovery', { deck_id: source.deck_id }), sourceBefore);
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});

test('feedback MCP live guided keyword opt-in and titleless graph part layouts survive cross-API updates', { timeout: 120000 }, async () => {
  const client = new Client({ name: 'feedback-layout-transport-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content)}`);
    return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    for (const name of ['validate_guided_presentation', 'create_guided_presentation']) {
      assertGuidedFeedbackSchema(tools.find(tool => tool.name === name).inputSchema);
    }
    const input = structuredClone(guidedExamples().find(input => input.profile_id === 'technical-explainer'));
    const template = input.slides[0];
    input.language = 'en';
    input.title = 'Synthetic transport briefing';
    input.audience = 'Test reviewers';
    input.purpose = 'Exercise guided authoring with synthetic assumptions';
    input.governing_message = 'Review the evidence before proceeding.';
    input.evidence = [{ id: 'proposal', kind: 'assumption', reference: 'Synthetic test fixture', statement: 'Illustrative review and action stages, not observed results.' }];
    template.headline = input.governing_message;
    template.part = { version: 1, preset: 'list-horizontal/balanced', title: 'Stages', subtitle: 'Synthetic review sequence', data: { kind: 'items', items: [{ label: 'Check', detail: 'Review' }, { label: 'Act', detail: 'Proceed' }] } };
    template.support = [{ clause: template.headline, body_paths: ['/data/items'], evidence_ids: ['proposal'] }];
    template.numbers = [];
    input.slides = Array.from({ length: 32 }, (_, index) => ({ ...structuredClone(template), id: `page-${index}` }));
    const defaultReview = await call('validate_guided_presentation', { input });
    assert.equal(defaultReview.ready, true, JSON.stringify(defaultReview));
    input.slides.push(...Array.from({ length: 7 }, (_, index) => ({ ...structuredClone(template), id: `page-${index + 32}` })));
    const overDefault = await call('validate_guided_presentation', { input });
    assert.equal(overDefault.ready, false);
    assert.match(JSON.stringify(overDefault.issues), /1\.\.32/);
    input.authoring = { headline_style: 'keyword', slide_limit: 39 };
    for (const slide of input.slides) { slide.headline = 'Boundary'; slide.support[0].clause = 'Boundary'; }
    const sentenceInput = { ...input, profile_id: 'consulting-decision', language: 'ja', authoring: { slide_limit: 39 }, slides: [input.slides[0]] };
    const sentenceDefault = await call('validate_guided_presentation', { input: sentenceInput });
    assert.equal(sentenceDefault.ready, false);
    assert.match(JSON.stringify(sentenceDefault.issues), /30-56/);
    assert.equal((await call('validate_guided_presentation', { input: { ...sentenceInput, authoring: { headline_style: 'keyword' } } })).ready, true);
    const review = await call('validate_guided_presentation', { input });
    assert.equal(review.ready, true, JSON.stringify(review));
    assert.equal(input.issues, undefined);
    const created = await call('create_guided_presentation', { input });
    assert.equal(created.slides, 39);
    assert.equal(created.profile_id, 'technical-explainer');
    assert.equal(created.model_inference, false);
    const guided = await call('get_document', { deck_id: created.deck_id });
    assert.equal(guided.deck.slides.length, 39);
    assert.equal(new Set(guided.deck.slides.map(slide => slide.id)).size, 39);
    assert.equal(guided.parts.length, 39);
    assert.ok(guided.parts.every(part => !part.stale));
    assert.ok(guided.deck.slides.every(slide => slide.elements.some(element => element.type === 'text' && element.text === 'Boundary')));
    const unsupported = { ...input, slides: [structuredClone(input.slides[0])] };
    unsupported.slides[0].support[0].evidence_ids = ['missing'];
    const evidenceReview = await call('validate_guided_presentation', { input: unsupported });
    assert.equal(evidenceReview.ready, false);
    assert.match(JSON.stringify(evidenceReview.issues), /missing evidence/);
    const numeric = structuredClone(guidedExamples().find(input => input.profile_id === 'status-report'));
    numeric.profile_id = 'technical-explainer';
    numeric.authoring = { headline_style: 'keyword' };
    numeric.slides = [numeric.slides[0]];
    assert.equal((await call('validate_guided_presentation', { input: numeric })).ready, true);
    numeric.slides[0].numbers = [];
    const numericReview = await call('validate_guided_presentation', { input: numeric });
    assert.equal(numericReview.ready, false);
    assert.match(JSON.stringify(numericReview.issues), /no source/);
    await call('close_deck', { deck_id: created.deck_id });

    const { deck_id } = await call('create_presentation', { title: 'Synthetic layout transport' });
    const graph = { version: 1, title: 'Hidden graph heading', subtitle: 'Hidden subtitle', show_title: false, nodes: [
      { id: 'client', label: 'Client', detail: 'Validate access\nRecord outcome', detail_font_size: 16, text_align: 'left', heading_bold: false, font_size: 24, x: 40, y: 0, width: 320, height: 200 },
      { id: 'api', label: 'API', x: 600, y: 120, width: 220, height: 112 },
    ], edges: [{ id: 'request', source: 'client', target: 'api', source_port: 'right', target_port: 'left', route: 'straight' }], groups: [] };
    const assertDetails = (group, detailText) => {
      assert.ok(group.children.every(child => !child.id.endsWith('-title') && !child.id.endsWith('-subtitle')));
      const heading = group.children.find(child => child.id.endsWith('-nt-client'));
      const detail = group.children.find(child => child.id.endsWith('-nd-client'));
      assert.ok(heading);
      assert.ok(detail);
      assert.equal(heading.text, 'Client');
      assert.equal(heading.bold, false);
      assert.equal(heading.format.alignment, 'left');
      assert.equal(detail.format.alignment, 'left');
      assert.equal(detail.text, detailText);
      assert.ok(detail.y >= heading.y + heading.height);
      assert.ok(detail.font_size >= 12 && detail.font_size <= heading.font_size);
    };
    const inserted = await call('add_graph', { deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'direct', spec: graph });
    let current = await call('get_document', { deck_id });
    assert.equal(inserted.revision, 1);
    assertDetails(current.deck.slides[0].elements[0], graph.nodes[0].detail);
    assert.equal(current.deck.slides[0].elements[0].children.find(child => child.id.endsWith('-n-client')).y, 0);
    graph.nodes[0].detail = 'Updated direct detail';
    await call('update_graph', { deck_id, expected_revision: current.revision, slide_id: 'slide-1', id: 'direct', spec: graph });
    current = await call('get_document', { deck_id });
    assertDetails(current.deck.slides[0].elements[0], graph.nodes[0].detail);
    assert.equal((await call('get_graph', { deck_id, slide_id: 'slide-1', id: 'direct' })).spec.show_title, false);
    await call('edit_slides', { deck_id, expected_revision: current.revision, operations: [{ op: 'insert', id: 'part-slide', after: 'slide-1', title: 'Explicit body box' }] });
    current = await call('get_document', { deck_id });
    const bodyGraph = structuredClone(graph);
    delete bodyGraph.show_title;
    bodyGraph.nodes[0].y = 120;
    const layout = { x: 24, y: 36, width: 1152, height: 424, show_title: false };
    const part = { version: 1, preset: 'diagram/custom', title: bodyGraph.title, subtitle: bodyGraph.subtitle, data: { kind: 'diagram', graph: bodyGraph }, layout };
    await call('add_part', { deck_id, expected_revision: current.revision, slide_id: 'part-slide', id: 'bounded', spec: part });
    const snapshot = await call('get_session_recovery', { deck_id });
    const assertLayout = document => {
      const metadata = document.parts.find(entry => entry.element_id === 'bounded');
      assert.deepEqual(metadata.spec.layout, layout);
      assert.equal(metadata.stale, false);
      const group = document.deck.slides.find(slide => slide.id === 'part-slide').elements.find(element => element.id === 'bounded');
      for (const key of ['x', 'y', 'width', 'height']) assert.equal(group[key], layout[key], key);
      assert.equal(group.view_width, layout.width);
      assert.equal(group.view_height, layout.height);
      assertDetails(group, metadata.spec.data.graph.nodes[0].detail);
      return group;
    };
    assertLayout(snapshot.document);
    for (const [name, args] of [
      ['update_graph', { spec: bodyGraph }],
      ['apply_graph', { operations: [{ op: 'move', ids: ['client'], dx: 0, dy: 0 }] }],
    ]) {
      await call(name, { deck_id, expected_revision: snapshot.document.revision, slide_id: 'part-slide', id: 'bounded', ...args });
      assert.deepEqual(await call('get_session_recovery', { deck_id }), snapshot, `${name} must preserve layout, hash and both histories`);
    }
    part.data.graph.nodes[0].detail = 'Updated part detail';
    await call('update_part', { deck_id, expected_revision: snapshot.document.revision, slide_id: 'part-slide', id: 'bounded', spec: part });
    current = await call('get_document', { deck_id });
    assert.equal(current.revision, snapshot.document.revision + 1);
    assertLayout(current);
    await call('apply_graph', { deck_id, expected_revision: current.revision, slide_id: 'part-slide', id: 'bounded', operations: [{ op: 'move', ids: ['client'], dx: 16, dy: 0 }] });
    current = await call('get_document', { deck_id });
    assert.equal(assertLayout(current).children.find(child => child.id.endsWith('-n-client')).x, 56);
    const updatedGraph = (await call('get_graph', { deck_id, slide_id: 'part-slide', id: 'bounded' })).spec;
    updatedGraph.nodes[0].detail = 'Updated through graph API';
    await call('update_graph', { deck_id, expected_revision: current.revision, slide_id: 'part-slide', id: 'bounded', spec: updatedGraph });
    const beforeReject = await call('get_session_recovery', { deck_id });
    assertLayout(beforeReject.document);
    const rejected = await client.callTool({ name: 'update_part', arguments: { deck_id, expected_revision: beforeReject.document.revision, slide_id: 'part-slide', id: 'bounded', spec: { ...part, layout: { ...layout, width: 20, height: 20 } } } });
    assert.equal(rejected.isError, true);
    assert.deepEqual(await call('get_session_recovery', { deck_id }), beforeReject);
    await call('undo', { deck_id });
    const restored = await call('get_session_recovery', { deck_id });
    assert.equal(restored.document.hash, current.hash);
    assert.deepEqual(restored.document.deck, current.deck);
    assertLayout(restored.document);
  } finally { await client.close(); }
});

test('master import MCP schemas are bounded, strict and expose read-only inspection and preview', async () => {
  const client = new Client({ name: 'master-import-schema-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const deck_id = '00000000-0000-4000-8000-000000000001';
  const input = { kind: 'potx', base64: 'c3ludGhldGlj', source_sha256: 'a'.repeat(64), mode: 'masters', ids: ['master-1'], prefix: 'brand', name: 'Synthetic brand' };
  const guarded = { deck_id, expected_revision: 0, expected_hash: 'b'.repeat(64), input };
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    for (const [name, readOnly, required] of [
      ['inspect_master_source', true, ['kind']],
      ['preview_master_import', true, ['deck_id', 'expected_revision', 'expected_hash', 'input']],
      ['import_masters', false, ['deck_id', 'expected_revision', 'expected_hash', 'input', 'expected_candidate_hash']],
    ]) {
      const tool = tools.find(tool => tool.name === name);
      assert.ok(tool, name);
      assert.equal(tool.annotations.readOnlyHint, readOnly);
      assert.equal(tool.annotations.openWorldHint, false);
      assert.equal(tool.inputSchema.additionalProperties, false);
      assert.deepEqual([...tool.inputSchema.required].sort(), required.sort());
      if (name === 'inspect_master_source') {
        assert.deepEqual(tool.inputSchema.oneOf, [{ required: ['base64'], not: { required: ['asset_id'] } }, { required: ['asset_id'], not: { required: ['base64'] } }]);
      } else {
        const schema = resolveSchemaRef(tool.inputSchema, tool.inputSchema.properties.input);
        assert.equal(schema.additionalProperties, false);
        assert.deepEqual([...schema.required].sort(), Object.keys(input).sort());
        const ids = resolveSchemaRef(tool.inputSchema, schema.properties.ids);
        assert.equal(ids.minItems, 1);
        assert.equal(ids.maxItems, 8);
        assert.equal(resolveSchemaRef(tool.inputSchema, ids.items).maxLength, 80);
      }
    }
    const invalid = [
      ['inspect_master_source', { kind: 'potx' }],
      ['inspect_master_source', { kind: 'potx', base64: input.base64, asset_id: deck_id }],
      ['inspect_master_source', { kind: 'thmx', base64: input.base64 }],
      ['inspect_master_source', { kind: 'potx', base64: '' }],
      ['inspect_master_source', { kind: 'potx', base64: input.base64, path: 'source.potx' }],
      ['inspect_master_source', { kind: 'potx', base64: input.base64, capacity_profile: 'unlimited' }],
      ['preview_master_import', { ...guarded, expected_revision: -1 }],
      ['preview_master_import', { ...guarded, expected_revision: Number.MAX_SAFE_INTEGER + 1 }],
      ['preview_master_import', { ...guarded, expected_hash: 'not-a-hash' }],
      ['preview_master_import', { ...guarded, capacity_profile: 'large' }],
      ['import_masters', { ...guarded, expected_candidate_hash: 'not-a-hash' }],
      ['import_masters', guarded],
    ];
    for (const change of [
      { kind: 'thmx' }, { mode: 'all' }, { ids: [] }, { ids: Array.from({ length: 9 }, (_, index) => `master-${index}`) },
      { ids: ['master-1', 'master-1'] }, { ids: [' '] }, { ids: ['x'.repeat(81)] }, { source_sha256: 'g'.repeat(64) },
      { source_sha256: 'a'.repeat(63) }, { prefix: '' }, { prefix: '../brand' }, { prefix: 'x'.repeat(33) },
      { name: ' ' }, { name: 'x'.repeat(61) }, { unknown: true }, { base64: '' },
    ]) {
      invalid.push(['preview_master_import', { ...guarded, input: { ...input, ...change } }]);
      invalid.push(['import_masters', { ...guarded, expected_candidate_hash: 'c'.repeat(64), input: { ...input, ...change } }]);
    }
    for (const [name, arguments_] of invalid) {
      const result = await client.callTool({ name, arguments: arguments_ });
      assert.equal(result.isError, true, name);
      assert.doesNotMatch(result.content[0].text, /Unknown deck handle|Core execution failed|unknown variant/i);
    }
    const overBudget = await client.callTool({ name: 'inspect_master_source', arguments: { kind: 'potx', base64: 'A'.repeat(4 * 1048576), capacity_profile: 'legacy' } });
    assert.equal(overBudget.isError, true);
    assert.match(overBudget.content[0].text, /selected capacity profile/i);
  } finally { await client.close(); }
});

test('master import MCP POTX inspection, preview, apply and one Undo preserve the target', { timeout: 120000 }, async () => {
  const { AislideClient } = await import('../packages/client/index.mjs');
  const { requestCore } = await import('./core-client.mjs');
  const { createHash } = await import('node:crypto');
  const sdk = new AislideClient(requestCore);
  const source = await sdk.createPresentation('master-source-mcp', 'Synthetic source');
  const sourceBefore = source.recoveryEnvelope;
  const exported = await source.exportTemplate('potx');
  const client = new Client({ name: 'master-import-workflow-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content));
    assert.ok(result.content.every(block => block.type === 'text'));
    return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const catalog = await call('inspect_master_source', { kind: 'potx', base64: exported.base64, capacity_profile: 'standard' });
    assert.equal(catalog.kind, 'potx');
    assert.equal(catalog.source_sha256, createHash('sha256').update(Buffer.from(exported.base64, 'base64')).digest('hex'));
    assert.equal(catalog.office_visual_parity, false);
    assert.equal(catalog.width, source.document.deck.width);
    assert.equal(catalog.height, source.document.deck.height);
    const master = catalog.masters.find(master => master.importable);
    assert.ok(master, JSON.stringify(catalog));
    assert.equal(master.reason, null);
    const { deck_id } = await call('create_presentation', { title: 'Synthetic target', capacity_profile: 'standard' });
    await call('update_notes', { deck_id, expected_revision: 0, slide_id: 'slide-1', notes: 'Retain target notes' });
    const before = await call('get_session_recovery', { deck_id });
    const input = { kind: 'potx', base64: exported.base64, source_sha256: catalog.source_sha256, mode: 'masters', ids: [master.id], prefix: 'brand', name: 'Imported brand' };
    const args = { deck_id, expected_revision: before.document.revision, expected_hash: before.document.hash, input };
    const preview = await call('preview_master_import', args);
    assert.equal(preview.base_revision, before.document.revision);
    assert.equal(preview.base_hash, before.document.hash);
    assert.equal(preview.source_sha256, catalog.source_sha256);
    assert.equal(preview.office_visual_parity, false);
    assert.match(preview.candidate_hash, /^[a-f0-9]{64}$/);
    assert.notEqual(preview.candidate_hash, before.document.hash);
    assert.equal(preview.design.masters.length, before.document.deck.design.masters.length + 1);
    assert.equal(preview.master_ids.length, 1);
    assert.equal(preview.layout_ids.length, master.layout_count);
    assert.equal(preview.preview_slides.length, master.layout_count);
    assert.deepEqual(preview.preview_slides.map(slide => slide.layout_id), preview.layout_ids);
    assert.equal('candidate_id' in preview, false);
    assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
    const apply = { ...args, expected_candidate_hash: preview.candidate_hash };
    for (const change of [
      { expected_revision: before.document.revision - 1 }, { expected_hash: '0'.repeat(64) },
      { input: { ...input, source_sha256: '0'.repeat(64) } }, { input: { ...input, ids: ['missing-master'] } },
    ]) {
      for (const name of ['preview_master_import', 'import_masters']) {
        const result = await client.callTool({ name, arguments: { ...(name === 'import_masters' ? apply : args), ...change } });
        assert.equal(result.isError, true, name);
      }
    }
    for (const change of [{ expected_candidate_hash: '0'.repeat(64) }, { input: { ...input, name: 'Changed after preview' } }]) {
      assert.equal((await client.callTool({ name: 'import_masters', arguments: { ...apply, ...change } })).isError, true);
    }
    assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
    const applied = await call('import_masters', apply);
    assert.equal(applied.revision, before.document.revision + 1);
    assert.equal(applied.hash, preview.candidate_hash);
    const after = await call('get_session_recovery', { deck_id });
    assert.deepEqual(after.document.deck.design, preview.design);
    assert.deepEqual({ ...after.document.deck, design: before.document.deck.design }, before.document.deck);
    for (const field of ['id', 'origin', 'sources', 'bindings', 'parts', 'report']) assert.deepEqual(after.document[field], before.document[field], field);
    assert.equal(after.past.length, before.past.length + 1);
    assert.equal((await client.callTool({ name: 'import_masters', arguments: apply })).isError, true);
    assert.deepEqual(await call('get_session_recovery', { deck_id }), after);
    await call('undo', { deck_id });
    const restored = await call('get_session_recovery', { deck_id });
    assert.equal(restored.document.hash, before.document.hash);
    assert.deepEqual(restored.document.deck, before.document.deck);
    assert.equal(restored.past.length, before.past.length);
    assert.equal(restored.future.length, 1);
    assert.equal((await client.callTool({ name: 'import_masters', arguments: apply })).isError, true);
    await call('preview_master_import', { ...args, expected_revision: restored.document.revision });
    assert.deepEqual(await call('get_session_recovery', { deck_id }), restored);
    assert.deepEqual(source.recoveryEnvelope, sourceBefore);
  } finally { await client.close(); }
});

test('P1 MCP finalization publishes a traceable new bundle without changing the session', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-finalize-mcp-'));
  const client = new Client({ name: 'finalization-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full', '--output-dir', directory], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content)); return result;
  };
  const metadata = result => JSON.parse(result.content[0].text);
  try {
    await client.connect(transport);
    const { deck_id } = metadata(await call('create_presentation', { title: 'Synthetic delivery' }));
    await call('update_notes', { deck_id, expected_revision: 0, slide_id: 'slide-1', notes: 'User supplied delivery notes.' });
    const before = metadata(await call('get_session_recovery', { deck_id }));
    const request = { deck_id, expected_revision: before.document.revision, expected_hash: before.document.hash, name: 'delivery', options: { page_indices: [0], pdf: true, preview: 'contact_sheet', notes: true, source_report: true, preflight: true, max_dimension: 640 } };
    const response = await call('finalize_presentation', request);
    const delivery = metadata(response);
    assert.equal(delivery.status, 'complete');
    assert.equal(delivery.revision, before.document.revision);
    assert.equal(delivery.hash, before.document.hash);
    assert.equal(response.content.filter(block => block.type === 'image').length, 1);
    assert.deepEqual(delivery.files.map(file => file.kind).sort(), ['notes', 'pdf', 'pptx', 'preview', 'source_report']);
    const manifestBytes = await readFile(delivery.manifest.path);
    const manifest = JSON.parse(manifestBytes.toString('utf8'));
    assert.equal(manifest.format, 'aislide.delivery');
    assert.equal(manifest.document.hash, before.document.hash);
    assert.equal(manifest.producer.name, 'aislide');
    assert.equal(manifest.producer.transport, 'mcp-stdio');
    assert.equal(manifest.checks.office_visual_parity, false);
    assert.deepEqual(manifest.checks.preflight_page_indices, [0]);
    assert.equal(manifest.multi_file_atomic, false);
    const { createHash } = await import('node:crypto');
    assert.equal(delivery.manifest.sha256, createHash('sha256').update(manifestBytes).digest('hex'));
    for (const file of delivery.files) {
      const bytes = await readFile(file.path);
      assert.equal(bytes.length, file.bytes);
      assert.equal(createHash('sha256').update(bytes).digest('hex'), file.sha256);
      assert.deepEqual(file.page_indices, [0]);
      assert.ok(manifest.files.some(entry => entry.filename === file.filename && entry.sha256 === file.sha256));
      if (file.kind === 'pptx') assert.equal(bytes.subarray(0, 2).toString(), 'PK');
      if (file.kind === 'notes') assert.match(bytes.toString('utf8'), /User supplied delivery notes/);
      if (file.kind === 'source_report') assert.equal(JSON.parse(bytes.toString('utf8')).source_authenticity_verified, false);
    }
    assert.equal((await client.callTool({ name: 'finalize_presentation', arguments: request })).isError, true);
    assert.deepEqual(await readFile(delivery.manifest.path), manifestBytes);
    for (const change of [{ name: '../escape' }, { name: 'CON' }, { expected_hash: '0'.repeat(64) }, { options: { ...request.options, page_indices: [1] } }]) {
      assert.equal((await client.callTool({ name: 'finalize_presentation', arguments: { ...request, name: 'invalid', ...change } })).isError, true);
    }
    assert.deepEqual(metadata(await call('get_session_recovery', { deck_id })), before);
    assert.equal((await readdir(directory)).length, 6);
    await writeFile(join(directory, 'collision.manifest.json'), 'Unrelated existing manifest', { flag: 'wx' });
    const conflict = await client.callTool({ name: 'finalize_presentation', arguments: { ...request, name: 'collision', options: { preview: 'none', preflight: false } } });
    assert.equal(conflict.isError, true);
    const failure = metadata(conflict);
    assert.equal(failure.code, 'BUNDLE_PUBLICATION_FAILED');
    assert.equal(failure.status, 'not_published');
    assert.deepEqual(failure.published_paths, []);
    assert.deepEqual(failure.pending_filenames, ['collision.pptx', 'collision.manifest.json']);
    assert.equal(await readFile(join(directory, 'collision.manifest.json'), 'utf8'), 'Unrelated existing manifest');
    assert.equal((await readdir(directory)).length, 7);
    assert.equal((await client.listTools()).tools.find(tool => tool.name === 'finalize_presentation').annotations.readOnlyHint, false);
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});

test('MCP open list recommendations render native text and keep failed edits atomic', async () => {
  const client = new Client({ name: 'open-list-integration-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content));
    return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const catalog = await call('part_catalog');
    assert.equal(catalog.presets.length, 111);
    for (const id of ['list/rows', 'list-horizontal/columns', 'list-enumeration/grid']) {
      const preset = catalog.presets.find(entry => entry.id === id);
      assert.equal(preset.recommended, true);
      assert.ok(preset.use_when.length > 0); assert.ok(preset.avoid_when.length > 0);
      const element = await call('create_part', { id: 'open-list', spec: preset.example });
      assert.ok(element.children.every(child => child.type === 'text'));
      assert.ok(element.children.every(child => ['@dk1', '@dk2'].includes(child.color)));
    }
    const { deck_id } = await call('create_presentation', { title: 'Synthetic open lists' });
    const original = await call('get_document', { deck_id });
    const spec = catalog.presets.find(entry => entry.id === 'list/rows').example;
    await call('add_part', { deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'open-list', spec });
    const before = await call('get_document', { deck_id });
    assert.equal(before.parts[0].spec.preset, 'list/rows');
    const invalid = structuredClone(spec); invalid.preset = 'list-horizontal/columns'; invalid.data.items.push({ label: 'Excess topic', detail: 'Synthetic' });
    const rejected = await client.callTool({ name: 'update_part', arguments: { deck_id, expected_revision: before.revision, slide_id: 'slide-1', id: 'open-list', spec: invalid } });
    assert.equal(rejected.isError, true);
    assert.match(JSON.stringify(rejected.content), /2-4 items/);
    assert.deepEqual(await call('get_document', { deck_id }), before);
    await call('undo', { deck_id });
    assert.equal((await call('get_document', { deck_id })).hash, original.hash);
  } finally { await client.close(); }
});

test('MCP bounded graph annotations honor small fonts and report actionable group bounds', async () => {
  const client = new Client({ name: 'bounded-graph-feedback-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content));
    return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const { deck_id } = await call('create_presentation', { title: 'Synthetic small graph annotations' });
    const current = await call('get_document', { deck_id });
    const slide_id = current.deck.slides[0].id;
    const spec = { version: 1, title: 'Small annotations', show_title: false, nodes: [
      { id: 'source', label: 'Source', x: 48, y: 40, width: 200, height: 96, group: 'boundary' },
      { id: 'target', label: 'Target', x: 560, y: 240, width: 220, height: 112, group: 'boundary' },
    ], edges: [{ id: 'flow', source: 'source', target: 'target', label: 'HTTPS', label_font_size: 11, badge: { number: 2, position: 0.75, font_size: 11 } }],
    groups: [{ id: 'boundary', label: 'Boundary', x: 0, y: 10, width: 1000, height: 450, padding: 12, header_height: 30, header_font_size: 11 }] };
    const operation = { op: 'add_graph', slide_id, id: 'architecture', spec, layout: { x: 64, y: 144, width: 1152, height: 512, show_title: false } };
    await call('apply_operations', { deck_id, expected_revision: current.revision, expected_hash: current.hash, operations: [operation] });
    const inserted = await call('get_document', { deck_id });
    const children = inserted.deck.slides[0].elements[0].children;
    for (const suffix of ['-et-flow', '-eb-flow', '-gt-boundary']) assert.equal(children.find(child => child.id.endsWith(suffix)).font_size, 11);
    const invalid = structuredClone(operation);
    invalid.id = 'invalid'; invalid.spec.nodes[0].x = 8;
    const rejected = await client.callTool({ name: 'apply_operations', arguments: { deck_id, expected_revision: inserted.revision, expected_hash: inserted.hash, operations: [invalid] } });
    assert.equal(rejected.isError, true);
    const error = JSON.stringify(rejected.content);
    for (const detail of ['source', 'boundary', 'x >= 12', 'y >= 40', 'actual x=8, y=40']) assert.ok(error.includes(detail), error);
    assert.deepEqual(await call('get_document', { deck_id }), inserted);
    await call('undo', { deck_id });
    assert.equal((await call('get_document', { deck_id })).hash, current.hash);
  } finally { await client.close(); }
});

test('MCP layout preflight reports rounded-container warnings and informational badges without mutation', async () => {
  const client = new Client({ name: 'layout-preflight-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content));
    return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const { deck_id } = await call('create_presentation', { title: 'Synthetic layout checks' });
    const current = await call('get_document', { deck_id });
    await call('apply_operations', { deck_id, expected_revision: current.revision, expected_hash: current.hash, operations: [{ op: 'add_elements', slide_id: current.deck.slides[0].id, elements: [
      { type: 'shape', id: 'card', preset: 'roundRect', x: 40, y: 40, width: 400, height: 240, fill: 'FFFFFF', stroke: '087F73', stroke_width: 2, text: '', font_size: 16, color: '000000', bold: false },
      { type: 'rect', id: 'accent', x: 40, y: 40, width: 8, height: 240, fill: '087F73' },
      { type: 'connector', id: 'route', x: 100, y: 216, width: 220, height: 0.01, color: '000000', stroke_width: 2, arrow: true },
      { type: 'shape', id: 'badge', preset: 'ellipse', x: 200, y: 200, width: 32, height: 32, fill: '087F73', stroke: '087F73', stroke_width: 1, text: '2', font_size: 12, color: 'FFFFFF', bold: true },
    ] }] });
    const before = await call('get_session_recovery', { deck_id });
    const result = await call('preflight_presentation', { deck_id, options: { page_indices: [0] } });
    assert.ok(result.checks.includes('container_clearance'));
    assert.ok(result.findings.some(finding => finding.code === 'CONTAINER_CORNER_OVERFLOW' && finding.severity === 'warning' && finding.element_ids.join(',') === 'accent,card'));
    assert.ok(result.findings.some(finding => finding.code === 'CONNECTOR_BADGE_OVERLAP' && finding.severity === 'info' && finding.element_ids.join(',') === 'badge,route'));
    assert.equal(result.hash, before.document.hash);
    assert.equal(result.office_visual_parity, false);
    assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
  } finally { await client.close(); }
});

test('P0 MCP guided options, diagnostics and staged revisions form a guarded visual loop', async () => {
  const client = new Client({ name: 'authoring-loop-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const prompts = await client.listPrompts();
    assert.ok(prompts.prompts.some(prompt => prompt.name === 'author_presentation'));
    const workflow = await client.getPrompt({ name: 'author_presentation' });
    assert.match(workflow.messages[0].content.text, /preview_slide_revision/);
    const resource = await client.readResource({ uri: 'aislide://authoring/workflow' });
    assert.match(resource.contents[0].text, /preflight_presentation/);
    const input = structuredClone(guidedExamples().find(input => input.profile_id === 'event-talk'));
    input.authoring = { context: 'projection', body_font_min: 24, density: 'comfortable', spacing: 'standard' };
    input.slides[0].speaker_notes = 'Synthetic speaker notes supplied through MCP.';
    const review = await call('validate_guided_presentation', { input });
    assert.equal(review.ready, true, JSON.stringify(review));
    const created = await call('create_guided_presentation', { input });
    const deck_id = created.deck_id;
    const before = await call('get_session_recovery', { deck_id });
    assert.match(before.document.deck.slides[0].notes, /Synthetic speaker notes/);
    const slide = before.document.deck.slides[0];
    const target = slide.elements.find(element => element.type === 'text' && element.id.endsWith('-headline')) ?? slide.elements.find(element => element.type === 'text');
    assert.ok(target);
    const diagnostics = await call('preflight_presentation', { deck_id, options: { page_indices: [0], min_font_size: 24 } });
    assert.equal(diagnostics.hash, before.document.hash);
    assert.equal(diagnostics.office_visual_parity, false);
    assert.ok(diagnostics.checks.includes('renderer_warnings'));
    const args = { deck_id, expected_revision: before.document.revision, expected_hash: before.document.hash, slide_id: slide.id, edits: [{ op: 'replace_text', id: target.id, text: 'Revised synthetic headline' }], max_dimension: 640 };
    const response = await client.callTool({ name: 'preview_slide_revision', arguments: args });
    assert.ok(!response.isError, JSON.stringify(response.content));
    assert.equal(response.content.filter(block => block.type === 'image').length, 2);
    const candidate = JSON.parse(response.content[0].text);
    assert.ok(candidate.candidate_id);
    assert.notEqual(candidate.before.images[0].sha256, candidate.after.images[0].sha256);
    assert.ok(candidate.expires_in_seconds > 0);
    assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
    const other = await call('create_presentation', { title: 'Other document' });
    assert.equal((await client.callTool({ name: 'apply_slide_revision', arguments: { deck_id: other.deck_id, candidate_id: candidate.candidate_id, expected_revision: 0, expected_hash: before.document.hash } })).isError, true);
    const apply = { deck_id, candidate_id: candidate.candidate_id, expected_revision: before.document.revision, expected_hash: before.document.hash };
    const applied = await call('apply_slide_revision', apply);
    assert.equal(applied.hash, candidate.candidate_hash);
    assert.equal(applied.revision, before.document.revision + 1);
    assert.equal((await call('get_session_recovery', { deck_id })).past.length, before.past.length + 1);
    assert.equal((await client.callTool({ name: 'apply_slide_revision', arguments: apply })).isError, true);
    await call('undo', { deck_id });
    assert.equal((await call('get_document', { deck_id })).hash, before.document.hash);
    const current = await call('get_document', { deck_id });
    const stale = await call('preview_slide_revision', { ...args, expected_revision: current.revision });
    await call('update_notes', { deck_id, expected_revision: current.revision, slide_id: slide.id, notes: 'Later edit' });
    assert.equal((await client.callTool({ name: 'apply_slide_revision', arguments: { ...apply, candidate_id: stale.candidate_id, expected_revision: current.revision } })).isError, true);
    for (const invalid of [{ authoring: { body_font_min: 41 } }, { authoring: { unknown: true } }]) {
      assert.equal((await client.callTool({ name: 'create_guided_presentation', arguments: { input: { ...input, ...invalid } } })).isError, true);
    }
    const tools = (await client.listTools()).tools;
    for (const name of ['preflight_presentation', 'preview_slide_revision']) assert.equal(tools.find(tool => tool.name === name).annotations.readOnlyHint, true);
    assert.equal(tools.find(tool => tool.name === 'apply_slide_revision').annotations.readOnlyHint, false);
  } finally { await client.close(); }
});

test('P0 MCP previews return bounded images without changing documents or writing files', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-preview-mcp-'));
  const client = new Client({ name: 'preview-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full', '--output-dir', directory], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const created = await call('create_presentation', { title: 'Preview only' });
    const deck_id = created.deck_id;
    await call('edit_slides', { deck_id, expected_revision: 0, operations: [{ op: 'duplicate', slide_id: 'slide-1', id: 'second' }] });
    const before = await call('get_session_recovery', { deck_id });
    const options = { page_indices: [1, 0], max_dimension: 640, layout: 'contact_sheet' };
    const result = await client.callTool({ name: 'preview_presentation', arguments: { deck_id, options } });
    assert.ok(!result.isError, JSON.stringify(result.content));
    const metadata = JSON.parse(result.content[0].text);
    assert.equal(metadata.revision, before.document.revision);
    assert.equal(metadata.hash, before.document.hash);
    assert.deepEqual(metadata.pages.map(page => page.slide_id), ['second', 'slide-1']);
    assert.equal(metadata.office_visual_parity, false);
    assert.equal(metadata.images.length, 1);
    const images = result.content.filter(block => block.type === 'image');
    assert.equal(images.length, 1);
    assert.equal(images[0].mimeType, 'image/png');
    const sharp = (await import('sharp')).default;
    const decoded = await sharp(Buffer.from(images[0].data, 'base64')).metadata();
    assert.equal(decoded.width, metadata.images[0].width);
    assert.equal(decoded.height, metadata.images[0].height);
    assert.ok(decoded.width <= 640 && decoded.height <= 640);
    assert.ok(Buffer.byteLength(JSON.stringify(result)) <= 4 * 1048576);
    const plain = await client.callTool({ name: 'preview_presentation', arguments: { deck_id, options, include_images: false } });
    assert.ok(!plain.isError, JSON.stringify(plain.content));
    assert.equal(plain.content.length, 1);
    for (const invalid of [
      { options: { ...options, page_indices: [0, 0] } },
      { options: { ...options, page_indices: [2] } },
      { options: { ...options, page_indices: Array.from({ length: 9 }, (_, index) => index) } },
      { options: { ...options, max_dimension: 4096 } },
      { filename: '../preview.png' },
    ]) assert.equal((await client.callTool({ name: 'preview_presentation', arguments: { deck_id, options, ...invalid } })).isError, true);
    assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
    assert.deepEqual(await readdir(directory), []);
    const tools = (await client.listTools()).tools;
    assert.equal(tools.find(tool => tool.name === 'preview_presentation').annotations.readOnlyHint, true);
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});

test('P0 MCP revision capacity rejects the seventeenth candidate and releases stale candidates', async () => {
  const client = new Client({ name: 'candidate-capacity-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const { deck_id } = await call('create_presentation', { title: 'Candidate budgets' });
    await call('apply_transaction', { deck_id, expected_revision: 0, operations: [{ op: 'add', path: '/deck/slides/0/elements/-', value: { type: 'text', id: 'target', x: 20, y: 20, width: 300, height: 60, text: 'Before', font_size: 24, color: '000000', bold: false } }] });
    const before = await call('get_session_recovery', { deck_id });
    const request = { deck_id, expected_revision: before.document.revision, expected_hash: before.document.hash, slide_id: 'slide-1', edits: [{ op: 'replace_text', id: 'target', text: 'After' }], max_dimension: 160, include_images: false };
    const candidates = [];
    for (let index = 0; index < 16; index++) candidates.push(await call('preview_slide_revision', request));
    const rejected = await client.callTool({ name: 'preview_slide_revision', arguments: request });
    assert.equal(rejected.isError, true); assert.match(rejected.content[0].text, /16 live/);
    assert.deepEqual(await call('get_session_recovery', { deck_id }), before);
    await call('update_notes', { deck_id, expected_revision: before.document.revision, slide_id: 'slide-1', notes: 'New base' });
    const current = await call('get_document', { deck_id });
    const fresh = await call('preview_slide_revision', { ...request, expected_revision: current.revision, expected_hash: current.hash });
    assert.ok(fresh.candidate_id);
    assert.equal((await client.callTool({ name: 'apply_slide_revision', arguments: { deck_id, candidate_id: candidates[0].candidate_id, expected_revision: current.revision, expected_hash: current.hash } })).isError, true);
    await call('close_deck', { deck_id });
    assert.equal((await client.callTool({ name: 'apply_slide_revision', arguments: { deck_id, candidate_id: fresh.candidate_id, expected_revision: current.revision, expected_hash: current.hash } })).isError, true);
  } finally { await client.close(); }
});

test('phase3 MCP projection and generated SVG are strict read-only helpers', async () => {
  const client = new Client({ name: 'phase3-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => { const result = await client.callTool({ name, arguments: args }); assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text); };
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    for (const name of ['compute_chart_presentation', 'render_element_preview']) assert.equal(tools.find(tool => tool.name === name).annotations.readOnlyHint, true);
    const created = await call('create_presentation', { title: 'Synthetic projection' });
    const before = await call('get_document', { deck_id: created.deck_id });
    const input = { kind: 'line', categories: ['1','2','3'], series: [{ name: 'Observed', color: '087F73', values: [3,5,7], trendline: { kind: 'linear', forward: 1 } }] };
    const projection = await call('compute_chart_presentation', input);
    assert.ok(Math.abs(projection.series[0].trend.points.at(-1).y - 9) < 1e-10);
    const element = { type: 'text', id: 'warp', x: 0, y: 0, width: 300, height: 200, text: 'office affinity', font_size: 32, bold: false, color: '087F73', visual: { text_warp: 'deflate' } };
    assert.ok((await call('render_element_preview', { element })).svg.includes('<path'));
    for (const [name, args] of [['compute_chart_presentation', { ...input, unexpected: true }], ['render_element_preview', { element, svg: '<script/>' }], ['render_element_preview', { element: { ...element, visual: { text_warp: 'unknown' } } }]]) assert.equal((await client.callTool({ name, arguments: args })).isError, true);
    assert.deepEqual(await call('get_document', { deck_id: created.deck_id }), before);
  } finally { await client.close(); }
});

test('G25 G27 MCP notes and auxiliary masters are strict atomic operations', async () => {
  const client = new Client({ name: 'native-notes-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => { const result = await client.callTool({ name, arguments: args }); assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text); };
  try {
    await client.connect(transport);
    const created = await call('create_presentation', { title: 'Synthetic rich notes' });
    const before = await call('get_document', { deck_id: created.deck_id });
    const paragraphs = [{ runs: [{ text: 'Notes', style: { bold: true } }] }];
    await call('update_rich_notes', { deck_id: created.deck_id, expected_revision: 0, slide_id: 'slide-1', paragraphs });
    const updated = await call('get_document', { deck_id: created.deck_id });
    assert.equal(updated.deck.slides[0].notes, 'Notes');
    for (const extra of [{ expected_revision: 0 }, { unknown: true }]) {
      const result = await client.callTool({ name: 'update_rich_notes', arguments: { deck_id: created.deck_id, expected_revision: 1, slide_id: 'slide-1', paragraphs, ...extra } });
      assert.equal(result.isError, true);
    }
    const design = { width: 720, height: 960, handout_master: { name: 'Handout', background: '@lt1', theme: before.deck.design.theme, elements: [] } };
    await call('update_auxiliary_design', { deck_id: created.deck_id, expected_revision: 1, design });
    assert.equal((await call('get_document', { deck_id: created.deck_id })).deck.auxiliary_design.handout_master.name, 'Handout');
    await call('undo', { deck_id: created.deck_id });
    assert.equal((await call('get_document', { deck_id: created.deck_id })).hash, updated.hash);
  } finally { await client.close(); }
});

test('G23 G25 MCP master fields and themes use strict typed revisioned tools', async () => {
  const client = new Client({ name: 'design-fields-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => { const result = await client.callTool({ name, arguments: args }); assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text); };
  try {
    await client.connect(transport);
    const created = await call('create_presentation', { title: 'Synthetic master fields' });
    const design = await call('design_defaults');
    await call('update_design', { deck_id: created.deck_id, expected_revision: 0, design });
    const before = await call('get_document', { deck_id: created.deck_id });
    const field = { master_id: 'master-1', kind: 'slide_number', reference_date: '2026-09-18' };
    assert.equal((await client.callTool({ name: 'set_design_field', arguments: { deck_id: created.deck_id, expected_revision: before.revision, field: { ...field, unknown: true } } })).isError, true);
    await call('set_design_field', { deck_id: created.deck_id, expected_revision: before.revision, field });
    const updated = await call('get_document', { deck_id: created.deck_id });
    assert.ok(updated.deck.slides[0].elements.some(element => element.format?.paragraphs?.some(paragraph => paragraph.runs.some(run => run.field?.kind === 'slidenum'))));
    assert.equal((await client.callTool({ name: 'set_master_theme', arguments: { deck_id: created.deck_id, expected_revision: before.revision, master_id: 'master-1', theme: null } })).isError, true);
    await call('undo', { deck_id: created.deck_id });
    assert.equal((await call('get_document', { deck_id: created.deck_id })).hash, before.hash);
  } finally { await client.close(); }
});

test('MCP cell-path helpers are strict read-only candidates and cell batches are atomic and undoable', async () => {
  const client = new Client({ name: 'cell-path-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full'], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text);
  };
  const table = { type: 'table', id: 'table', x: 40, y: 80, width: 600, height: 200, font_size: 20, rows: [['A', ''], ['', '']], format: { merges: [{ row: 0, column: 0, row_span: 1, col_span: 2 }], cells: [{ row: 0, column: 0, style: { fill: '@accent2', text_format: { paragraphs: [{ runs: [{ text: 'A', style: { italic: true } }] }] } } }] } };
  const polygon = { type: 'polygon', id: 'polygon', x: 40, y: 320, width: 300, height: 200, points: [[0, 0], [1, 0], [0, 1]], fill: '@accent1', stroke: '@dk1', stroke_width: 1, visual: { opacity: 0.5 } };
  const path = { commands: [{ op: 'move', point: [0, 0] }, { op: 'quadratic', control: [0.5, 0], point: [1, 1] }, { op: 'line', point: [0, 1] }, { op: 'close' }] };
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    for (const name of ['set_table_cell_text', 'edit_vector']) assert.equal(tools.find(tool => tool.name === name)?.annotations?.readOnlyHint, true);
    const created = await call('create_presentation', { title: 'Cell path test' });
    await call('apply_transaction', { deck_id: created.deck_id, expected_revision: 0, operations: [{ op: 'add', path: '/deck/slides/0/elements/-', value: table }, { op: 'add', path: '/deck/slides/0/elements/-', value: polygon }] });
    const before = await call('get_document', { deck_id: created.deck_id });
    const cell = await call('set_table_cell_text', { element: table, row: 0, column: 0, text: 'Changed' });
    assert.equal(cell.rows[0][0], 'Changed');
    assert.equal(cell.format.cells[0].style.text_format.paragraphs[0].runs[0].style.italic, true);
    const vector = await call('edit_vector', { element: polygon, path });
    assert.deepEqual(vector.visual, { ...polygon.visual, path });
    assert.deepEqual(vector.points, [[0, 0], [0.5, 0], [1, 1], [0, 1]]);
    assert.deepEqual(await call('get_document', { deck_id: created.deck_id }), before);
    for (const [name, args] of [
      ['edit_vector', { element: { ...polygon, unknown: true }, path }],
      ['edit_vector', { element: polygon, path: { ...path, unknown: true } }],
      ['edit_vector', { element: polygon, path, unknown: true }],
      ['set_table_cell_text', { element: table, row: 0, column: 0, text: 'X', unknown: true }],
      ['edit_table', { deck_id: created.deck_id, expected_revision: 1, slide_id: 'slide-1', id: 'table', operations: [{ op: 'set_cell_text', row: 0, column: 0, text: 'Changed' }, { op: 'set_cell_text', row: 0, column: 1, text: 'Follower' }] }],
      ['edit_table', { deck_id: created.deck_id, expected_revision: 1, slide_id: 'slide-1', id: 'table', operations: [{ op: 'set_cell_text', row: 0, column: 0, text: 'Changed', unknown: true }] }],
    ]) assert.equal((await client.callTool({ name, arguments: args })).isError, true, name);
    assert.deepEqual(await call('get_document', { deck_id: created.deck_id }), before);
    await call('edit_table', { deck_id: created.deck_id, expected_revision: 1, slide_id: 'slide-1', id: 'table', operations: [{ op: 'set_cell_text', row: 0, column: 0, text: 'Changed' }] });
    assert.equal((await call('get_document', { deck_id: created.deck_id })).deck.slides[0].elements[0].rows[0][0], 'Changed');
    await call('undo', { deck_id: created.deck_id });
    assert.equal((await call('get_document', { deck_id: created.deck_id })).hash, before.hash);
  } finally { await client.close(); }
});

test('MCP static export preflights every output and verifies recovery without replacing decks', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-static-mcp-'));
  const client = new Client({ name: 'static-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full', '--output-dir', directory], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    assert.ok(tools.some(tool => tool.name === 'export_static'));
    const created = await call('create_presentation', { title: 'Static MCP' });
    await call('edit_slides', { deck_id: created.deck_id, expected_revision: 0, operations: [{ op: 'duplicate', slide_id: 'slide-1', id: 'second' }] });
    const before = await call('get_document', { deck_id: created.deck_id });
    const request = { deck_id: created.deck_id, filename: 'pages.png', options: { format: 'png', page_indices: [1, 0], scale: 0.5 } };
    await writeFile(join(directory, 'pages-page-001.png'), 'existing', { flag: 'wx' });
    assert.equal((await client.callTool({ name: 'export_static', arguments: request })).isError, true);
    assert.deepEqual(await readdir(directory), ['pages-page-001.png']);
    const exported = await call('export_static', { ...request, filename: 'new.png' });
    assert.deepEqual(exported.files.map(file => file.page_indices), [[1], [0]]);
    for (const file of exported.files) assert.equal((await readFile(file.path)).subarray(1, 4).toString(), 'PNG');
    const pdf = await call('export_static', { ...request, filename: 'new.pdf', options: { format: 'pdf' } });
    assert.equal(pdf.files.length, 1); assert.equal(pdf.pdf_text_outlined, true);
    assert.equal(pdf.pdf_searchable_text, true); assert.equal(pdf.pdf_selectable_text, true);
    assert.equal(pdf.pdf_tagged, true); assert.equal(pdf.pdf_semantic_overlay, true);
    assert.equal(pdf.pdf_editable_text, false); assert.equal(pdf.pdf_ua_certified, false);
    assert.equal((await readFile(pdf.files[0].path)).subarray(0, 5).toString(), '%PDF-');
    for (const filename of ['wrong.jpg', '../bad.png', 'CON.png']) {
      assert.equal((await client.callTool({ name: 'export_static', arguments: { ...request, filename } })).isError, true);
    }
    assert.equal((await client.callTool({ name: 'export_static', arguments: { ...request, options: { format: 'png', shell: true } } })).isError, true);
    const verified = await call('verify_recovery', { document_json: JSON.stringify(before) });
    assert.deepEqual(verified, before);
    const recovered = await call('recover_presentation', { document_json: JSON.stringify(before) });
    assert.notEqual(recovered.deck_id, created.deck_id);
    assert.equal((await client.callTool({ name: 'undo', arguments: { deck_id: recovered.deck_id } })).isError, true);
    assert.equal((await client.callTool({ name: 'verify_recovery', arguments: { document_json: JSON.stringify({ ...before, hash: '0'.repeat(64) }) } })).isError, true);
    assert.deepEqual(await call('get_document', { deck_id: created.deck_id }), before);
    assert.equal(await readFile(join(directory, 'pages-page-001.png'), 'utf8'), 'existing');
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});

test('MCP generates, edits and saves with the GUI closed', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-mcp-'));
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full', '--output-dir', directory], stderr: 'pipe', env: coreEnvironment });
  const client = new Client({ name: 'aislide-test', version: '1.0.0' });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content));
    return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const tools = await client.listTools();
    assert.ok(tools.tools.some((tool) => tool.name === 'compile_report'));
    const report = await call('sample_report');
    const result = await call('compile_report', { report });
    assert.equal(result.slides, 12);
    await call('update_text', { deck_id: result.deck_id, slide_id: 'slide-1', element_id: 'title', text: 'MCP edited title' });
    const deck = await call('get_deck', { deck_id: result.deck_id });
    assert.equal(deck.slides[0].elements.find((element) => element.id === 'title').text, 'MCP edited title');
    await call('export_pptx', { deck_id: result.deck_id, filename: 'test-report.pptx' });
    const bytes = await readFile(join(directory, 'test-report.pptx'));
    assert.equal(bytes.subarray(0, 2).toString(), 'PK');
    const duplicate = await client.callTool({ name: 'export_pptx', arguments: { deck_id: result.deck_id, filename: 'test-report.pptx' } });
    assert.equal(duplicate.isError, true);
    assert.deepEqual(await readFile(join(directory, 'test-report.pptx')), bytes);
    const traversal = await client.callTool({ name: 'export_pptx', arguments: { deck_id: result.deck_id, filename: '../escape.pptx' } });
    assert.equal(traversal.isError, true);
  } finally {
    await client.close();
    await rm(directory, { recursive: true, force: true });
  }
});