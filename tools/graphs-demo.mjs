import assert from 'node:assert/strict';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createHash } from 'node:crypto';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const directory = resolve(process.argv[2] ?? `.artifacts/graphs-${Date.now()}`);
await mkdir(directory, { recursive: true });
assert.equal((await readdir(directory)).length, 0, 'Use a new or empty output directory');
const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
const client = new Client({ name: 'architecture-qualification', version: '1.0.0' });
const call = async (name, args = {}) => {
  const result = await client.callTool({ name, arguments: args });
  assert.ok(!result.isError, JSON.stringify(result.content));
  return JSON.parse(result.content[0].text);
};
try {
  await client.connect(transport);
  const catalog = await call('graph_catalog');
  const specs = catalog.examples.map((entry) => structuredClone(entry.spec));
  specs.push({
    version: 1, title: 'Routed exchange', subtitle: 'Synthetic example / attached, dashed and bidirectional connectors',
    nodes: [{ id: 'client', label: 'Client', x: 48, y: 128, width: 240, height: 104 }, { id: 'service', label: 'Service', kind: 'rounded_rectangle', x: 560, y: 320, width: 240, height: 104 }],
    edges: [{ id: 'reply', source: 'service', target: 'client', source_port: 'left', target_port: 'right', route: 'elbow', start_arrow: true, dashed: true, label: 'Response' }], groups: [],
  });
  specs.push({
    version: 1, title: 'Editable graph shapes', subtitle: 'Synthetic example / each label and connector remains native',
    nodes: catalog.shapes.map((kind, index) => ({ id: `node-${index}`, label: kind.replaceAll('_', ' '), kind, x: 40 + index % 3 * 376, y: 128 + Math.floor(index / 3) * 208, width: 264, height: 136 })),
    edges: [{ id: 'first', source: 'node-0', target: 'node-1', route: 'straight' }, { id: 'second', source: 'node-1', target: 'node-2', route: 'straight' }, { id: 'third', source: 'node-3', target: 'node-4', route: 'straight' }, { id: 'fourth', source: 'node-4', target: 'node-5', route: 'straight' }, { id: 'vertical', source: 'node-2', target: 'node-3', source_port: 'bottom', target_port: 'top', route: 'elbow' }], groups: [],
  });
  const created = await call('compile_report', { report: { title: 'AISlide architecture graphs', subtitle: '', period: '', source: 'Synthetic qualification examples, not a production topology', sections: specs.map((spec) => ({ title: spec.title, layout: 'statement', body: [], rows: [], metrics: [] })) } });
  let document = await call('get_document', { deck_id: created.deck_id });
  await call('apply_transaction', { deck_id: created.deck_id, expected_revision: document.revision, operations: document.deck.slides.map((_, index) => ({ op: 'replace', path: `/deck/slides/${index}/elements`, value: [] })) });
  for (const [index, spec] of specs.entries()) {
    document = await call('get_document', { deck_id: created.deck_id });
    await call('add_graph', { deck_id: created.deck_id, expected_revision: document.revision, slide_id: `slide-${index + 1}`, id: `graph-${index + 1}`, spec });
  }
  document = await call('get_document', { deck_id: created.deck_id });
  const counts = {};
  function visit(elements) { for (const element of elements) { counts[element.type] = (counts[element.type] ?? 0) + 1; if (element.type === 'group') visit(element.children); } }
  for (const slide of document.deck.slides) visit(slide.elements);
  await call('export_pptx', { deck_id: created.deck_id, filename: 'architecture-graphs.pptx' });
  const original = await readFile(join(directory, 'architecture-graphs.pptx'));
  assert.equal(original.subarray(0, 2).toString(), 'PK');
  const reopened = await call('open_pptx', { base64: original.toString('base64') });
  for (let index = 0; index < specs.length; index++) {
    const graph = await call('get_graph', { deck_id: reopened.deck_id, slide_id: `slide-${index + 1}`, id: `graph-${index + 1}` });
    assert.equal(graph.stale, false);
    assert.equal(graph.spec.nodes.length, specs[index].nodes.length);
  }
  await call('apply_graph', { deck_id: reopened.deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'graph-1', operations: [{ op: 'move', ids: ['user'], dx: 24, dy: 16 }] });
  const moved = await call('get_graph', { deck_id: reopened.deck_id, slide_id: 'slide-1', id: 'graph-1' });
  assert.equal(moved.spec.nodes[0].x, specs[0].nodes[0].x + 24);
  assert.equal(moved.stale, false);
  await call('export_pptx', { deck_id: reopened.deck_id, filename: 'architecture-graphs-edited.pptx' });
  const edited = await readFile(join(directory, 'architecture-graphs-edited.pptx'));
  const editedOpened = await call('open_pptx', { base64: edited.toString('base64') });
  assert.equal((await call('get_graph', { deck_id: editedOpened.deck_id, slide_id: 'slide-1', id: 'graph-1' })).stale, false);
  await call('undo', { deck_id: reopened.deck_id });
  await call('export_pptx', { deck_id: reopened.deck_id, filename: 'architecture-graphs-undone.pptx' });
  assert.deepEqual(await readFile(join(directory, 'architecture-graphs-undone.pptx')), original);
  assert.deepEqual(await readFile(join(directory, 'architecture-graphs.pptx')), original);
  const evidence = {
    generated_at: new Date().toISOString(), transport: 'Official MCP SDK / stdio / local Rust core', slides: specs.length, objects: counts,
    graphs: specs.map((spec) => ({ title: spec.title, nodes: spec.nodes.length, connections: spec.edges.length, groups: spec.groups?.length ?? 0 })),
    original_sha256: createHash('sha256').update(original).digest('hex'), edited_sha256: createHash('sha256').update(edited).digest('hex'),
    reopened_metadata_current: true, native_graph_edit: true, undo_byte_identical: true, original_unchanged: true, office_visual_parity: false,
  };
  await writeFile(join(directory, 'evidence.json'), JSON.stringify(evidence, null, 2), { flag: 'wx', encoding: 'utf8' });
  console.log(JSON.stringify({ directory, ...evidence }, null, 2));
} finally { await client.close(); }