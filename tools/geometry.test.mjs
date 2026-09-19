import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { AislideClient, DocumentSession } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';

const input = { ids: ['first', 'second'], operation: 'subtract', result_id: 'combined' };
const rectangle = (id, x, y, width, height) => ({ type: 'rect', id, x, y, width, height, fill: '007F73' });

test('G10 SDK combine guards revisions and early/late cancellation', async () => {
  const before = { id: 'g10', revision: 4, hash: 'before', deck: { slides: [] } };
  const after = { ...before, revision: 5, hash: 'after' };
  const effects = { added_ids: ['combined'], removed_ids: ['first', 'second'] };
  let calls = 0; let finish;
  const session = new DocumentSession(async request => {
    calls += 1;
    assert.deepEqual(request, { op: 'combine_shapes', document: before, expected_revision: 4, slide_id: 'slide', ...input });
    return new Promise(resolveReply => { finish = resolveReply; });
  }, before);
  const early = new AbortController(); early.abort();
  await assert.rejects(() => session.combineShapes('slide', input, { signal: early.signal }), /cancelled/i);
  await assert.rejects(() => session.combineShapes('slide', input, { expectedRevision: 3 }), /revision/i);
  assert.equal(calls, 0);
  const late = new AbortController();
  const pending = session.combineShapes('slide', input, { signal: late.signal });
  const rejected = assert.rejects(pending, /cancelled/i);
  late.abort(); finish({ transaction: { document: after, receipt: null }, effects, clipboard: null }); await rejected;
  assert.deepEqual(session.document, before);
  assert.equal(session.canUndo, false);
  const normal = session.combineShapes('slide', input, { expectedRevision: 4 });
  finish({ transaction: { document: after, receipt: { inverse: [] }, changes: ['geometry'] }, effects, clipboard: null });
  assert.deepEqual((await normal).effects, effects);
  assert.equal(session.revision, 5);
  assert.equal(session.canUndo, true);
});

test('G10 real SDK native hole, pixels, reopen and Undo', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('g10-sdk', 'Synthetic boolean fixture');
  const deck = session.document.deck;
  deck.slides[0].elements = [rectangle('first', 100, 100, 200, 200), rectangle('second', 150, 150, 100, 100)];
  await session.replaceDeck(deck);
  const source = await session.exportPresentation();
  const { session: native } = await client.openPresentation('g10-sdk', source.base64);
  const before = native.document;
  await native.combineShapes('slide-1', input, { expectedRevision: native.revision });
  const bytes = await native.exportPresentation();
  const { session: reopened } = await client.openPresentation('g10-reopen', bytes.base64);
  const polygon = reopened.document.deck.slides[0].elements[0];
  assert.equal(polygon.type, 'polygon');
  assert.equal(polygon.visual.path.commands.filter(command => command.op === 'move').length, 2);
  const image = await reopened.exportStatic({ format: 'png', scale: 1 });
  const { default: sharp } = await import('sharp');
  const { data, info } = await sharp(Buffer.from(image.files[0].base64, 'base64')).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  const pixel = (x, y) => Array.from(data.subarray((y * info.width + x) * 4, (y * info.width + x) * 4 + 4));
  assert.deepEqual(pixel(125, 125), [0, 127, 115, 255]);
  assert.deepEqual(pixel(200, 200), [255, 255, 255, 255]);
  await native.undo();
  assert.equal(native.document.hash, before.hash);
  assert.equal((await native.exportPresentation()).base64, source.base64);
  await native.redo();
  assert.equal(native.document.deck.slides[0].elements.length, 1);
});

test('G10 MCP strict combine schema, transaction, native export and Undo', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-g10-'));
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
  const client = new Client({ name: 'g10-proof', version: '1.0.0' });
  const call = async (name, args = {}) => {
    const reply = await client.callTool({ name, arguments: args });
    assert.ok(!reply.isError, `${name}: ${JSON.stringify(reply.content)}`);
    return JSON.parse(reply.content[0].text);
  };
  try {
    await client.connect(transport);
    const tool = (await client.listTools()).tools.find(tool => tool.name === 'combine_shapes');
    assert.ok(tool);
    assert.equal(tool.inputSchema.additionalProperties, false);
    assert.equal(tool.inputSchema.properties.ids.maxItems, 32);
    const { deck_id } = await call('create_presentation', { title: 'Synthetic geometry' });
    await call('apply_transaction', { deck_id, expected_revision: 0, operations: [{ op: 'replace', path: '/deck/slides/0/elements', value: [rectangle('first', 100, 100, 200, 200), rectangle('second', 150, 150, 100, 100)] }] });
    const before = await call('get_document', { deck_id });
    for (const invalid of [{ operation: 'invalid' }, { ids: ['first'] }, { ids: Array(33).fill('first') }, { unexpected: true }, { expected_revision: 0 }]) {
      const reply = await client.callTool({ name: 'combine_shapes', arguments: { deck_id, expected_revision: before.revision, slide_id: 'slide-1', ...input, ...invalid } });
      assert.equal(reply.isError, true);
      assert.equal((await call('get_document', { deck_id })).hash, before.hash);
    }
    const combined = await call('combine_shapes', { deck_id, expected_revision: before.revision, slide_id: 'slide-1', ...input });
    assert.deepEqual(combined.effects.removed_ids, ['first', 'second']);
    assert.equal(combined.revision, before.revision + 1);
    await call('export_pptx', { deck_id, filename: 'synthetic-hole.pptx' });
    const bytes = await readFile(join(directory, 'synthetic-hole.pptx'));
    assert.equal(bytes.subarray(0, 2).toString(), 'PK');
    await call('undo', { deck_id });
    assert.equal((await call('get_document', { deck_id })).hash, before.hash);
    const fragments = await call('combine_shapes', { deck_id, expected_revision: (await call('get_document', { deck_id })).revision, slide_id: 'slide-1', ...input, operation: 'fragment' });
    assert.deepEqual(fragments.effects.added_ids, ['combined', 'combined-2']);
    await call('undo', { deck_id });
    assert.equal((await call('get_document', { deck_id })).hash, before.hash);
  } finally { await client.close(); await transport.close(); await rm(directory, { recursive: true, force: true }); }
});

test('phase2 SDK vector assets retain native SVG and exact Undo after conversion', async () => {
  const client = new AislideClient(requestCore);
  const { default: sharp } = await import('sharp');
  const png = (await sharp({ create: { width: 2, height: 2, channels: 3, background: '#00AA44' } }).png().toBuffer()).toString('base64');
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40"><text x="0" y="20" font-size="18">Literal SVG</text><image x="100" width="20" height="40" href="data:image/png;base64,${png}"/></svg>`;
  const emf = Buffer.alloc(144);
  for (const [offset, value] of [[0,1],[4,88],[16,100],[20,100],[40,0x464d4520],[44,0x10000],[48,144],[52,4],[56,1],[72,100],[76,100],[80,25],[84,25]]) emf.writeUInt32LE(value, offset);
  [37,12,0x80000004,43,24,10,10,90,90,14,20,0,0,20].forEach((value,index) => emf.writeUInt32LE(value,88+index*4));
  const wmf = Buffer.alloc(82);
  wmf.writeUInt32LE(0x9ac6cdd7,0);
  [0,0,0,100,100,1440,0,0].forEach((value,index) => wmf.writeUInt16LE(value,4+index*2));
  let checksum=0; for(let offset=0;offset<20;offset+=2) checksum ^= wmf.readUInt16LE(offset); wmf.writeUInt16LE(checksum,20);
  [1,9,0x300,30,0,1,7,0,0,7,0,0x02fc,0,0x00ff,0,0,4,0,0x012d,0,7,0,0x041b,90,90,10,10,3,0,0].forEach((value,index) => wmf.writeUInt16LE(value,22+index*2));
  for (const [mime_type, bytes] of [['image/svg+xml',Buffer.from(svg)],['image/emf',emf],['image/wmf',wmf]]) {
    const picture = await client.createAsset({ id: 'vector', base64: bytes.toString('base64'), mime_type, alt: 'Synthetic vector', size: 120 });
    assert.ok(picture.svg);
    const icon = await client.createGraphIcon({ base64:bytes.toString('base64'),mime_type,alt:'Converted icon' });
    assert.equal(icon.mime_type,'image/png');
    const session = await client.createPresentation(`phase2-${mime_type.split('/')[1].replace('+','-')}`);
    const deck = session.document.deck; deck.slides[0].elements = [picture]; await session.replaceDeck(deck);
    const original = await session.exportPresentation();
    const native = (await client.openPresentation('phase2-native',original.base64)).session;
    await native.transact([{ op: 'replace', path: '/deck/slides/0/elements/0/x', value: 300 }]);
    const reopened = (await client.openPresentation('phase2-reopen',(await native.exportPresentation()).base64)).session;
    assert.equal(reopened.document.deck.slides[0].elements[0].svg,picture.svg);
    await native.undo(); assert.equal((await native.exportPresentation()).base64,original.base64);
  }
});