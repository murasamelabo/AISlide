import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

test('MCP guided authoring retrieves profiles and creates only evidence-linked decks', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-guided-mcp-'));
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
  const client = new Client({ name: 'guided-proof', version: '1.0.0' });
  const call = async (name, args = {}) => { const response = await client.callTool({ name, arguments: args }); assert.ok(!response.isError, JSON.stringify(response.content)); return JSON.parse(response.content[0].text); };
  const headline = 'We should validate each boundary before we deploy the service.';
  const input = { version: 1, profile_id: 'technical-explainer', title: 'Technical briefing', audience: 'Engineering reviewers', purpose: 'Agree the validation order', governing_message: headline, language: 'en', evidence: [{ id: 'design', kind: 'assumption', reference: 'Synthetic architecture proposal for this test', statement: 'Validation before deployment is a proposed control, not a measured outcome.' }], slides: [{ id: 'process', section: 'Validation', headline, sentence_form: 'proposal', pattern_id: 'native-part', question: 'What should we validate?', parent_message: 'governing', transition: 'therefore', parallel_basis: 'boundary', part: { version: 1, preset: 'flow/balanced', title: 'Proposed validation stages', subtitle: 'Synthetic proposal', data: { kind: 'items', items: [{ label: 'Identity', detail: 'Verify the caller' }, { label: 'Data', detail: 'Validate the contract' }, { label: 'Release', detail: 'Check the outcome' }] } }, support: [{ clause: headline, body_paths: ['/data/items'], evidence_ids: ['design'] }], numbers: [] }] };
  try {
    await client.connect(transport);
    const profiles = await call('best_practice_profiles');
    assert.equal(profiles.profiles.length, 4);
    const guide = await call('best_practice_guide', { profile_id: 'consulting-decision' });
    assert.equal(guide.patterns.length, 48);
    assert.equal(guide.semantic_truth_verified, false);
    assert.match(guide.markdown, /Claim/);
    assert.equal((await call('validate_guided_presentation', { input })).ready, true);
    assert.deepEqual(await readdir(directory), []);
    const invalid = structuredClone(input); invalid.slides[0].support = [];
    assert.equal((await call('validate_guided_presentation', { input: invalid })).ready, false);
    assert.equal((await client.callTool({ name: 'create_guided_presentation', arguments: { input: invalid } })).isError, true);
    const result = await call('create_guided_presentation', { input });
    assert.equal(result.slides, 1);
    assert.equal(result.validation.ready, true);
    assert.equal(result.model_inference, false);
    const document = await call('get_document', { deck_id: result.deck_id });
    assert.equal(document.parts[0].spec.preset, 'flow/balanced');
    assert.equal(document.parts[0].stale, false);
    await call('export_pptx', { deck_id: result.deck_id, filename: 'guided.pptx' });
    const bytes = await readFile(join(directory, 'guided.pptx'));
    assert.equal(bytes.subarray(0, 2).toString(), 'PK');
    const reopened = await call('open_pptx', { base64: bytes.toString('base64') });
    assert.equal((await call('get_document', { deck_id: reopened.deck_id })).parts[0].stale, false);
    await call('update_part', { deck_id: result.deck_id, expected_revision: document.revision, slide_id: 'process', id: document.parts[0].element_id, spec: { ...document.parts[0].spec, title: 'Updated validation stages' } });
    await call('undo', { deck_id: result.deck_id });
    assert.equal((await call('get_document', { deck_id: result.deck_id })).hash, document.hash);
    assert.equal((await client.callTool({ name: 'best_practice_guide', arguments: { profile_id: '../../secret' } })).isError, true);
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});

test('MCP design presets share the native catalog and guarded application', async () => {
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
  const client = new Client({ name: 'preset-proof', version: '1.0.0' });
  const call = async (name, args = {}) => { const response = await client.callTool({ name, arguments: args }); assert.ok(!response.isError, JSON.stringify(response.content)); return JSON.parse(response.content[0].text); };
  try {
    await client.connect(transport);
    const { presets } = await call('design_presets');
    assert.equal(presets.length, 7);
    const { deck_id } = await call('create_presentation', { title: 'Preset fixture' });
    await call('apply_design_preset', { deck_id, expected_revision: 0, preset_id: 'trust' });
    assert.equal((await call('get_deck', { deck_id })).design.theme.name, 'Trusted report');
    assert.equal((await client.callTool({ name: 'apply_design_preset', arguments: { deck_id, expected_revision: 0, preset_id: 'minimal' } })).isError, true);
    await call('undo', { deck_id });
    assert.equal((await call('get_deck', { deck_id })).design.masters.length, 1);
  } finally { await client.close(); }
});

test('MCP workspace commands create blank files, import icons and edit native slides', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-mcp-workspace-'));
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
  const client = new Client({ name: 'workspace-proof', version: '1.0.0' });
  const call = async (name, args = {}) => { const response = await client.callTool({ name, arguments: args }); assert.ok(!response.isError, `${name}: ${JSON.stringify(response.content)}`); return JSON.parse(response.content[0].text); };
  try {
    await client.connect(transport);
    const created = await call('create_presentation', { title: 'Workspace example' });
    const deck_id = created.deck_id;
    assert.equal((await call('get_document', { deck_id })).deck.slides[0].elements.length, 0);
    await call('edit_slides', { deck_id, expected_revision: 0, operations: [{ op: 'insert', id: 'icons', after: 'slide-1', title: 'Icon slide' }] });
    await call('add_asset', { deck_id, expected_revision: 1, slide_id: 'icons', id: 'symbol', size: 96, alt: 'Provided synthetic icon', mime_type: 'image/svg+xml', base64: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="#0017c1"/></svg>').toString('base64') });
    await call('edit_elements', { deck_id, expected_revision: 2, slide_id: 'icons', operations: [{ op: 'duplicate', id: 'symbol', new_id: 'symbol-copy' }, { op: 'order', id: 'symbol-copy', index: 0 }] });
    await call('undo', { deck_id });
    await call('export_pptx', { deck_id, filename: 'workspace.pptx' });
    const original = await readFile(join(directory, 'workspace.pptx'));
    const opened = await call('open_pptx', { base64: original.toString('base64') });
    await call('edit_slides', { deck_id: opened.deck_id, expected_revision: 0, operations: [{ op: 'duplicate', slide_id: 'icons', id: 'native-copy' }, { op: 'move', slide_id: 'native-copy', index: 0 }, { op: 'remove', slide_id: 'slide-1' }] });
    const modified = await call('get_document', { deck_id: opened.deck_id });
    assert.equal(modified.deck.slides[0].id, 'native-copy');
    assert.equal(modified.deck.slides.length, 2);
    assert.equal(modified.deck.slides[0].elements[0].mime_type, 'image/png');
    const stale = await client.callTool({ name: 'edit_slides', arguments: { deck_id: opened.deck_id, expected_revision: 0, operations: [{ op: 'remove', slide_id: 'icons' }] } });
    assert.equal(stale.isError, true);
    await call('export_pptx', { deck_id: opened.deck_id, filename: 'workspace-edited.pptx' });
    const verified = await call('open_pptx', { base64: (await readFile(join(directory, 'workspace-edited.pptx'))).toString('base64') });
    assert.equal((await call('get_document', { deck_id: verified.deck_id })).deck.slides.length, 2);
    assert.deepEqual(await readFile(join(directory, 'workspace.pptx')), original);
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});

test('MCP graph tools share native editing, revision guards and standalone PPTX', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-mcp-graph-'));
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
  const client = new Client({ name: 'graph-proof', version: '1.0.0' });
  const call = async (name, args = {}) => { const response = await client.callTool({ name, arguments: args }); assert.ok(!response.isError, JSON.stringify(response.content)); return JSON.parse(response.content[0].text); };
  try {
    await client.connect(transport);
    const catalog = await call('graph_catalog');
    const picture = await call('create_graph_icon', { alt: 'Application icon', mime_type: 'image/svg+xml', base64: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect width="24" height="24" fill="#007a4d"/></svg>').toString('base64') });
    const spec = structuredClone(catalog.examples[2].spec);
    spec.nodes[1].icon = { base64: picture.base64, mime_type: picture.mime_type, alt: picture.alt };
    const created = await call('compile_report', { report: { title: 'Graph fixture', subtitle: '', period: '', source: 'Synthetic fixture', sections: [{ title: 'Architecture', layout: 'statement', body: [], rows: [], metrics: [] }] } });
    const deck_id = created.deck_id;
    await call('add_graph', { deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'network', spec });
    await call('apply_graph', { deck_id, expected_revision: 1, slide_id: 'slide-1', id: 'network', operations: [{ op: 'move', ids: ['private'], dx: 16, dy: 16 }] });
    const graph = await call('get_graph', { deck_id, slide_id: 'slide-1', id: 'network' });
    assert.equal(graph.spec.nodes[1].x, 446);
    assert.equal(graph.stale, false);
    assert.deepEqual(graph.spec.nodes[1].icon, spec.nodes[1].icon);
    const stale = await client.callTool({ name: 'apply_graph', arguments: { deck_id, expected_revision: 1, slide_id: 'slide-1', id: 'network', operations: [{ op: 'remove', ids: ['client'] }] } });
    assert.equal(stale.isError, true);
    await call('export_pptx', { deck_id, filename: 'graph.pptx' });
    const bytes = await readFile(join(directory, 'graph.pptx'));
    const reopened = await call('open_pptx', { base64: bytes.toString('base64') });
    const restored = await call('get_graph', { deck_id: reopened.deck_id, slide_id: 'slide-1', id: 'network' });
    assert.equal(restored.stale, false);
    assert.deepEqual(restored.spec.nodes[1].icon, spec.nodes[1].icon);
    await call('undo', { deck_id });
    assert.equal((await call('get_graph', { deck_id, slide_id: 'slide-1', id: 'network' })).spec.nodes[1].x, 430);
    await call('apply_graph', { deck_id, expected_revision: 3, slide_id: 'slide-1', id: 'network', operations: [{ op: 'put_node', node: { ...spec.nodes[1], icon: null } }] });
    assert.equal((await call('get_graph', { deck_id, slide_id: 'slide-1', id: 'network' })).spec.nodes[1].icon, undefined);
    await call('undo', { deck_id });
    assert.deepEqual((await call('get_graph', { deck_id, slide_id: 'slide-1', id: 'network' })).spec.nodes[1].icon, spec.nodes[1].icon);
    const unsupported = structuredClone(spec); unsupported.nodes[1].icon.mime_type = 'image/svg+xml';
    assert.equal((await client.callTool({ name: 'create_graph', arguments: { id: 'unsupported', spec: unsupported } })).isError, true);
    assert.deepEqual(await readFile(join(directory, 'graph.pptx')), bytes);
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});

test('MCP authoring tools insert objects and edit master theme with transactional undo', async () => {
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
  const client = new Client({ name: 'authoring-proof', version: '1.0.0' });
  const call = async (name, args = {}) => {
    const response = await client.callTool({ name, arguments: args });
    assert.ok(!response.isError, JSON.stringify(response.content));
    return JSON.parse(response.content[0].text);
  };
  try {
    await client.connect(transport);
    const catalog = await call('object_catalog'); assert.equal(catalog.charts.length, 9);
    const design = await call('design_defaults');
    const created = await call('compile_report', { report: { title: 'Authoring', subtitle: '', period: '', source: 'Synthetic fixture', sections: [{ title: 'Master fixture', layout: 'statement', body: ['Native authoring'], rows: [], metrics: [] }] } });
    const deck_id = created.deck_id;
    await call('update_design', { deck_id, expected_revision: 0, design });
    await call('assign_layout', { deck_id, expected_revision: 1, slide_id: 'slide-1', layout_id: 'title-content' });
    await call('add_object', { deck_id, expected_revision: 2, slide_id: 'slide-1', id: 'native-shape', kind: 'shape', preset: 'ellipse' });
    await call('update_text', { deck_id, expected_revision: 3, slide_id: 'slide-1', element_id: 'native-shape', text: 'Shape through MCP' });
    const theme = structuredClone(design.theme); theme.colors.accent1 = 'B53055';
    await call('apply_theme', { deck_id, expected_revision: 4, theme });
    let deck = await call('get_deck', { deck_id });
    assert.equal(deck.design.theme.colors.accent1, 'B53055');
    assert.equal(deck.slides[0].elements.at(-1).text, 'Shape through MCP');
    await call('undo', { deck_id }); deck = await call('get_deck', { deck_id });
    assert.equal(deck.design.theme.colors.accent1, design.theme.colors.accent1);
    const stale = await client.callTool({ name: 'assign_layout', arguments: { deck_id, expected_revision: 0, slide_id: 'slide-1', layout_id: 'blank' } });
    assert.equal(stale.isError, true);
    const parts = await call('part_catalog'); assert.equal(parts.presets.length, 108);
    const spec = parts.presets.find((preset) => preset.id === 'flow/balanced').example;
    await call('add_part', { deck_id, expected_revision: 6, slide_id: 'slide-1', id: 'mcp-part', spec });
    await call('update_part', { deck_id, expected_revision: 7, slide_id: 'slide-1', id: 'mcp-part', spec: { ...spec, title: 'MCP part update' } });
    assert.equal((await call('get_document', { deck_id })).parts[0].spec.title, 'MCP part update');
    await call('undo', { deck_id });
    assert.equal((await call('get_document', { deck_id })).parts[0].spec.title, spec.title);
  } finally { await client.close(); }
});

test('MCP saves and reopens one source-bound PPTX with native graphics and transactions', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-mcp-poc-'));
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
  const client = new Client({ name: 'poc-proof', version: '1.0.0' });
  const call = async (name, args = {}) => {
    const response = await client.callTool({ name, arguments: args });
    assert.ok(!response.isError, JSON.stringify(response.content));
    return JSON.parse(response.content[0].text);
  };
  try {
    await client.connect(transport);
    const source = await call('ingest_source', { format: 'csv', name: 'fixture.csv', base64: Buffer.from('Quarter,Value\nQ1,-2\nQ2,0\nQ3,12\n').toString('base64') });
    const result = await call('compile_data_report', { source_id: source.source_id, mapping: { title: 'MCP provided values', period: 'Synthetic fixture', table_index: 0, category_column: 0, value_columns: [1], row_start: 0, row_count: 3, chart_kind: 'column' } });
    assert.equal(result.slides, 12);
    const before = await call('get_document', { deck_id: result.deck_id });
    assert.equal(before.sources[0].name, 'fixture.csv');
    const titleIndex = before.deck.slides[0].elements.findIndex((element) => element.id === 'title');
    const change = await call('apply_transaction', { deck_id: result.deck_id, expected_revision: 0, operations: [{ op: 'replace', path: `/deck/slides/0/elements/${titleIndex}/text`, value: 'Verified MCP edit' }] });
    assert.equal(change.revision, 1);
    const stale = await client.callTool({ name: 'apply_transaction', arguments: { deck_id: result.deck_id, expected_revision: 0, operations: [{ op: 'replace', path: '/deck/title', value: 'Stale' }] } });
    assert.equal(stale.isError, true);
    await call('undo', { deck_id: result.deck_id });
    assert.equal((await call('get_document', { deck_id: result.deck_id })).deck.slides[0].elements[titleIndex].text, before.deck.slides[0].elements[titleIndex].text);
    await call('redo', { deck_id: result.deck_id });
    await call('add_diagram', { deck_id: result.deck_id, slide_id: 'slide-12', id: 'mcp-flow', steps: ['Ingest', 'Validate', 'Export'] });
    const parts = await call('part_catalog');
    const spec = parts.presets.find((preset) => preset.id === 'pyramid/balanced').example;
    await call('add_part', { deck_id: result.deck_id, expected_revision: 4, slide_id: 'slide-11', id: 'persisted-part', spec });
    await call('export_pptx', { deck_id: result.deck_id, filename: 'poc-proof.pptx' });
    const pptx = await readFile(join(directory, 'poc-proof.pptx'));
    assert.equal(pptx.subarray(0, 2).toString(), 'PK');
    assert.deepEqual(await readdir(directory), ['poc-proof.pptx']);
    const restored = await call('open_pptx', { base64: pptx.toString('base64') });
    const restoredDocument = await call('get_document', { deck_id: restored.deck_id });
    assert.equal(restoredDocument.sources[0].sha256, before.sources[0].sha256);
    assert.equal(restoredDocument.bindings.length, before.bindings.length);
    assert.equal(restoredDocument.parts[0].spec.preset, 'pyramid/balanced');
    assert.equal(restoredDocument.parts[0].stale, false);
    assert.equal(restoredDocument.deck.slides[0].elements[titleIndex].text, 'Verified MCP edit');
    await call('update_text', { deck_id: restored.deck_id, expected_revision: 0, slide_id: restoredDocument.deck.slides[0].id, element_id: restoredDocument.deck.slides[0].elements[titleIndex].id, text: 'Reopened PPTX edit' });
    await call('undo', { deck_id: restored.deck_id });
    const duplicate = await client.callTool({ name: 'export_pptx', arguments: { deck_id: result.deck_id, filename: 'poc-proof.pptx' } });
    assert.equal(duplicate.isError, true);
    assert.deepEqual(await readFile(join(directory, 'poc-proof.pptx')), pptx);
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});