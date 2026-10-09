import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile, readdir, symlink, truncate, mkdir, rename } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory.js';
import { guidedExamples } from './guided-demo.mjs';
import { registerHooks } from 'node:module';
import { randomUUID, createHash } from 'node:crypto';
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
      if (request.op === 'inspect_raster') {
        const bytes = Buffer.from(request.base64, 'base64');
        return { width: 1, height: 1, mime_type: request.mime_type, sha256: createHash('sha256').update(bytes).digest('hex'), byte_length: bytes.length };
      }
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

test('issue22 MCP discovers bounded metadata detach without raw patch paths', async () => {
  await feedbackMcpFixture(async ({ registrations, call, calls, fixture }) => {
    const tool = registrations.get('detach_part');
    assert.ok(tool);
    assert.equal(tool.config.annotations.readOnlyHint, false);
    assert.match(tool.config.description, /native|geometry/i);
    const found = await call('discover_tools', { query: 'detach_part' });
    assert.ok(JSON.stringify(found).includes('detach_part'));
    const created = await call('create_presentation', { title: 'Synthetic detach' });
    const input = { deck_id: created.deck_id, expected_revision: 0, expected_hash: 'a'.repeat(64), slide_id: 'slide-1', id: 'managed' };
    for (const invalid of [{ ...input, id: 'x'.repeat(41) }, { ...input, path: '/parts/0' }, { ...input, document: {} }]) {
      assert.equal(tool.config.inputSchema.safeParse(invalid).success, false);
    }
    fixture.onRequest = async request => ({ document: { ...request.document, revision: 1, hash: 'b'.repeat(64) }, receipt: { inverse: [] } });
    const detached = await call('detach_part', input);
    assert.equal(calls.at(-1).request.op, 'detach_part');
    assert.equal(calls.at(-1).request.slide_id, input.slide_id);
    assert.equal(calls.at(-1).request.id, input.id);
    assert.equal(detached.revision, 1);
    assert.equal(detached.metadata_detached, true);
    assert.equal(detached.native_figures_retained, true);
  });
});

test('issue16 MCP returns creation effect warnings outside graph diagnostics', async () => {
  await feedbackMcpFixture(async ({ call, fixture }) => {
    const created = await call('create_presentation', { title: 'Synthetic effects' });
    const warning = { code: 'EFFECT_APPROXIMATION', page_index: 0, element_id: 'soft-0', message: 'Synthetic budget warning' };
    fixture.onRequest = async request => ({ document: { ...request.document, revision: request.document.revision + 1, hash: 'b'.repeat(64) }, receipt: { inverse: [] }, render_warnings: [warning] });
    const added = await call('add_elements', { deck_id: created.deck_id, expected_revision: 0, slide_id: 'slide-1', elements: [{ type: 'text', id: 'soft-0', x: 20, y: 20, width: 200, height: 100, text: 'Synthetic', font_size: 24, color: '000000', bold: false }] });
    assert.deepEqual(added.renderWarnings, { revision: 1, hash: 'b'.repeat(64), status: 'complete', warnings: [warning] });
    assert.equal(added.graphDiagnostics, undefined);
    const changed = await call('apply_operations', { deck_id: created.deck_id, expected_revision: 1, expected_hash: 'b'.repeat(64), operations: [{ op: 'set_slide_background', slide_id: 'slide-1', color: 'FFFFFF' }] });
    assert.equal(changed.renderWarnings.revision, 2);
    const resized = await call('set_frames', { deck_id: created.deck_id, expected_revision: 2, slide_id: 'slide-1', frames: [{ id: 'soft-0', frame: { x: 20, y: 20, width: 640, height: 390 } }] });
    assert.equal(resized.renderWarnings.revision, 3);
  });
});

test('issue18 MCP exposes bounded template inspection and selection', async () => {
  await feedbackMcpFixture(async ({ call, calls, registrations, fixture }) => {
    assert.ok(registrations.has('inspect_template'));
    const schema = registrations.get('import_template').config.inputSchema;
    const selection = { include_sample_slides: false, layout_ids: ['ppt/slideLayouts/slideLayout1.xml'], strip_sections: true, source_sha256: 'c'.repeat(64) };
    assert.equal(schema.safeParse({ kind: 'potx', base64: 'UEs=', options: selection }).success, true);
    for (const options of [{ unknown: true }, { layout_ids: Array(33).fill('layout') }, { master_names: Array(9).fill('Master') }, { source_sha256: 'not-a-hash' }, { include_sample_slides: 'false' }]) {
      assert.equal(schema.safeParse({ kind: 'potx', base64: 'UEs=', options }).success, false);
    }
    fixture.onRequest = async request => {
      if (request.op === 'inspect_template') return { source_sha256: 'c'.repeat(64), layouts: [{ id: selection.layout_ids[0], name: 'Blank', master_id: 'master' }], masters: [{ id: 'master', name: 'Master' }] };
      assert.equal(request.op, 'import_template');
      return { version: 1, id: request.id, revision: 0, hash: 'a'.repeat(64), sources: [], bindings: [], parts: [], deck: { version: 1, title: 'Selected template', width: 1280, height: 720, slides: [{ id: 'slide-1', title: '', background: 'FFFFFF', elements: [], notes: '' }] } };
    };
    const inspected = await call('inspect_template', { kind: 'potx', base64: 'UEs=', capacity_profile: 'standard' });
    assert.equal(inspected.source_sha256, selection.source_sha256);
    assert.equal(calls.at(-1).request.capacity_profile, 'standard');
    await call('import_template', { kind: 'potx', base64: 'UEs=', options: selection });
    assert.deepEqual(calls.at(-1).request.options, selection);
  });
});

test('reference MCP registers guarded publication with default-deny entries', async () => {
  await feedbackMcpFixture(async ({ registrations, call, calls, fixture }) => {
    const tool = registrations.get('set_references');
    assert.ok(tool);
    assert.equal(tool.config.annotations.readOnlyHint, false);
    const input = { deck_id: randomUUID(), expected_revision: 0, expected_hash: 'a'.repeat(64), options: { placement: 'auto', entries: [{ id: 'learn', name: 'Microsoft Learn', url: 'https://learn.microsoft.com/azure/', slide_ids: ['slide-1'] }] } };
    const parsed = tool.config.inputSchema.parse(input);
    assert.notEqual(parsed.options.entries[0].publish, true);
    for (const placement of ['auto', 'footnotes', 'appendix', 'appendix_only']) assert.equal(tool.config.inputSchema.safeParse({ ...input, options: { ...input.options, placement } }).success, true, placement);
    assert.match(tool.config.description, /appendix_only/);
    assert.match(tool.config.description, /page numbers/i);
    assert.match(tool.config.description, /bottom quarter/i);
    assert.equal(tool.config.inputSchema.safeParse({ ...input, options: { ...input.options, unknown: true } }).success, false);
    assert.equal(tool.config.inputSchema.safeParse({ ...input, options: { ...input.options, placement: 'automatic_appendix_only' } }).success, false);
    assert.equal(tool.config.inputSchema.safeParse({ ...input, options: { ...input.options, entries: [{ ...input.options.entries[0], caption: 'Private title' }] } }).success, false);
    input.options.placement = 'appendix_only';
    input.deck_id = (await call('create_presentation', { title: 'Synthetic reference test' })).deck_id;
    fixture.onRequest = async request => ({ document: { ...request.document, revision: 1, hash: 'b'.repeat(64) }, receipt: { inverse: [] }, publication: { supplied: 1, published: 0, excluded: 1 } });
    const result = await call('set_references', input);
    assert.equal(calls.at(-1).request.op, 'set_references');
    assert.equal(calls.at(-1).request.expected_hash, input.expected_hash);
    assert.deepEqual(calls.at(-1).request.options, input.options);
    assert.equal(calls.length, 2);
    assert.equal(result.revision, 1);
    assert.deepEqual(result.publication, { supplied: 1, published: 0, excluded: 1 });
    assert.doesNotMatch(JSON.stringify(result), /learn\.microsoft/);
  }, []);
});

test('layout_graph MCP uses strict coordinate-free input and one pure core request', async () => {
  await feedbackMcpFixture(async ({ registrations, calls, call, fixture }) => {
    const tool = registrations.get('layout_graph');
    assert.ok(tool, 'Missing layout_graph tool');
    assert.equal(tool.config.annotations.readOnlyHint, true);
    assert.equal(tool.config.annotations.openWorldHint, false);
    assert.match(tool.config.description, /grid.*not.*hierarchical/i);
    assert.match(tool.config.description, /add_graph/);
    const input = { version: 1, title: 'Synthetic grid', columns: 2, nodes: [{ id: 'first', label: 'First', width: 200, height: 96, fill: '@accent2', detail: 'Detail', text_align: 'left', heading_bold: false }, { id: 'second', label: 'Second' }], edges: [{ id: 'link', source: 'first', target: 'second', label: 'Relationship', route: 'elbow', dashed: true }] };
    const schema = tool.config.inputSchema;
    assert.deepEqual(schema.parse({ input }), { input });
    for (const change of [
      { columns: 0 }, { columns: 9 }, { columns: 1.5 }, { columns: null },
      { version: 2 }, { groups: [] }, { layout: 'hierarchical' },
      { nodes: [] }, { nodes: Array(49).fill(input.nodes[0]) }, { edges: Array(65).fill(input.edges[0]) },
      ...['x', 'y', 'group', 'parent', 'children', 'unknown'].map(field => ({ nodes: [{ ...input.nodes[0], [field]: 0 }] })),
      { nodes: [{ id: 'bad/id', label: 'Invalid' }] }, { nodes: [{ id: 'node', label: 'x'.repeat(161) }] },
      { nodes: [{ id: 'node', label: 'Invalid', width: 63 }] }, { nodes: [{ id: 'node', label: 'Invalid', height: 39 }] },
      { edges: [{ ...input.edges[0], unknown: true }] },
    ]) assert.equal(schema.safeParse({ input: { ...input, ...change } }).success, false, JSON.stringify(change).slice(0, 100));
    assert.equal(schema.safeParse({ input, unknown: true }).success, false);
    const minimal = { version: 1, title: 'Defaults', nodes: [{ id: 'only', label: 'Only' }] };
    assert.equal(schema.safeParse({ input: minimal }).success, true);
    assert.equal(registrations.get('create_graph').config.inputSchema.safeParse({ id: 'graph', spec: minimal }).success, false);
    const canonical = { version: 1, title: input.title, nodes: input.nodes.map((node, index) => ({ ...node, x: 180 + index * 576, y: 220 })), edges: input.edges, groups: [] };
    fixture.onRequest = async request => { assert.deepEqual(request, { op: 'layout_graph', input }); return canonical; };
    const controller = new AbortController();
    assert.deepEqual(await call('layout_graph', { input }, controller.signal), canonical);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].signal, controller.signal);
    assert.equal((await call('list_decks')).decks.length, 0);
  });
});

test('layout_graph MCP transport discovers an advanced strict schema and forwards canonical output', async () => {
  await feedbackMcpFixture(async ({ fixture, calls }) => {
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const client = new Client({ name: 'layout-graph-regression', version: '1.0.0' });
    try {
      await McpServer.prototype.connect.call(fixture.instance, serverTransport);
      await client.connect(clientTransport);
      const invoke = (name, input) => client.callTool({ name, arguments: input });
      assert.equal((await client.listTools()).tools.some(tool => tool.name === 'layout_graph'), false);
      const discovered = JSON.parse((await invoke('discover_tools', { query: 'layout_graph' })).content[0].text);
      assert.deepEqual(discovered.tools.map(tool => [tool.name, tool.loaded, tool.read_only]), [['layout_graph', false, true]]);
      const detailed = JSON.parse((await invoke('get_tool_schema', { name: 'layout_graph' })).content[0].text);
      const listed = (await client.listTools()).tools.find(tool => tool.name === 'layout_graph');
      assert.deepEqual(listed.inputSchema, detailed.inputSchema);
      const root = detailed.inputSchema;
      const inputSchema = resolveSchemaRef(root, root.properties.input);
      const nodeSchema = resolveSchemaRef(root, resolveSchemaRef(root, inputSchema.properties.nodes).items);
      assert.equal(root.additionalProperties, false);
      assert.equal(inputSchema.additionalProperties, false);
      assert.equal(nodeSchema.additionalProperties, false);
      assert.deepEqual(nodeSchema.required, ['id', 'label']);
      for (const field of ['x', 'y', 'group']) assert.equal(Object.hasOwn(nodeSchema.properties, field), false);
      assert.equal(Object.hasOwn(inputSchema.properties, 'groups'), false);
      assert.equal(resolveSchemaRef(root, inputSchema.properties.nodes).maxItems, 48);
      assert.equal(resolveSchemaRef(root, inputSchema.properties.edges).maxItems, 64);
      assert.equal(resolveSchemaRef(root, inputSchema.properties.columns).maximum, 8);
      const input = { version: 1, title: 'Transport grid', nodes: [{ id: 'only', label: 'Only' }] };
      for (const argumentsValue of [{ input, extra: true }, { input: { ...input, groups: [] } }, { input: { ...input, nodes: [{ ...input.nodes[0], x: 0 }] } }, { input: { ...input, columns: 9 } }]) {
        const response = await invoke('layout_graph', argumentsValue);
        assert.equal(response.isError, true);
      }
      assert.equal(calls.length, 0);
      const canonical = { version: 1, title: input.title, subtitle: '', nodes: [{ ...input.nodes[0], x: 488, y: 260, width: 176, height: 80 }], edges: [], groups: [] };
      fixture.onRequest = async request => { assert.deepEqual(request, { op: 'layout_graph', input }); return canonical; };
      const response = await invoke('layout_graph', { input });
      assert.equal(response.isError, undefined);
      assert.deepEqual(JSON.parse(response.content[0].text), canonical);
      assert.equal(response._meta.aislide_timing.core_calls, 1);
      assert.equal(calls.length, 1);
      fixture.onRequest = async () => { throw new Error('grid does not fit existing node sizes'); };
      const failed = await invoke('layout_graph', { input });
      assert.equal(failed.isError, true);
      assert.match(failed.content[0].text, /grid does not fit existing node sizes/);
      assert.equal(calls.length, 2);
      assert.deepEqual(JSON.parse((await invoke('list_decks', {})).content[0].text).decks, []);
    } finally { await client.close(); }
  }, []);
});

test('comparison panels MCP keeps strict paired rows and expands reusable image handles', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-comparison-panels-'));
  try {
    await writeFile(join(directory, 'indicator.png'), 'Synthetic raster fixture');
    await feedbackMcpFixture(async ({ call, calls, registrations }) => {
      const asset = await call('register_asset', { path: 'indicator.png' });
      const created = await call('create_presentation', { title: 'Synthetic comparison' });
      const spec = { version: 1, preset: 'contrast/panels', title: '', data: { kind: 'comparison_panels', transition: false, panels: [{ label: 'Before', items: [{ text: 'Separate signals', icon: { asset_id: asset.asset_id } }] }, { label: 'After', items: [{ text: 'Shared signals', icon: { asset_id: asset.asset_id } }] }] } };
      const input = { deck_id: created.deck_id, expected_revision: 0, expected_hash: 'a'.repeat(64), operations: [{ op: 'add_part', slide_id: 'slide-1', id: 'comparison', spec }] };
      await call('apply_operations', input);
      const operation = calls.at(-1).request.operations[0];
      assert.equal(operation.spec.data.panels[0].items[0].icon.mime_type, 'image/png');
      assert.equal(operation.spec.data.panels[0].items[0].icon.base64, Buffer.from('Synthetic raster fixture').toString('base64'));
      assert.equal(operation.spec.data.panels[0].items[0].icon.asset_id, undefined);
      const schema = registrations.get('add_part').config.inputSchema;
      const individual = { deck_id: created.deck_id, expected_revision: 1, slide_id: 'slide-1', id: 'comparison', spec };
      for (const panels of [spec.data.panels.slice(0, 1), [...spec.data.panels, spec.data.panels[0]], [{ ...spec.data.panels[0], items: [] }, spec.data.panels[1]], [{ ...spec.data.panels[0], items: [...spec.data.panels[0].items, ...spec.data.panels[0].items] }, spec.data.panels[1]], [{ ...spec.data.panels[0], body_size: 8 }, spec.data.panels[1]], [{ ...spec.data.panels[0], invented: true }, spec.data.panels[1]]]) assert.equal(schema.safeParse({ ...individual, spec: { ...spec, data: { ...spec.data, panels } } }).success, false);
    }, ['--tool-profile', 'full', '--asset-dir', directory]);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('briefing parts MCP keeps strict data and Lucide assets return reusable icon objects', async () => {
  await feedbackMcpFixture(async ({ call, calls, registrations, fixture }) => {
    const found = await call('lucide_icons', { query: 'shield check' });
    assert.equal(found.license, 'ISC');
    assert.ok(found.icons.some(icon => icon.name === 'ShieldCheck'));
    const security = await call('lucide_icons', { category: 'security', limit: 5 });
    assert.ok(security.icons.length > 0 && security.icons.length <= 5 && security.icons.every(icon => icon.categories.includes('security')));
    assert.ok((await call('lucide_icons', {})).categories.includes('security'));
    for (const [query, expected] of [['list-checks', 'ListChecks'], ['list_checks', 'ListChecks'], ['ListChecks', 'ListChecks'], ['message-square', 'MessageSquare'], ['lock', 'Lock'], ['eye', 'Eye'], ['table', 'Table']]) {
      const result = await call('lucide_icons', { query, limit: 1 });
      assert.equal(result.icons[0]?.name, expected, query);
    }
    assert.ok(!(await call('lucide_icons', { query: 'eye', limit: 50 })).icons.some(icon => icon.name === 'BadgeJapaneseYen'));
    assert.equal(calls.length, 0);
    fixture.onRequest = async request => {
      assert.equal(request.op, 'create_graph_icon');
      assert.equal(request.mime_type, 'image/svg+xml');
      const svg = Buffer.from(request.base64, 'base64').toString();
      assert.match(svg, /^<svg[^>]*stroke="#[0-9A-F]{6}"/);
      assert.doesNotMatch(svg, /<script|href=/i);
      return { base64: Buffer.from(`synthetic-png:${createHash('sha256').update(svg).digest('hex')}`).toString('base64'), mime_type: 'image/png', alt: request.alt };
    };
    const prepared = await call('lucide_icon_assets', { icons: [{ name: 'ShieldCheck', color: 'c4314b' }, { name: 'ShieldCheck', color: 'C4314B' }, { name: 'Bot', stroke_width: 2, alt: 'Agent' }] });
    assert.equal(prepared.icons.length, 3);
    assert.deepEqual(Object.keys(prepared.icons[0].icon).sort(), ['alt', 'asset_id', 'mime_type']);
    assert.equal(prepared.icons[0].icon.asset_id, prepared.icons[1].icon.asset_id);
    assert.notEqual(prepared.icons[0].icon.asset_id, prepared.icons[2].icon.asset_id);
    assert.equal(prepared.icons[2].icon.alt, 'Agent');
    assert.equal(prepared.icons[0].icon.alt, 'Shield Check icon');
    const aliases = await call('lucide_icon_assets', { icons: [{ name: 'History' }, { name: 'RotateCcwClock' }, { name: 'Fingerprint' }, { name: 'FingerprintPattern' }] });
    assert.deepEqual(aliases.icons.map(icon => icon.name), ['RotateCcwClock', 'RotateCcwClock', 'FingerprintPattern', 'FingerprintPattern']);
    assert.equal(aliases.icons[0].icon.asset_id, aliases.icons[1].icon.asset_id);
    assert.equal(aliases.icons[2].icon.asset_id, aliases.icons[3].icon.asset_id);
    const assets = registrations.get('lucide_icon_assets');
    const unknown = await assets.callback(assets.config.inputSchema.parse({ icons: [{ name: 'NotALucideIcon' }] }), { signal: new AbortController().signal });
    assert.equal(unknown.isError, true);
    assert.match(unknown.content[0].text, /Unknown Lucide icon/);
    for (const invalid of [{ icons: [] }, { icons: [{ name: 'shield' }] }, { icons: [{ name: 'Bot', color: '@accent1' }] }, { icons: [{ name: 'Bot', stroke_width: 5 }] }, { icons: [{ name: 'Bot', extra: true }] }]) assert.equal(assets.config.inputSchema.safeParse(invalid).success, false);
    fixture.onRequest = undefined;
    const created = await call('create_presentation', { title: 'Synthetic briefing' });
    const icon = prepared.icons[0].icon;
    const spec = { version: 1, preset: 'list-horizontal/icon-cards', title: '', data: { kind: 'icon_cards', numbered: true, message: { text: 'Synthetic message' }, cards: [{ label: 'First', detail: 'Detail', icon }, { label: 'Second', points: ['One', 'Two'], tag: 'Tag' }] }, layout: { x: 48, y: 120, width: 1184, height: 540, show_title: false } };
    await call('apply_operations', { deck_id: created.deck_id, expected_revision: 0, expected_hash: 'a'.repeat(64), operations: [{ op: 'add_part', slide_id: 'slide-1', id: 'cards', spec }] });
    const forwarded = calls.at(-1).request.operations[0].spec.data.cards[0].icon;
    assert.equal(forwarded.asset_id, undefined);
    assert.equal(forwarded.mime_type, 'image/png');
    assert.ok(forwarded.base64.length > 0);
    const schema = registrations.get('add_part').config.inputSchema;
    const input = { deck_id: created.deck_id, expected_revision: 1, slide_id: 'slide-1', id: 'briefing', spec };
    const valid = [
      spec,
      { ...spec, preset: 'list/icon-rows', data: { kind: 'icon_rows', boxed: true, rows: [{ label: 'A', detail: 'Detail', icon }, { label: 'B' }] } },
      { ...spec, preset: 'before-after/shift', data: { kind: 'shift_rows', from_label: 'Today', to_label: 'Next', rows: [{ from: 'A', to: 'B\nC', caption: 'c' }, { from: 'D', to: 'E', detail: 'Detail' }] } },
      { ...spec, preset: 'flow/cards', data: { kind: 'step_cards', step_label: 'PHASE', steps: [{ label: 'A', outcome: 'Result', image: icon }, { label: 'B', points: ['One'] }] } },
      { ...spec, preset: 'list/agenda', data: { kind: 'agenda', items: [{ label: 'A', meta: '10 min' }, { label: 'B', detail: 'Detail' }] } },
      { ...spec, preset: 'list-enumeration/screenshot-callouts', data: { kind: 'screenshot_callouts', image: icon, callouts: [{ x: 0.2, y: 0.4, label: 'Enable', detail: 'Detail' }, { x: 1, y: 0, label: 'Assign' }] } },
      { ...spec, preset: 'flow/open-steps', data: { kind: 'open_steps', steps: [{ label: 'A' }, { label: 'B', detail: 'Explain' }] } },
      { ...spec, preset: 'vertical-flow/rail', data: { kind: 'rail_steps', steps: [{ label: 'A' }, { label: 'B' }], accent: '@accent1' } },
      { ...spec, preset: 'flow/roadmap', data: { kind: 'roadmap', phases: [{ period: 'Now', label: 'A', points: ['One'], outcome: 'Done' }, { period: 'Later', label: 'B' }] } },
      { ...spec, preset: 'list-horizontal/icon-columns', data: { kind: 'icon_columns', items: [{ label: 'A', icon }, { label: 'B' }] } },
      { ...spec, preset: 'list-horizontal/fact-columns', data: { kind: 'fact_columns', items: [{ value: '0.04', unit: '%', label: 'A', qualifier: 'Own denominator' }, { value: 'Slow', label: 'B' }], columns: 2 } },
      { ...spec, preset: 'list-horizontal/image-columns', data: { kind: 'image_columns', items: [{ image: icon, label: 'A', caption: 'Evidence' }, { image: icon, label: 'B' }] } },
    ];
    for (const candidate of valid) assert.equal(schema.safeParse({ ...input, spec: candidate }).success, true, candidate.preset);
    const card = { label: 'Topic' };
    for (const data of [
      { ...spec.data, cards: Array(7).fill(card) }, { ...spec.data, columns: 5 }, { ...spec.data, cards: Array(5).fill(card), columns: 2 }, { ...spec.data, body_size: 30 },
      { ...spec.data, cards: [{ label: ' ' }, card] }, { ...spec.data, cards: [{ ...card, invented: true }, card] }, { ...spec.data, cards: [{ ...card, points: ['1', '2', '3', '4', '5'] }, card] },
      { ...spec.data, message: { text: 'x'.repeat(121) } }, { ...spec.data, cards: [{ label: 'A\r\nB' }, card] },
      { kind: 'agenda', items: [{ label: 'A', meta: 'two\nlines' }, { label: 'B' }] }, { kind: 'step_cards', step_label: 'X'.repeat(13), steps: [{ label: 'A' }, { label: 'B' }] },
      { kind: 'shift_rows', rows: Array(6).fill({ from: 'A', to: 'B' }) }, { kind: 'icon_rows', rows: [{ label: 'A' }] },
      { kind: 'screenshot_callouts', image: icon, callouts: [{ x: 1.2, y: 0.5, label: 'A' }] }, { kind: 'screenshot_callouts', image: icon, callouts: Array(7).fill({ x: 0.5, y: 0.5, label: 'A' }) },
      { kind: 'screenshot_callouts', callouts: [{ x: 0.5, y: 0.5, label: 'A' }] },
      { kind: 'open_steps', steps: [{ label: 'Only' }] },
      { kind: 'rail_steps', steps: Array(6).fill({ label: 'Too many' }) },
      { kind: 'roadmap', phases: [{ period: 'Now\nLater', label: 'A' }, { period: 'Later', label: 'B' }] },
      { kind: 'fact_columns', items: [{ value: '1\n2', label: 'A' }, { value: '3', label: 'B' }] },
      { kind: 'fact_columns', items: Array(6).fill({ value: '1', label: 'A' }), columns: 2 },
      { kind: 'image_columns', items: [{ label: 'A' }, { image: icon, label: 'B' }] },
    ]) assert.equal(schema.safeParse({ ...input, spec: { ...spec, data } }).success, false, JSON.stringify(data).slice(0, 80));
    const business = [
      ['list-horizontal/kpi-cards', { kind: 'kpi_cards', cards: [{ label: 'Revenue', value: '12.4', unit: 'B', delta: '+8%', status: 'good', comparison: 'Plan 12' }], columns: null }],
      ['horizontal-bar-graph/bullet', { kind: 'bullet_graphs', rows: [{ label: 'Margin', actual: 38, target: 40, ranges: [30, 36, 45], unit: '%', lower_is_better: false }] }],
      ['water-fall/variance', { kind: 'variance', total_label: 'Total', unit: 'M', rows: [{ label: 'A', plan: 1, actual: 2 }, { label: 'B', plan: -3, actual: -2 }] }],
      ['matrix/harvey-balls', { kind: 'harvey_matrix', columns: ['Speed', 'Cost'], rows: [{ label: 'A', levels: [0, 4] }, { label: 'B', levels: [2, 3] }], legend: ['0', '1', '2', '3', '4'] }],
      ['matrix/heatmap', { kind: 'heatmap', columns: ['Q1', 'Q2'], rows: [{ label: 'A', values: [1, 2] }, { label: 'B', values: [3, 4] }], unit: '%', midpoint: null }],
      ['matrix/raci', { kind: 'raci', roles: ['PM', 'Dev'], tasks: [{ label: 'Plan', assignments: ['A/R', 'C'] }, { label: 'Build', assignments: ['A', 'R'] }] }],
      ['matrix/risk', { kind: 'risk_matrix', risks: [{ id: 'R1', label: 'Delay', likelihood: 4, impact: 5, action: 'Mitigate' }], zone_labels: ['Critical', 'High', 'Medium', 'Low'] }],
      ['vertical-bar-graph/pareto', { kind: 'pareto', items: [{ label: 'A', value: 5 }, { label: 'B', value: 3 }, { label: 'C', value: 0 }], threshold: 0.8 }],
      ['line-graph/control-chart', { kind: 'control_chart', labels: ['1', '2', '3', '4', '5'], values: [1, 2, 1, 2, 1], upper: null }],
      ['tree/fishbone', { kind: 'fishbone', effect: 'Delay', categories: [{ label: 'People', causes: [{ text: 'Handover', focus: true }] }, { label: 'Process', causes: [{ text: 'Manual entry' }] }] }],
      ['flow/swimlane', { kind: 'swimlane', lanes: ['A', 'B'], steps: [{ id: 's1', label: 'Start', lane: 0, shape: 'event' }, { id: 's2', label: 'Do', lane: 1, column: 1 }], flows: [{ from: 's1', to: 's2', label: 'Go' }, { from: 's2', to: 's1', exception: true }] }],
      ['flow/sankey', { kind: 'sankey', unit: 'k', nodes: [{ id: 'a', label: 'A' }, { id: 'b', label: 'B', color: '@accent2' }], links: [{ from: 'a', to: 'b', value: 3 }] }],
      ['flow/journey', { kind: 'journey', stages: ['One', 'Two'], rows: [{ label: 'Do', cells: ['a', 'b'], boxed: true }], emotions: [1, -1], emotion_notes: ['Good', 'Bad'], highlight: 1 }],
      ['correlation/c4-container', { kind: 'architecture', system: 'Shop', elements: [{ id: 'u', label: 'User', kind: 'person' }, { id: 'w', label: 'Web', kind: 'container', detail: 'UI' }], relations: [{ from: 'u', to: 'w', label: 'Uses' }] }],
    ];
    for (const [preset, data] of business) assert.equal(schema.safeParse({ ...input, spec: { ...spec, preset, data } }).success, true, preset);
    for (const data of [
      { kind: 'kpi_cards', cards: [{ label: 'A', value: '1\n2' }] }, { kind: 'kpi_cards', cards: [{ label: 'A', value: '1', status: 'great' }] },
      { kind: 'bullet_graphs', rows: [{ label: 'A', actual: 1, target: 1, ranges: [0] }] }, { kind: 'bullet_graphs', rows: [{ label: 'A', actual: 1, target: 1, ranges: [1, 2, 3, 4] }] },
      { kind: 'raci', roles: ['PM', 'Dev'], tasks: [{ label: 'Plan', assignments: ['X', 'C'] }, { label: 'Build', assignments: ['A', 'R'] }] },
      { kind: 'risk_matrix', risks: [{ id: 'R100', label: 'Delay', likelihood: 4, impact: 5 }] }, { kind: 'risk_matrix', risks: [{ id: 'R1', label: 'Delay', likelihood: 6, impact: 5 }] },
      { kind: 'pareto', items: [{ label: 'A', value: -1 }, { label: 'B', value: 1 }, { label: 'C', value: 1 }] }, { kind: 'pareto', items: [{ label: 'A', value: 1 }, { label: 'B', value: 1 }, { label: 'C', value: 1 }], threshold: 1 },
      { kind: 'swimlane', lanes: ['A', 'B'], steps: [{ id: 'bad id', label: 'S', lane: 0 }, { id: 's2', label: 'T', lane: 1 }], flows: [{ from: 'bad id', to: 's2' }] },
      { kind: 'swimlane', lanes: ['A', 'B', 'C', 'D', 'E', 'F'], steps: [{ id: 's1', label: 'S', lane: 0 }, { id: 's2', label: 'T', lane: 1 }], flows: [{ from: 's1', to: 's2' }] },
      { kind: 'swimlane', lanes: ['A', 'B'], steps: [{ id: 's1', label: 'S', lane: 0, column: 7 }, { id: 's2', label: 'T', lane: 1 }], flows: [{ from: 's1', to: 's2' }] },
      { kind: 'control_chart', labels: Array.from({ length: 33 }, (_, index) => `W${index}`), values: Array.from({ length: 33 }, () => 1) },
      { kind: 'sankey', nodes: [{ id: 'a', label: 'A' }, { id: 'b', label: 'B' }], links: [{ from: 'a', to: 'b', value: 0 }] },
      { kind: 'journey', stages: ['One', 'Two'], rows: [{ label: 'Do', cells: ['a', 'b'] }], emotions: [3, 0] },
      { kind: 'architecture', system: 'Shop', elements: [{ id: 'u', label: 'User', kind: 'robot' }, { id: 'w', label: 'Web', kind: 'container' }], relations: [{ from: 'u', to: 'w' }] },
      { kind: 'heatmap', columns: ['Q1', 'Q2'], rows: [{ label: 'A', values: [1, 2], extra: 1 }, { label: 'B', values: [3, 4] }] },
    ]) assert.equal(schema.safeParse({ ...input, spec: { ...spec, data } }).success, false, JSON.stringify(data).slice(0, 80));
  });
});

test('semantic authoring MCP forwards composition and single-source text with strict schemas', async () => {
  await feedbackMcpFixture(async ({ call, calls, registrations }) => {
    const created = await call('create_presentation', { title: 'Synthetic semantic input' });
    const spec = { title: 'Synthetic', footer: 'Example', blocks: [{ kind: 'cards', items: [{ label: 'First', detail: 'Detail' }, { label: 'Second' }] }] };
    await call('compose_slide', { deck_id: created.deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'content', spec });
    assert.deepEqual(calls.at(-1).request.operations, [{ op: 'compose_slide', slide_id: 'slide-1', id: 'content', spec }]);
    const paragraphs = [{ runs: [{ text: 'Single source', style: { bold: true, alternative_language: 'ja-JP' } }] }];
    await call('set_rich_text', { deck_id: created.deck_id, expected_revision: 1, slide_id: 'slide-1', id: 'content-b0-c0', paragraphs });
    assert.deepEqual(calls.at(-1).request.operations, [{ op: 'set_rich_text', slide_id: 'slide-1', id: 'content-b0-c0', paragraphs }]);
    const schema = registrations.get('compose_slide').config.inputSchema;
    const input = { deck_id: created.deck_id, expected_revision: 2, slide_id: 'slide-1', id: 'content', spec };
    for (const invalid of [{ ...spec, unknown: true }, { ...spec, blocks: [] }, { ...spec, style: { padding: 0 } }, { ...spec, blocks: [{ kind: 'cards', items: [{ label: 'First' }], x: 10 }] }]) assert.equal(schema.safeParse({ ...input, spec: invalid }).success, false);
    assert.equal(registrations.get('set_rich_text').config.inputSchema.safeParse({ deck_id: created.deck_id, expected_revision: 2, slide_id: 'slide-1', id: 'text', text: 'Duplicate source', paragraphs }).success, false);
    for (const language of ['', 'ja_JP', 'a'.repeat(65)]) {
      assert.equal(registrations.get('set_rich_text').config.inputSchema.safeParse({ deck_id: created.deck_id, expected_revision: 2, slide_id: 'slide-1', id: 'text', paragraphs: [{ runs: [{ text: 'Invalid', style: { alternative_language: language } }] }] }).success, false);
    }
  });
});

test('semantic authoring MCP forwards pattern compositions, design tokens and preset report options', async () => {
  await feedbackMcpFixture(async ({ call, calls, registrations, fixture }) => {
    const created = await call('create_presentation', { title: 'Synthetic patterns', setup: { design_preset: 'trust' } });
    const spec = { title: '\u4e09\u3064\u306e\u67f1', footer: 'Synthetic', pattern: { id: 'columns/3' }, slots: {
      'column-1': { kind: 'cards', items: [{ label: 'Clarity', detail: 'One message' }] },
      'column-2': { kind: 'metric', value: '42%', label: 'Synthetic share' },
      'column-3': { kind: 'text', paragraphs: [{ runs: [{ text: '\u5408\u6210\u30c7\u30fc\u30bf' }] }] },
    } };
    await call('compose_slide', { deck_id: created.deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'pattern', spec });
    assert.deepEqual(calls.at(-1).request.operations, [{ op: 'compose_slide', slide_id: 'slide-1', id: 'pattern', spec }]);
    const schema = registrations.get('compose_slide').config.inputSchema;
    const input = { deck_id: created.deck_id, expected_revision: 1, slide_id: 'slide-1', id: 'pattern', spec };
    for (const valid of [
      { title: 'Quote', pattern: { id: 'text/quote' }, slots: { quote: { kind: 'quote', text: 'Synthetic words.', attribution: 'Synthetic source' } } },
      { title: 'Flow', pattern: { id: 'sequence/steps-h', count: 2, message_band: true }, slots: { 'step-1': { kind: 'cards', items: [{ label: 'A' }] }, 'step-2': { kind: 'cards', items: [{ label: 'B' }] }, message: { kind: 'label', text: 'Synthetic' } } },
      { title: 'Data', pattern: { id: 'compare/table-full' }, slots: { table: { kind: 'table', rows: [['A', 'B'], ['1', '2']] }, decision: { kind: 'callout', text: 'Synthetic' } } },
      { title: 'Chart', pattern: { id: 'focus/full' }, slots: { primary: { kind: 'chart', chart: { kind: 'column', categories: ['A'], series: [{ name: 'Synthetic', values: [1], color: '@accent1' }] } } } },
      { title: 'Image', pattern: { id: 'focus/image-caption' }, slots: { image: { kind: 'image', asset_id: randomUUID(), alt: 'Synthetic' }, caption: { kind: 'label', text: 'Synthetic' } } },
    ]) assert.equal(schema.safeParse({ ...input, spec: valid }).success, true, JSON.stringify(valid).slice(0, 120));
    for (const invalid of [
      { ...spec, blocks: [{ kind: 'callout', text: 'Both modes' }] },
      { title: 'No slots', pattern: { id: 'columns/3' } },
      { title: 'No pattern', slots: spec.slots },
      { title: 'Legacy', blocks: [{ kind: 'statement', text: 'Pattern-only block' }] },
      { ...spec, slots: { 'Column 1': spec.slots['column-1'] } },
      { ...spec, slots: { image: { kind: 'image', base64: 'AAAA', mime_type: 'image/png', alt: '' } } },
      { ...spec, slots: { image: { kind: 'image', base64: 'AAAA', mime_type: 'image/png', alt: 'Both', asset_id: randomUUID() } } },
      { ...spec, slots: { metric: { kind: 'metric', value: '', label: 'Empty value' } } },
      { ...spec, pattern: { id: 'columns/3', unknown: true } },
    ]) assert.equal(schema.safeParse({ ...input, spec: invalid }).success, false, JSON.stringify(invalid).slice(0, 120));

    fixture.onRequest = request => request.op === 'design_tokens' ? { version: 1, source: 'preset:trust' } : { ready: true, op: request.op };
    assert.equal((await call('design_tokens', { deck_id: created.deck_id })).source, 'preset:trust');
    assert.equal(calls.at(-1).request.op, 'design_tokens');
    assert.equal(calls.at(-1).request.deck.title, 'Synthetic patterns');
    assert.equal(registrations.get('design_tokens').config.annotations.readOnlyHint, true);

    const generate = registrations.get('generate_report').config.inputSchema;
    assert.equal(generate.safeParse({ prompt: 'Synthetic', slide_count: 3, design_preset: 'trust', language: 'ja' }).success, true);
    for (const invalid of [{ design_preset: 'unknown' }, { language: 'fr' }]) assert.equal(generate.safeParse({ prompt: 'Synthetic', slide_count: 3, ...invalid }).success, false, JSON.stringify(invalid));
    fixture.onRequest = request => {
      if (request.op === 'compile') return { deck: { version: 1, title: 'Synthetic', width: 1280, height: 720, slides: [{ id: 'slide-1', title: 'Synthetic', background: 'FFFFFF', elements: [], notes: '' }] }, issues: [] };
      if (request.op === 'new_document') return { version: 1, id: request.id, revision: 0, hash: 'c'.repeat(64), sources: [], bindings: [], parts: [], deck: request.deck };
      return { ready: true, op: request.op };
    };
    const report = { title: 'Synthetic', subtitle: '', period: '', source: 'Synthetic fixture', sections: [{ title: 'Statement', layout: 'statement', body: ['Synthetic'] }] };
    await call('compile_report', { report, design_preset: 'minimal' });
    assert.deepEqual(calls.filter(entry => entry.request.op === 'compile').at(-1).request.options, { design_preset: 'minimal' });
    await call('compile_report', { report });
    assert.equal('options' in calls.filter(entry => entry.request.op === 'compile').at(-1).request, false, 'the fixed layout sends no options');
    assert.equal(registrations.get('compile_report').config.inputSchema.safeParse({ report, design_preset: 'unknown' }).success, false);
  });
});

test('semantic authoring MCP forwards initial setup in one core request', async () => {
  await feedbackMcpFixture(async ({ call, calls, registrations }) => {
    const setup = { design_preset: 'minimal', font_family: 'Noto Sans CJK JP' };
    const created = await call('create_presentation', { title: 'Synthetic initial setup', setup });
    assert.equal(created.revision, 0);
    assert.equal(calls.length, 1);
    assert.deepEqual(calls[0].request.setup, setup);
    const schema = registrations.get('create_presentation').config.inputSchema;
    for (const invalid of [{ unknown: true }, { font_family: ' ' }, { font_family: 'bad\nfont' }, { design_preset: 'missing' }]) assert.equal(schema.safeParse({ title: 'Synthetic', setup: invalid }).success, false);
  });
});

test('semantic authoring MCP exposes strict text padding in elements and batches', async () => {
  await feedbackMcpFixture(async ({ call, calls, registrations }) => {
    const created = await call('create_presentation', { title: 'Synthetic padding' });
    const padding = { left: 24, right: 24, top: 16, bottom: 16 };
    const input = { deck_id: created.deck_id, expected_revision: 0, expected_hash: 'a'.repeat(64), slide_id: 'slide-1', ids: ['card'], padding };
    await call('set_text_padding', input);
    assert.deepEqual(calls.at(-1).request.operations, [{ op: 'set_text_padding', slide_id: 'slide-1', ids: ['card'], padding }]);
    const schema = registrations.get('set_text_padding').config.inputSchema;
    assert.equal(schema.safeParse({ ...input, padding: null }).success, true);
    assert.equal(schema.safeParse({ ...input, padding: { left: -1 } }).success, false);
    assert.equal(schema.safeParse({ ...input, padding: { left: 16, unknown: true } }).success, false);
    const element = { type: 'text', id: 'text', x: 0, y: 0, width: 300, height: 200, text: 'Synthetic', font_size: 24, color: '@dk1', bold: false, format: { padding } };
    assert.equal(registrations.get('add_elements').config.inputSchema.safeParse({ deck_id: created.deck_id, expected_revision: 1, slide_id: 'slide-1', elements: [element] }).success, true);
  });
});

test('roundtrip feedback MCP rejects process constraints at exact input paths before core execution', async () => {
  await feedbackMcpFixture(async ({ registrations, calls }) => {
    const schema = registrations.get('compile_report').config.inputSchema;
    const report = { title: 'Synthetic limits', subtitle: '', period: '', source: 'Synthetic fixture', sections: [{ title: 'Process', layout: 'process', body: ['First', 'x'.repeat(81)] }] };
    const invalid = schema.safeParse({ report });
    assert.equal(invalid.success, false);
    const issue = invalid.error.issues.find(issue => issue.path.join('.') === 'report.sections.0.body.1');
    assert.ok(issue, JSON.stringify(invalid.error.issues));
    assert.match(issue.message, /actual=81.*limit=80/);
    assert.equal(calls.length, 0);
    for (const scalar of ['x', '\u754c', '\u{1f680}']) {
      report.sections[0].body[1] = scalar.repeat(80);
      assert.equal(schema.safeParse({ report }).success, true);
    }
    report.sections[0].body = ['Only one'];
    const tooFew = schema.safeParse({ report });
    assert.equal(tooFew.success, false);
    assert.ok(tooFew.error.issues.some(issue => issue.path.join('.') === 'report.sections.0.body'));
    report.sections[0].layout = 'columns';
    report.sections[0].body = ['\u{1f680}'.repeat(240)];
    assert.equal(schema.safeParse({ report }).success, true);
    report.sections[0].body[0] += 'x';
    assert.equal(schema.safeParse({ report }).success, false);
    const section = z.toJSONSchema(schema, { io: 'input' }).properties.report.properties.sections.items;
    assert.equal(section.properties.body.items.maxLength, 240);
    const processRule = section.allOf.find(rule => rule.if?.properties?.layout?.const === 'process');
    assert.equal(processRule.then.properties.body.minItems, 2);
    assert.equal(processRule.then.properties.body.items.maxLength, 80);
  });
});

test('roundtrip feedback MCP sends notes headers and accessibility in one guarded batch', async () => {
  await feedbackMcpFixture(async ({ call, calls, registrations }) => {
    const { deck_id } = await call('create_presentation', { title: 'Synthetic metadata batch' });
    const operations = Array.from({ length: 25 }, (_, index) => ({ op: 'update_notes', slide_id: `slide-${index + 1}`, notes: `Synthetic notes ${index + 1}` }));
    operations.push(...Array.from({ length: 7 }, (_, index) => ({ op: 'set_table_headers', slide_id: `slide-${index + 1}`, element_id: 'table', policy: 'first_row' })));
    operations.push(...Array.from({ length: 5 }, (_, index) => ({ op: 'set_accessibility', slide_id: `slide-${index + 1}`, element_id: 'picture', metadata: { description: `Synthetic alternative ${index + 1}`, decorative: false } })));
    const before = calls.length;
    const result = await call('apply_operations', { deck_id, expected_revision: 0, expected_hash: 'a'.repeat(64), operations });
    assert.equal(calls.length, before + 1);
    assert.deepEqual(calls.at(-1).request.operations, operations);
    assert.equal(result.revision, 1);
    assert.equal(result.can_undo, true);
    const schema = registrations.get('apply_operations').config.inputSchema;
    for (const operation of [
      { op: 'update_notes', slide_id: 'slide-1', notes: 'x'.repeat(8001) },
      { op: 'set_table_headers', slide_id: 'slide-1', element_id: 'table', policy: 'all' },
      { op: 'set_accessibility', slide_id: 'slide-1', element_id: 'picture' },
      { op: 'set_accessibility', slide_id: 'slide-1', element_id: 'picture', metadata: { unknown: true } },
    ]) assert.equal(schema.safeParse({ deck_id, expected_revision: 1, expected_hash: result.hash, operations: [operation] }).success, false);
    assert.equal(schema.safeParse({ deck_id, expected_revision: 1, expected_hash: result.hash, operations: [{ op: 'update_notes', slide_id: 'slide-1', notes: '\u{1f680}'.repeat(8000) }, { op: 'set_accessibility', slide_id: 'slide-1', element_id: 'picture', metadata: null }] }).success, true);
    for (const name of ['update_notes', 'set_table_headers', 'set_accessibility']) assert.ok(registrations.has(name));
  });
});

test('roundtrip feedback MCP timings count core calls without duplicating document content', async () => {
  await feedbackMcpFixture(async ({ registrations, fixture, calls }) => {
    const invoke = (name, input) => {
      const tool = registrations.get(name);
      return tool.callback(tool.config.inputSchema.parse(input), { signal: new AbortController().signal });
    };
    const created = await invoke('create_presentation', { title: 'Synthetic timed operation' });
    const timings = created._meta.aislide_timing;
    assert.equal(timings.core_calls, 1);
    assert.ok(timings.core_roundtrip_ms >= 0);
    assert.ok(timings.handler_elapsed_ms >= timings.core_roundtrip_ms);
    assert.deepEqual(Object.keys(timings).sort(), ['core_calls', 'core_roundtrip_ms', 'handler_elapsed_ms']);
    const { deck_id } = JSON.parse(created.content[0].text);
    const state = await invoke('get_deck_summary', { deck_id });
    assert.equal(state._meta.aislide_timing.core_calls, 0);
    assert.equal(state._meta.aislide_timing.core_roundtrip_ms, 0);
    assert.equal(calls.length, 1);
    const pending = Promise.withResolvers();
    const started = Promise.withResolvers();
    fixture.onRequest = async () => { started.resolve(); return pending.promise; };
    const first = invoke('create_graph_icon', { base64: 'c3ludGhldGlj', mime_type: 'image/png' });
    await started.promise;
    const concurrent = await invoke('get_deck_summary', { deck_id });
    assert.equal(concurrent._meta.aislide_timing.core_calls, 0);
    pending.reject(new Error('Synthetic core failure'));
    const failed = await first;
    assert.equal(failed.isError, true);
    assert.equal(failed._meta.aislide_timing.core_calls, 1);
    assert.ok(failed._meta.aislide_timing.handler_elapsed_ms >= failed._meta.aislide_timing.core_roundtrip_ms);
    assert.ok(Buffer.byteLength(JSON.stringify(failed._meta)) < 256);
    assert.doesNotMatch(JSON.stringify(failed._meta), /Synthetic|base64|deck_id|sha256/);
  }, []);
});

test('retest preview contention explains sequential recovery for reads and edits', async () => {
  await feedbackMcpFixture(async ({ call, calls, fixture, registrations }) => {
    const created = await call('create_presentation', { title: 'Synthetic preview contention' });
    const pending = Promise.withResolvers();
    const started = Promise.withResolvers();
    const invoke = (name, input) => {
      const tool = registrations.get(name);
      return tool.callback(tool.config.inputSchema.parse(input), { signal: new AbortController().signal });
    };
    fixture.onRequest = async () => { started.resolve(); return pending.promise; };
    const preview = invoke('preview_presentation', { deck_id: created.deck_id });
    await started.promise;
    const before = calls.length;
    try {
      for (const [name, input] of [
        ['preview_presentation', { deck_id: created.deck_id }],
        ['apply_operations', { deck_id: created.deck_id, expected_revision: created.revision, expected_hash: created.hash, operations: [{ op: 'set_slide_background', slide_id: 'slide-1', color: 'FFFFFF' }] }],
      ]) {
        const response = await invoke(name, input);
        assert.equal(response.isError, true);
        assert.match(response.content[0].text, /busy.*sequentially.*revision\/hash/i);
      }
      assert.equal(calls.length, before);
    } finally {
      pending.reject(new Error('Synthetic preview stopped'));
      await preview;
    }
    assert.equal((await call('get_deck_summary', { deck_id: created.deck_id })).revision, created.revision);
  }, []);
});

test('roundtrip feedback skill and initialization expose the short safe authoring path', async () => {
  const skillUrl = new URL('../.github/skills/aislide-authoring/SKILL.md', import.meta.url);
  const skill = (await readFile(skillUrl, 'utf8')).replace(/^\uFEFF/, '');
  assert.match(skill, /^---\r?\nname: aislide-authoring\r?\ndescription: '[^\r\n]+'\r?\n---/);
  assert.ok(Buffer.byteLength(skill) < 6000);
  for (const text of ['asset_id', 'apply_operations', 'set_table_headers', 'set_accessibility', 'update_notes', '80 Unicode', 'Serialize core-backed AISlide calls', 'aislide_timing', 'unmet requirements']) assert.ok(skill.includes(text), text);
  for (const path of ['../../../docs/api.md', '../../../docs/authoring/README.md']) assert.ok((await readFile(new URL(path, skillUrl), 'utf8')).length > 0);
  await feedbackMcpFixture(async ({ fixture }) => {
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const client = new Client({ name: 'short-path-instructions', version: '1.0.0' });
    try {
      await McpServer.prototype.connect.call(fixture.instance, serverTransport);
      await client.connect(clientTransport);
      const instructions = client.getInstructions();
      assert.ok(Buffer.byteLength(instructions) < 1024);
      for (const text of ['Serialize core-backed AISlide calls', 'apply_operations', 'asset_id', 'get_deck_summary', 'unmet requirements']) assert.ok(instructions.includes(text), text);
    } finally { await client.close(); }
  }, []);
});

test('retest registered rasters expose validated dimensions and preserve image fit inputs', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-raster-info-'));
  const bytes = Buffer.from('Synthetic inspector input');
  try {
    await writeFile(join(directory, 'image.png'), bytes);
    await writeFile(join(directory, 'invalid.png'), Buffer.from('Invalid image'));
    await feedbackMcpFixture(async ({ call, calls, fixture, registrations }) => {
      const created = await call('create_presentation', { title: 'Synthetic image fitting' });
      fixture.onRequest = async request => {
        assert.equal(request.op, 'inspect_raster');
        assert.equal(request.mime_type, 'image/png');
        const input = Buffer.from(request.base64, 'base64');
        if (!input.equals(bytes)) throw new Error('Invalid raster image');
        return { width: 200, height: 100, mime_type: 'image/png', byte_length: input.length, sha256: createHash('sha256').update(input).digest('hex') };
      };
      const asset = await call('register_asset', { path: 'image.png' });
      assert.equal(asset.width, 200);
      assert.equal(asset.height, 100);
      assert.equal(asset.base64, undefined);
      const before = calls.length;
      assert.equal((await call('register_asset', { path: 'image.png' })).asset_id, asset.asset_id);
      assert.equal(calls.length, before, 'Immutable raster metadata must be reused');
      const tool = registrations.get('register_asset');
      const invalid = await tool.callback(tool.config.inputSchema.parse({ path: 'invalid.png' }), { signal: new AbortController().signal });
      assert.equal(invalid.isError, true);
      assert.equal((await call('list_assets')).assets.length, 1);
      fixture.onRequest = undefined;
      for (const fit of ['contain', 'cover', 'stretch']) {
        const input = { deck_id: created.deck_id, expected_revision: calls.filter(call => call.request.op === 'apply_operations').length, slide_id: 'slide-1', id: fit, asset_id: asset.asset_id, alt: 'Synthetic', frame: { x: 100, y: 100, width: 300, height: 200 }, fit };
        await call('add_picture', input);
        assert.equal(calls.at(-1).request.operations[0].fit, fit);
        assert.equal(calls.at(-1).request.operations[0].base64, bytes.toString('base64'));
        assert.equal(registrations.get('add_picture').config.inputSchema.safeParse({ ...input, fit: 'fill' }).success, false);
      }
    }, ['--asset-dir', directory]);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('retest raster registration rejects invalid dimensions and late cancellation atomically', async () => {
  const { McpAssets } = await import('./mcp-assets.mjs');
  const directory = await mkdtemp(join(tmpdir(), 'aislide-raster-atomic-'));
  const bytes = Buffer.from('Synthetic inspection fixture');
  try {
    await writeFile(join(directory, 'image.png'), bytes);
    const assets = await McpAssets.create([directory]);
    const info = { width: 200, height: 100, mime_type: 'image/png', byte_length: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
    for (const change of [{ width: undefined, height: undefined }, { width: 0 }, { height: NaN }, { width: 4097 }, { height: 0.5 }, { sha256: '0'.repeat(64) }, { mime_type: 'image/jpeg' }, { byte_length: 0 }]) {
      await assert.rejects(() => assets.registerFile({ path: 'image.png' }, new AbortController().signal, async () => ({ ...info, ...change })), /inspection|dimensions/i);
      assert.deepEqual(assets.list().assets, []);
    }
    const controller = new AbortController();
    await assert.rejects(() => assets.registerFile({ path: 'image.png' }, controller.signal, async () => { controller.abort(); return info; }), /abort/i);
    assert.deepEqual(assets.list().assets, []);
    const result = await assets.registerFile({ path: 'image.png' }, new AbortController().signal, async () => info);
    assert.equal(result.width, 200);
    assert.equal(result.height, 100);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('retest default batch schema defers advanced shapes while full runtime validation remains strict', async context => {
  await feedbackMcpFixture(async ({ fixture, calls }) => {
    const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
    const client = new Client({ name: 'progressive-batch-schema', version: '1.0.0' });
    const call = async (name, arguments_ = {}) => client.callTool({ name, arguments: arguments_ });
    const variants = schema => {
      const items = resolveSchemaRef(schema, resolveSchemaRef(schema, schema.properties.operations).items);
      return (items.oneOf ?? items.anyOf).map(variant => resolveSchemaRef(schema, variant));
    };
    try {
      await McpServer.prototype.connect.call(fixture.instance, serverTransport);
      await client.connect(clientTransport);
      const listed = await client.listTools();
      const basic = listed.tools.find(tool => tool.name === 'apply_operations').inputSchema;
      assert.ok(Buffer.byteLength(JSON.stringify(listed)) < 40 * 1024);
      assert.equal(variants(basic).some(variant => resolveSchemaRef(basic, variant.properties.op).const === 'add_graph'), false);
      const expanded = JSON.parse((await call('get_tool_schema', { name: 'apply_operations' })).content[0].text);
      assert.ok(variants(expanded.inputSchema).some(variant => resolveSchemaRef(expanded.inputSchema, variant.properties.op).const === 'add_graph'));
      const detailed = (await client.listTools()).tools.find(tool => tool.name === 'apply_operations').inputSchema;
      assert.deepEqual(detailed, expanded.inputSchema);
      const created = JSON.parse((await call('create_presentation', { title: 'Synthetic advanced schema' })).content[0].text);
      const operation = { op: 'add_elements', slide_id: 'slide-1', elements: [{ type: 'table', id: 'table', x: 0, y: 0, width: 200, height: 100, rows: [['Synthetic', 'Header']], font_size: 20 }] };
      const accepted = await call('apply_operations', { deck_id: created.deck_id, expected_revision: 0, expected_hash: created.hash, operations: [operation] });
      assert.equal(accepted.isError, undefined);
      const before = calls.length;
      operation.elements[0].rows = [['x'.repeat(201)]];
      const invalid = await call('apply_operations', { deck_id: created.deck_id, expected_revision: 1, expected_hash: 'b'.repeat(64), operations: [operation] });
      assert.equal(invalid.isError, true);
      assert.equal(calls.length, before);
      for (const name of ['get_document', 'undo', 'redo', 'get_graph']) await call('get_tool_schema', { name });
      const restored = (await client.listTools()).tools.find(tool => tool.name === 'apply_operations').inputSchema;
      assert.deepEqual(restored, basic);
      context.diagnostic(JSON.stringify({ default_tool_list_bytes: Buffer.byteLength(JSON.stringify(listed)), basic_batch_bytes: Buffer.byteLength(JSON.stringify(basic)), full_batch_bytes: Buffer.byteLength(JSON.stringify(detailed)) }));
    } finally { await client.close(); }
  }, []);
});

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
      assert.equal(asset.width, 1);
      assert.equal(asset.height, 1);
      assert.equal(calls.length, 1);
      assert.equal((await call('register_asset', { path: 'image.png' })).asset_id, asset.asset_id);
      assert.equal((await call('list_assets')).assets.length, 1);
      const created = await call('create_presentation', { title: 'Synthetic asset reuse' });
      await writeFile(join(directory, 'image.png'), Buffer.from('changed after registration'));
      const added = await call('apply_operations', { deck_id: created.deck_id, expected_revision: 0, expected_hash: created.hash, operations: [{ op: 'add_picture', slide_id: 'slide-1', id: 'picture', asset_id: asset.asset_id, alt: 'Synthetic image' }] });
      assert.equal(calls.length, 3);
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

test('scoped asset preparation is atomic cancellable and reports per-image cost without raw bytes', async () => {
  const { McpAssets } = await import('./mcp-assets.mjs');
  const assets = await McpAssets.create([]);
  const source = assets.retain(Buffer.from('Synthetic source'), 'png', 'source.png', { width: 200, height: 100 });
  const input = { asset_id: source.asset_id, params: { resize_longest_side: 100 } };
  const prepared = { base64: Buffer.from('Synthetic prepared').toString('base64'), mime_type: 'image/png', width: 100, height: 50 };
  const before = assets.list();
  let count = 0;
  await assert.rejects(() => assets.prepareRasters([input, input], undefined, async () => { if (++count === 2) throw new Error('Preparation failure'); return prepared; }), /Preparation failure/);
  assert.deepEqual(assets.list(), before);
  const controller = new AbortController();
  await assert.rejects(() => assets.prepareRasters([input], controller.signal, async () => { controller.abort(); return prepared; }), /abort/i);
  assert.deepEqual(assets.list(), before);
  const result = await assets.prepareRasters([input, input], undefined, async request => { assert.deepEqual(request.params, input.params); return prepared; });
  assert.equal(result.assets[0].asset_id, result.assets[1].asset_id);
  assert.equal(result.assets[0].source_asset_id, source.asset_id);
  assert.equal(result.usage.asset_count, 2);
  assert.equal(result.assets[0].raster_cost.rgba_byte_length, 20000);
  assert.equal(result.assets[0].raster_cost.encoded_byte_length, prepared.base64.length);
  assert.equal(result.assets[0].raster_cost.document_total_included, false);
  assert.equal(assets.get(source.asset_id).width, 200);
  assert.doesNotMatch(JSON.stringify(result), /base64|Synthetic prepared/);
  for (const invalid of [[], Array(33).fill(input), [{ ...input, path: 'unapproved.png' }], [{ asset_id: 'unknown', params: {} }]]) await assert.rejects(() => assets.prepareRasters(invalid, undefined, async () => prepared));
  const filled = await McpAssets.create([]);
  const originals = filled.retainMany(Array.from({ length: 32 }, (_, index) => ({ bytes: Buffer.from(`Synthetic ${index}`), format: 'png', name: `${index}.png` })));
  const full = filled.list();
  await assert.rejects(() => filled.prepareRasters([{ asset_id: originals[0].asset_id, params: {} }], undefined, async () => prepared), /limit reached/);
  assert.deepEqual(filled.list(), full);
});

test('scoped asset preparation MCP forwards explicit core edits and returns only handles', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-prepare-assets-'));
  try {
    await writeFile(join(directory, 'source.png'), 'Synthetic source');
    await feedbackMcpFixture(async ({ call, fixture, registrations }) => {
      const source = await call('register_asset', { path: 'source.png' });
      fixture.onRequest = async request => { assert.equal(request.op, 'edit_image'); assert.deepEqual(request.params, { resize_longest_side: 100 }); return { base64: Buffer.from('Prepared fixture').toString('base64'), mime_type: 'image/png', width: 100, height: 50 }; };
      const result = await call('prepare_assets', { assets: [{ asset_id: source.asset_id, params: { resize_longest_side: 100 } }] });
      assert.equal(result.assets[0].width, 100);
      assert.equal(result.assets[0].source_asset_id, source.asset_id);
      assert.doesNotMatch(JSON.stringify(result), /base64/);
      assert.equal(registrations.get('prepare_assets').config.inputSchema.safeParse({ assets: [{ asset_id: source.asset_id, params: { resize_longest_side: 5000 } }] }).success, false);
    }, ['--tool-profile', 'full', '--asset-dir', directory]);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('scoped asset batch rolls back failed reads and reports deduplicated registry usage', async () => {
  const { McpAssets } = await import('./mcp-assets.mjs');
  const directory = await mkdtemp(join(tmpdir(), 'aislide-asset-batch-'));
  try {
    await writeFile(join(directory, 'first.txt'), 'First');
    await writeFile(join(directory, 'second.txt'), 'Second');
    const assets = await McpAssets.create([directory]);
    const before = assets.list();
    await assert.rejects(() => assets.registerFiles([{ path: 'first.txt' }, { path: 'missing.txt' }]), /ENOENT/);
    assert.deepEqual(assets.list(), before);
    const result = await assets.registerFiles([{ path: 'first.txt' }, { path: 'second.txt' }, { path: 'first.txt' }]);
    assert.equal(result.assets.length, 3);
    assert.equal(result.assets[0].asset_id, result.assets[2].asset_id);
    assert.deepEqual(result.assets.map(asset => asset.name), ['first.txt', 'second.txt', 'first.txt']);
    assert.deepEqual(result.usage, {
      scope: 'process_asset_registry', asset_count: 2, asset_limit: 32, remaining_assets: 30,
      raw_byte_length: 11, raw_byte_limit: 67108864, remaining_raw_bytes: 67108853, document_budgets_included: false,
    });
    assert.deepEqual(assets.list().usage, result.usage);
    assert.equal(assets.list().byte_length, result.usage.raw_byte_length);
    assert.doesNotMatch(JSON.stringify(result), /base64|First|Second/);
    const single = await assets.registerFile({ path: 'first.txt' });
    assert.equal(single.asset_id, result.assets[0].asset_id);
    assert.deepEqual(single.usage, result.usage);
    const closed = assets.close(single.asset_id);
    assert.equal(closed.closed, single.asset_id);
    assert.equal(closed.usage.asset_count, 1);
    assert.equal(closed.usage.raw_byte_length, 6);
    assert.equal(closed.usage.remaining_assets, 31);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('scoped asset batch MCP stays advanced with strict shape and ordered cross-root raster metadata', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-batch-mcp-'));
  const other = await mkdtemp(join(tmpdir(), 'aislide-batch-root-'));
  try {
    await writeFile(join(directory, 'first.txt'), 'First');
    await writeFile(join(other, 'image.png'), 'Synthetic raster');
    await writeFile(join(other, 'image.jpg'), 'Synthetic JPEG');
    await feedbackMcpFixture(async ({ fixture, registrations, calls }) => {
      const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
      const client = new Client({ name: 'scoped-asset-batch', version: '1.0.0' });
      const call = async (name, arguments_ = {}) => {
        const result = await client.callTool({ name, arguments: arguments_ });
        assert.equal(result.isError, undefined, JSON.stringify(result.content));
        return JSON.parse(result.content[0].text);
      };
      try {
        await McpServer.prototype.connect.call(fixture.instance, serverTransport);
        await client.connect(clientTransport);
        const initial = await client.listTools();
        assert.equal(initial.tools.length, 10);
        assert.equal(initial.tools.some(tool => tool.name === 'register_assets'), false);
        const found = await call('discover_tools', { query: 'register_assets' });
        const advanced = found.tools.find(tool => tool.name === 'register_assets');
        assert.equal(advanced.loaded, false);
        assert.equal(advanced.read_only, false);
        const definition = await call('get_tool_schema', { name: 'register_assets' });
        assert.equal(definition.inputSchema.additionalProperties, false);
        const array = resolveSchemaRef(definition.inputSchema, definition.inputSchema.properties.assets);
        assert.equal(array.minItems, 1);
        assert.equal(array.maxItems, 32);
        const item = resolveSchemaRef(definition.inputSchema, array.items);
        assert.equal(item.additionalProperties, false);
        assert.deepEqual(item.required, ['path']);
        assert.equal(item.properties.path.maxLength, 512);
        assert.equal(item.properties.root.maximum, 7);
        assert.ok((await client.listTools()).tools.some(tool => tool.name === 'register_assets'));
        const schema = registrations.get('register_assets').config.inputSchema;
        for (const invalid of [
          {}, { assets: [] }, { assets: null }, { assets: Array(33).fill({ path: 'first.txt' }) },
          { assets: [{ path: '' }] }, { assets: [{ path: 'x'.repeat(513) }] }, { assets: [{ path: 'first.txt', root: -1 }] },
          { assets: [{ path: 'first.txt', root: 8 }] }, { assets: [{ path: 'first.txt', root: 0.5 }] },
          { assets: [{ path: 'first.txt', root: null }] }, { assets: [{ path: 'first.txt', directory }] },
          { assets: [{ path: 'first.txt' }], root: directory },
        ]) assert.equal(schema.safeParse(invalid).success, false, JSON.stringify(invalid));
        const invalid = await client.callTool({ name: 'register_assets', arguments: { assets: [{ path: 'first.txt', base64: 'ignored' }] } });
        assert.equal(invalid.isError, true);
        assert.equal(calls.length, 0);
        const result = await call('register_assets', { assets: [{ path: 'first.txt' }, { path: 'image.png', root: 1 }, { path: 'image.jpg', root: 1 }, { path: 'image.png', root: 1 }] });
        assert.deepEqual(Object.keys(result).sort(), ['assets', 'usage']);
        assert.equal(result.assets.length, 4);
        assert.equal(result.assets[1].asset_id, result.assets[3].asset_id);
        assert.equal(result.usage.asset_count, 3);
        assert.equal(result.usage.raw_byte_length, 35);
        assert.equal(result.usage.document_budgets_included, false);
        assert.deepEqual(result.assets.map(asset => asset.format), ['text', 'png', 'jpeg', 'png']);
        assert.equal(result.assets[1].width, 1);
        assert.equal(result.assets[2].height, 1);
        assert.deepEqual(calls.map(call => call.request.op), ['inspect_raster', 'inspect_raster']);
        assert.equal(result.assets.some(asset => 'base64' in asset || 'path' in asset || 'root' in asset || 'usage' in asset), false);
        assert.equal((await call('list_assets')).root_count, 2);
        assert.deepEqual((await call('list_assets')).usage, result.usage);
        assert.deepEqual((await call('register_asset', { path: 'first.txt' })).usage, result.usage);
        for (const name of ['register_asset', 'register_assets', 'list_assets', 'close_asset']) assert.match(registrations.get(name).config.description, /document.*encoded\/raster/i);
        assert.equal((await call('close_asset', { asset_id: result.assets[0].asset_id })).usage.raw_byte_length, 30);
        for (const name of ['list_assets', 'close_asset', 'get_document', 'undo']) await call('get_tool_schema', { name });
        assert.equal((await client.listTools()).tools.some(tool => tool.name === 'register_assets'), false);
      } finally { await client.close(); }
    }, ['--asset-dir', directory, '--asset-dir', other]);
  } finally { await rm(directory, { recursive: true, force: true }); await rm(other, { recursive: true, force: true }); }
});

test('scoped asset batch MCP cancellation leaves no partial usage and releases the mutation guard', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-batch-cancel-'));
  const bytes = Buffer.from('Synthetic raster');
  try {
    await writeFile(join(directory, 'first.txt'), 'First');
    await writeFile(join(directory, 'image.png'), bytes);
    await feedbackMcpFixture(async ({ call, fixture, registrations }) => {
      const before = await call('list_assets');
      const controller = new AbortController();
      const started = Promise.withResolvers();
      const pending = Promise.withResolvers();
      fixture.onRequest = async (request, options) => {
        assert.equal(request.op, 'inspect_raster');
        assert.equal(options.signal, controller.signal);
        started.resolve();
        return pending.promise;
      };
      const tool = registrations.get('register_assets');
      const input = { assets: [{ path: 'first.txt' }, { path: 'image.png' }] };
      const operation = tool.callback(tool.config.inputSchema.parse(input), { signal: controller.signal });
      try {
        await started.promise;
        assert.deepEqual(await call('list_assets'), before);
        const single = registrations.get('register_asset');
        const blocked = await single.callback(single.config.inputSchema.parse({ path: 'first.txt' }), { signal: new AbortController().signal });
        assert.equal(blocked.isError, true);
        assert.match(blocked.content[0].text, /Another mutation/);
      } finally {
        controller.abort();
        pending.resolve({ width: 1, height: 1, mime_type: 'image/png', byte_length: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') });
        await operation;
      }
      const cancelled = await operation;
      assert.equal(cancelled.isError, true);
      assert.match(cancelled.content[0].text, /abort|cancel/i);
      assert.deepEqual(await call('list_assets'), before);
      fixture.onRequest = undefined;
      const accepted = await call('register_assets', input);
      assert.equal(accepted.usage.asset_count, 2);
      assert.equal(accepted.usage.raw_byte_length, 21);
    }, ['--asset-dir', directory]);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('scoped asset batch rejects unauthorized paths and invalid files without retaining earlier items', async () => {
  const { McpAssets } = await import('./mcp-assets.mjs');
  const directory = await mkdtemp(join(tmpdir(), 'aislide-batch-paths-'));
  const outside = await mkdtemp(join(tmpdir(), 'aislide-batch-outside-'));
  try {
    await writeFile(join(directory, 'valid.txt'), 'Valid');
    await writeFile(join(directory, 'empty.txt'), '');
    await writeFile(join(directory, 'unsupported.bin'), 'Unsupported');
    await writeFile(join(directory, 'large.png'), '');
    await truncate(join(directory, 'large.png'), 1048577);
    await mkdir(join(directory, 'folder.txt'));
    await writeFile(join(outside, 'outside.txt'), 'Outside');
    await symlink(outside, join(directory, 'linked'), process.platform === 'win32' ? 'junction' : 'dir');
    const assets = await McpAssets.create([directory]);
    assets.retain(Buffer.from('Existing'), 'text', 'existing.txt');
    const before = assets.list();
    for (const input of [
      { path: '../outside.txt' }, { path: '..\\outside.txt' }, { path: join(outside, 'outside.txt') },
      { path: 'https://example.com/file.txt' }, { path: 'valid.txt:stream' }, { path: 'C:valid.txt' },
      { path: '\\\\?\\C:\\valid.txt' }, { path: 'CON.txt' }, { path: 'nul.txt' }, { path: 'COM1.txt' }, { path: 'lpt9.txt' },
      { path: 'valid.txt.' }, { path: 'valid.txt ' }, { path: 'folder/../valid.txt' }, { path: './valid.txt' },
      { path: 'folder//valid.txt' }, { path: 'bad\u0000.txt' }, { path: 'linked/outside.txt' }, { path: 'folder.txt' },
      { path: 'empty.txt' }, { path: 'unsupported.bin' }, { path: 'large.png' }, { path: 'valid.txt', root: 1 },
    ]) {
      await assert.rejects(() => assets.registerFiles([{ path: 'valid.txt' }, input]));
      assert.deepEqual(assets.list(), before, JSON.stringify(input));
    }
    for (const input of [[], Array(33).fill({ path: 'valid.txt' }), null, [{ path: 'valid.txt', root: 0.1 }], [{ path: 'valid.txt', extra: true }]]) {
      await assert.rejects(() => assets.registerFiles(input), /1-32|Invalid asset registration/);
      assert.deepEqual(assets.list(), before);
    }
    const unscoped = await McpAssets.create([]);
    await assert.rejects(() => unscoped.registerFiles([{ path: 'valid.txt' }]), /asset-dir/);
    assert.equal(unscoped.list().usage.asset_count, 0);
  } finally { await rm(directory, { recursive: true, force: true }); await rm(outside, { recursive: true, force: true }); }
});

test('scoped asset batch rolls back inspection errors cancellation and staged metadata upgrades', async () => {
  const { McpAssets } = await import('./mcp-assets.mjs');
  const directory = await mkdtemp(join(tmpdir(), 'aislide-batch-inspect-'));
  const bytes = Buffer.from('Synthetic raster');
  const info = { width: 200, height: 100, mime_type: 'image/png', byte_length: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
  try {
    await writeFile(join(directory, 'first.png'), bytes);
    await writeFile(join(directory, 'second.png'), 'Second raster');
    const assets = await McpAssets.create([directory]);
    const cached = assets.retain(bytes, 'png', 'cached.png');
    const before = assets.list();
    for (const failure of ['throw', 'invalid', 'cancel']) {
      const controller = new AbortController();
      let inspections = 0;
      await assert.rejects(() => assets.registerFiles([{ path: 'first.png' }, { path: 'second.png' }], controller.signal, async () => {
        inspections += 1;
        assert.deepEqual(assets.list(), before, 'No intermediate handles or dimension upgrades');
        if (inspections === 1) return info;
        if (failure === 'throw') throw new Error('Synthetic decoder failure');
        if (failure === 'cancel') controller.abort();
        return { ...info, width: 0 };
      }), /decoder failure|inspection|abort/i);
      assert.equal(inspections, 2);
      assert.deepEqual(assets.list(), before);
      assert.equal(assets.get(cached.asset_id).width, undefined);
    }
    const cancelled = new AbortController();
    cancelled.abort();
    await assert.rejects(() => assets.registerFiles([{ path: 'first.png' }], cancelled.signal, async () => assert.fail('No inspection after cancellation')), /abort/i);
    await assert.rejects(() => assets.registerFiles([{ path: 'first.png' }]), /inspection is required/);
    assert.deepEqual(assets.list(), before);
    const accepted = await assets.registerFiles([{ path: 'first.png' }], undefined, async () => info);
    assert.equal(accepted.assets[0].asset_id, cached.asset_id);
    assert.equal(accepted.assets[0].width, 200);
    assert.deepEqual(accepted.usage, before.usage);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('scoped asset batch detects file root and intermediate-link changes before commit', async () => {
  const { McpAssets } = await import('./mcp-assets.mjs');
  const directory = await mkdtemp(join(tmpdir(), 'aislide-batch-races-'));
  const bytes = Buffer.from('Synthetic raster');
  const info = { width: 1, height: 1, mime_type: 'image/png', byte_length: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
  try {
    for (const mutation of ['first-file', 'raster-file', 'root', 'link']) {
      const root = join(directory, mutation);
      const rasterRoot = join(directory, `${mutation}-raster`);
      await mkdir(join(root, 'sub'), { recursive: true });
      await mkdir(rasterRoot);
      await writeFile(join(root, 'sub', 'first.txt'), 'First');
      await writeFile(join(rasterRoot, 'image.png'), bytes);
      const assets = await McpAssets.create([root, rasterRoot]);
      const before = assets.list();
      await assert.rejects(() => assets.registerFiles([{ path: 'sub/first.txt' }, { path: 'image.png', root: 1 }], undefined, async () => {
        if (mutation === 'first-file') await writeFile(join(root, 'sub', 'first.txt'), 'Changed');
        if (mutation === 'raster-file') await writeFile(join(rasterRoot, 'image.png'), 'Changed');
        if (mutation === 'root') {
          await rename(root, `${root}-moved`);
          await mkdir(root);
        }
        if (mutation === 'link') {
          await rename(join(root, 'sub'), join(root, 'moved'));
          await symlink(join(root, 'moved'), join(root, 'sub'), process.platform === 'win32' ? 'junction' : 'dir');
        }
        return info;
      }), /changed|links/i);
      assert.deepEqual(assets.list(), before, mutation);
    }
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('scoped asset batch verifies unchanged bytes after a metadata-only ctime update', async () => {
  const { chmod, stat } = await import('node:fs/promises');
  const { McpAssets } = await import('./mcp-assets.mjs');
  const directory = await mkdtemp(join(tmpdir(), 'aislide-ctime-snapshot-'));
  const path = join(directory, 'first.txt');
  try {
    const content = Buffer.from('Unchanged content');
    const raster = Buffer.from('Synthetic raster fixture');
    await writeFile(path, content); await writeFile(join(directory, 'second.png'), raster);
    const assets = await McpAssets.create([directory]);
    const before = await stat(path, { bigint: true });
    const result = await assets.registerFiles([{ path: 'first.txt' }, { path: 'second.png' }], undefined, async () => {
      await chmod(path, 0o444);
      const current = await stat(path, { bigint: true });
      assert.equal(current.mtimeNs, before.mtimeNs); assert.equal(current.ino, before.ino);
      assert.notEqual(current.ctimeNs, before.ctimeNs);
      return { width: 1, height: 1, mime_type: 'image/png', sha256: createHash('sha256').update(raster).digest('hex'), byte_length: raster.length };
    });
    assert.equal(result.assets[0].sha256, createHash('sha256').update(content).digest('hex'));
    assert.equal(result.usage.asset_count, 2);
    assert.deepEqual(await readFile(path), content);
  } finally { await chmod(path, 0o600); await rm(directory, { recursive: true, force: true }); }
});

test('scoped asset batch preserves handle and raw byte budgets at atomic boundaries', async () => {
  const { McpAssets } = await import('./mcp-assets.mjs');
  const directory = await mkdtemp(join(tmpdir(), 'aislide-batch-budget-'));
  try {
    await writeFile(join(directory, 'one.txt'), '1');
    await writeFile(join(directory, 'two.txt'), '22');
    const handles = await McpAssets.create([directory]);
    handles.retainMany(Array.from({ length: 31 }, (_, index) => ({ bytes: Buffer.from(`Existing ${index}`), format: 'text', name: `existing-${index}.txt` })));
    const beforeHandles = handles.list();
    await assert.rejects(() => handles.registerFiles([{ path: 'one.txt' }, { path: 'two.txt' }]), /limit/);
    assert.deepEqual(handles.list(), beforeHandles);
    const full = await handles.registerFiles([{ path: 'one.txt' }]);
    assert.equal(full.usage.remaining_assets, 0);
    const duplicates = await handles.registerFiles(Array(32).fill({ path: 'one.txt' }));
    assert.equal(duplicates.assets.length, 32);
    assert.ok(duplicates.assets.every(asset => asset.asset_id === full.assets[0].asset_id));
    assert.deepEqual(duplicates.usage, full.usage);
    const bytes = await McpAssets.create([directory]);
    for (let index = 0; index < 4; index += 1) bytes.retain(Buffer.alloc(16 * 1048576 - (index === 3 ? 2 : 0), index), 'pptx', `synthetic-${index}.pptx`);
    const beforeBytes = bytes.list();
    assert.equal(beforeBytes.usage.remaining_raw_bytes, 2);
    await assert.rejects(() => bytes.registerFiles([{ path: 'one.txt' }, { path: 'two.txt' }]), /limit/);
    assert.deepEqual(bytes.list(), beforeBytes);
    const filled = await bytes.registerFiles([{ path: 'two.txt' }]);
    assert.equal(filled.usage.raw_byte_length, 67108864);
    assert.equal(filled.usage.remaining_raw_bytes, 0);
    assert.equal(filled.usage.remaining_assets, 27);
    assert.deepEqual((await bytes.registerFiles([{ path: 'two.txt' }])).usage, filled.usage);
    assert.equal(bytes.close(filled.assets[0].asset_id).usage.remaining_raw_bytes, 2);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('scoped asset batch rejects concurrent registry changes without losing independent retained assets', async () => {
  const { McpAssets } = await import('./mcp-assets.mjs');
  const directory = await mkdtemp(join(tmpdir(), 'aislide-batch-concurrent-'));
  const bytes = Buffer.from('Synthetic raster');
  try {
    await writeFile(join(directory, 'first.txt'), 'First');
    await writeFile(join(directory, 'image.png'), bytes);
    const assets = await McpAssets.create([directory]);
    let concurrent;
    await assert.rejects(() => assets.registerFiles([{ path: 'first.txt' }, { path: 'image.png' }], undefined, async () => {
      assert.equal(assets.list().usage.asset_count, 0);
      assets.retain(Buffer.from('Independent'), 'text', 'independent.txt');
      concurrent = assets.list();
      return { width: 1, height: 1, mime_type: 'image/png', byte_length: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
    }), /registry changed/);
    assert.deepEqual(assets.list(), concurrent);
  } finally { await rm(directory, { recursive: true, force: true }); }
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

test('roundtrip feedback live metadata batch configures twenty-five slides with one core call', { timeout: 180000 }, async context => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-metadata-batch-'));
  const sharp = (await import('sharp')).default;
  const image = await sharp({ create: { width: 200, height: 100, channels: 4, background: { r: 30, g: 150, b: 110, alpha: 1 } } }).png().toBuffer();
  await writeFile(join(directory, 'synthetic.png'), image);
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory, '--asset-dir', directory], stderr: 'pipe', env: coreEnvironment });
  const client = new Client({ name: 'roundtrip-metadata-proof', version: '1.0.0' });
  const invoke = async (name, arguments_ = {}) => {
    const response = await client.callTool({ name, arguments: arguments_ }, undefined, { timeout: 120000 });
    assert.ok(!response.isError, `${name}: ${JSON.stringify(response.content)}`);
    return { response, result: JSON.parse(response.content[0].text) };
  };
  const call = async (name, arguments_) => (await invoke(name, arguments_)).result;
  try {
    await client.connect(transport);
    const { deck_id } = await call('create_presentation', { title: 'Synthetic twenty-five-slide metadata' });
    const before = await call('edit_slides', { deck_id, expected_revision: 0, operations: Array.from({ length: 24 }, (_, index) => ({ op: 'insert', id: `slide-${index + 2}`, after: `slide-${index + 1}`, title: `Synthetic ${index + 2}` })) });
    const asset = await call('register_asset', { path: 'synthetic.png' });
    assert.equal(asset.width, 200);
    assert.equal(asset.height, 100);
    const operations = [];
    for (let index = 0; index < 25; index += 1) {
      const slide_id = `slide-${index + 1}`;
      if (index < 7) operations.push({ op: 'add_elements', slide_id, elements: [{ type: 'table', id: 'table', x: 80, y: 200, width: 400, height: 120, font_size: 20, rows: [['Synthetic', 'Value'], ['Row', String(index + 1)]] }] });
      if (index < 5) operations.push({ op: 'add_picture', slide_id, id: 'picture', asset_id: asset.asset_id, alt: 'Before description', frame: { x: 600, y: 200, width: 64, height: 64 }, fit: 'contain' });
      operations.push({ op: 'update_notes', slide_id, notes: `Synthetic speaker notes ${index + 1}` });
      if (index < 7) operations.push({ op: 'set_table_headers', slide_id, element_id: 'table', policy: 'first_row' });
      if (index < 5) operations.push({ op: 'set_accessibility', slide_id, element_id: 'picture', metadata: { title: 'Synthetic picture', description: `Synthetic alternative ${index + 1}` } });
    }
    const { response, result } = await invoke('apply_operations', { deck_id, expected_revision: before.revision, expected_hash: before.hash, operations });
    assert.equal(response._meta.aislide_timing.core_calls, 1);
    assert.equal(result.revision, before.revision + 1);
    const check = document => {
      assert.equal(document.deck.slides.length, 25);
      for (const [index, slide] of document.deck.slides.entries()) {
        assert.equal(slide.notes, `Synthetic speaker notes ${index + 1}`);
        if (index < 7) assert.equal(slide.review.table_headers.table, 'first_row');
        if (index < 5) {
          const picture = slide.elements.find(element => element.id === 'picture');
          assert.equal(picture.type, 'picture');
          assert.equal(picture.alt, `Synthetic alternative ${index + 1}`);
          assert.equal(picture.width, 64);
          assert.equal(picture.height, 32);
          assert.deepEqual(Buffer.from(picture.base64, 'base64'), image);
        }
      }
    };
    check(await call('get_document', { deck_id }));
    assert.equal((await call('undo', { deck_id })).hash, before.hash);
    assert.equal((await call('redo', { deck_id })).hash, result.hash);
    const saved = await call('export_pptx', { deck_id, filename: 'synthetic-metadata.pptx' });
    const bytes = await readFile(saved.path);
    const presentationAsset = await call('register_asset', { path: 'synthetic-metadata.pptx' });
    const reopened = await call('open_pptx', { asset_id: presentationAsset.asset_id });
    check(await call('get_document', { deck_id: reopened.deck_id }));
    assert.deepEqual(await readFile(saved.path), bytes);
    context.diagnostic(JSON.stringify({ slides: 25, notes: 25, table_headers: 7, images_with_alt: 5, operations: operations.length, core_calls: response._meta.aislide_timing.core_calls, timing: response._meta.aislide_timing, source_unchanged: true }));
    await call('close_deck', { deck_id: reopened.deck_id });
    await call('close_deck', { deck_id });
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
    /set_accessibility after its target exists.*same apply_operations batch/, /at least 65 seconds.*size-aware.*180 seconds.*opt-in MCP progress/,
    /Batch update_notes.*set_table_headers.*set_accessibility across slides in one Undo/, /Serialize core-backed AISlide calls/,
    /process sections require 2-4 body entries of at most 80 Unicode scalars/, /_meta.aislide_timing.*handler_elapsed_ms.*core_calls.*core_roundtrip_ms/,
    /disclose substitutions and unmet requirements/,
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
  const notes = authoring.properties.ledger_notes.anyOf.find(branch => branch.type === 'string');
  assert.equal(input.properties.slides.maxItems, 128);
  assert.equal(limit.minimum, 32);
  assert.equal(limit.maximum, 128);
  assert.deepEqual(headline.enum, ['sentence', 'keyword']);
  assert.deepEqual(notes.enum, ['full', 'summary', 'none']);
}

test('feedback MCP stub discovery shares reasoned authoring choices without core calls', async () => {
  await feedbackMcpFixture(async ({ registrations, resources, prompts, calls }) => {
    assertFeedbackWorkflow(await prompts.get('author_presentation').callback(), await resources.get('aislide://authoring/workflow').callback());
    assert.match(registrations.get('create_guided_presentation').config.description, /32 slides.*headline_style="keyword".*32\.\.128.*Evidence and numeric checks/);
    assert.match(registrations.get('create_guided_presentation').config.description, /ledger_notes.*full.*summary.*none.*Custom XML/);
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
    assert.equal(published.properties.operations.items.oneOf.length, 19);
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
      await assert.rejects(() => call('apply_operations', input), /in progress.*sequentially/i);
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
    assert.equal(operations.length, 19);
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
      assert.ok(group.children.every(child => !child.id.endsWith('-nt-client') && !child.id.endsWith('-nd-client')));
      const shape = group.children.find(child => child.id.endsWith('-n-client'));
      assert.equal(shape.type, 'shape');
      const [heading, ...detail] = shape.format.paragraphs;
      assert.equal(shape.text, `Client\n${detailText}`);
      assert.equal(heading.runs[0].text, 'Client');
      assert.equal(heading.runs[0].style.bold, false);
      assert.equal(heading.alignment, 'left');
      assert.equal(shape.format.vertical, 'top');
      assert.equal(detail[0].alignment, 'left');
      assert.equal(detail.map(paragraph => paragraph.runs[0].text).join('\n'), detailText);
      assert.ok(detail[0].space_before.value > 0);
      const detailSize = detail[0].runs[0].style.font_size;
      assert.ok(detailSize >= 12 && detailSize <= heading.runs[0].style.font_size);
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
    assert.equal(catalog.presets.length, 138);
    for (const id of ['list/rows', 'list-horizontal/columns', 'list-enumeration/grid']) {
      const preset = catalog.presets.find(entry => entry.id === id);
      assert.equal(preset.recommended, true);
      assert.ok(preset.use_when.length > 0); assert.ok(preset.avoid_when.length > 0);
      const element = await call('create_part', { id: 'open-list', spec: preset.example });
      assert.ok(element.children.every(child => child.type === 'text'));
      assert.ok(element.children.every(child => ['@dk1', '@dk2'].includes(child.color)));
    }
    for (const id of ['flow/open-steps', 'vertical-flow/rail', 'flow/roadmap', 'list-horizontal/icon-columns', 'list-horizontal/fact-columns', 'list-horizontal/image-columns']) {
      const preset = catalog.presets.find(entry => entry.id === id);
      assert.equal(preset.recommended, true, id);
      assert.ok(preset.use_when && preset.avoid_when, id);
      const element = await call('create_part', { id: 'editorial-part', spec: { ...preset.example, layout: { x: 48, y: 120, width: 1184, height: 540, show_title: false } } });
      assert.equal(element.type, 'group', id);
      assert.equal(element.width, element.view_width, id);
      assert.equal(element.height, element.view_height, id);
      assert.ok(element.children.length > 0, id);
      assert.ok(element.children.every(child => child.type !== 'chart'), id);
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

test('MCP layout patterns are opt-in and resolve frames for the deck canvas', async () => {
  const connect = async (args, name) => {
    const client = new Client({ name, version: '1.0.0' });
    await client.connect(new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), ...args], stderr: 'pipe', env: coreEnvironment }));
    const call = async (tool, input = {}) => {
      const result = await client.callTool({ name: tool, arguments: input });
      assert.ok(!result.isError, JSON.stringify(result.content));
      return JSON.parse(result.content[0].text);
    };
    return { client, call };
  };
  const disabled = await connect([], 'layout-patterns-off-test');
  try {
    assert.equal((await disabled.client.listTools()).tools.some(tool => tool.name === 'layout_patterns'), false);
    assert.equal((await disabled.call('discover_tools', { query: 'layout pattern' })).tools.some(tool => tool.name === 'layout_patterns'), false);
    assert.equal((await disabled.client.callTool({ name: 'get_tool_schema', arguments: { name: 'layout_patterns' } })).isError, true);
    assert.doesNotMatch(disabled.client.getInstructions(), /layout_patterns/);
  } finally { await disabled.client.close(); }
  const enabled = await connect(['--layout-patterns', 'on'], 'layout-patterns-on-test');
  try {
    assert.ok((await enabled.client.listTools()).tools.some(tool => tool.name === 'layout_patterns'));
    assert.match(enabled.client.getInstructions(), /layout_patterns/);
    const contrast = await enabled.call('layout_patterns', { relationship: 'contrast' });
    assert.ok(contrast.patterns.some(pattern => pattern.id === 'split/1-1'));
    assert.equal(contrast.patterns[0].slots, undefined);
    assert.equal(contrast.guidance.length, 8);
    const all = await enabled.call('layout_patterns', { limit: 50, offset: 50 });
    assert.equal(all.total, 54);
    assert.equal(all.next_offset, null);
    const standard = await enabled.call('layout_patterns', { pattern_id: 'columns/4', canvas: { width: 960, height: 720 } });
    assert.equal(standard.fits, false);
    assert.equal(standard.fallback, 'grid/2x2');
    const { deck_id } = await enabled.call('create_presentation', { title: 'Synthetic layout pattern canvas' });
    const resolved = await enabled.call('layout_patterns', { pattern_id: 'split/2-1', deck_id, message_band: true });
    assert.deepEqual(resolved.slots.map(slot => [slot.id, slot.frame]), [
      ['primary', { x: 48, y: 120, width: 768, height: 444 }],
      ['support', { x: 848, y: 120, width: 384, height: 444 }],
      ['message', { x: 48, y: 588, width: 1184, height: 72 }],
    ]);
    const ranked = await enabled.call('layout_patterns', { part_preset: 'cycle/balanced' });
    assert.equal(ranked.part.sensitivity, 'strict');
    assert.equal(ranked.patterns[0].fit, 'stretch');
    const fitted = await enabled.call('layout_patterns', { pattern_id: 'sequence/cycle', deck_id, part_preset: 'cycle/balanced' });
    assert.equal(fitted.slots[0].part_fit.fit, 'contain');
    assert.equal(fitted.slots[1].part_fit, null);
    for (const input of [{ pattern_id: 'split/1-1', query: 'contrast' }, { mirror: true }, { pattern_id: 'split/1-1', deck_id, canvas: { width: 1280, height: 720 } }, { pattern_id: 'columns/3', count: 3 }]) {
      assert.equal((await enabled.client.callTool({ name: 'layout_patterns', arguments: input })).isError, true, JSON.stringify(input));
    }
  } finally { await enabled.client.close(); }
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
    for (const suffix of ['-et-flow', '-eb-flow', '-g-boundary']) assert.equal(children.find(child => child.id.endsWith(suffix)).font_size, 11);
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

test('MCP guided ledger notes separate presenter text and preserve native creation records', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-guided-notes-mcp-'));
  const client = new Client({ name: 'guided-notes-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--tool-profile', 'full', '--output-dir', directory, '--asset-dir', directory], stderr: 'pipe', env: coreEnvironment });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content)}`);
    return JSON.parse(result.content[0].text);
  };
  const template = structuredClone(guidedExamples().find(input => input.profile_id === 'status-report'));
  template.slides = [template.slides[0]];
  const speaker = 'Synthetic comparison only.\nExplain the evidence before the recommendation.';
  try {
    await client.connect(transport);
    for (const mode of ['full', 'summary', 'none']) {
      const input = { ...structuredClone(template), authoring: { ledger_notes: mode } };
      input.slides[0].speaker_notes = speaker;
      assert.equal((await call('validate_guided_presentation', { input })).ready, true);
      const { deck_id } = await call('create_guided_presentation', { input });
      const original = await call('get_document', { deck_id });
      const notes = original.deck.slides[0].notes;
      if (mode === 'full') {
        assert.match(notes, /^Profile:/);
        assert.match(notes, /\nLedger:.*\nEvidence:/);
        assert.equal(Object.hasOwn(original, 'guided_record'), false);
      } else {
        assert.ok(notes.startsWith(speaker));
        assert.doesNotMatch(notes, /"body_paths"|"evidence_ids"/);
        if (mode === 'none') assert.equal(notes, speaker);
        assert.deepEqual(original.guided_record.input.evidence, input.evidence);
        assert.deepEqual(original.guided_record.input.slides[0].numbers, input.slides[0].numbers);
        assert.equal(Object.hasOwn(original.guided_record.input.slides[0], 'speaker_notes'), false);
      }
      const filename = `guided-notes-${mode}.pptx`;
      const saved = await call('export_pptx', { deck_id, filename });
      const bytes = await readFile(saved.path);
      assert.equal(bytes.subarray(0, 2).toString(), 'PK');
      const asset = await call('register_asset', { path: filename });
      const reopened = await call('open_pptx', { asset_id: asset.asset_id });
      const native = await call('get_document', { deck_id: reopened.deck_id });
      assert.equal(native.deck.slides[0].notes, notes);
      assert.deepEqual(native.guided_record, original.guided_record);
      assert.deepEqual(await readFile(saved.path), bytes);
      await call('apply_operations', { deck_id, expected_revision: original.revision, expected_hash: original.hash, operations: [{ op: 'update_notes', slide_id: original.deck.slides[0].id, notes: 'Later presenter edit' }] });
      assert.deepEqual((await call('get_document', { deck_id })).guided_record, original.guided_record);
      await call('undo', { deck_id });
      assert.equal((await call('get_document', { deck_id })).hash, original.hash);
      await call('close_deck', { deck_id: reopened.deck_id });
      await call('close_deck', { deck_id });
    }
    for (const mode of ['private', true, 1, {}, []]) {
      const input = { ...structuredClone(template), authoring: { ledger_notes: mode } };
      for (const name of ['validate_guided_presentation', 'create_guided_presentation']) {
        assert.equal((await client.callTool({ name, arguments: { input } })).isError, true);
      }
    }
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
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