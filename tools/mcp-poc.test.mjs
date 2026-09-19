import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

test('MCP expanded authoring exposes strict APIs, revisions and create-new templates', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-expanded-mcp-'));
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
  const client = new Client({ name: 'expanded-api-proof', version: '1.0.0' });
  const call = async (name, args = {}) => { const response = await client.callTool({ name, arguments: args }); assert.ok(!response.isError, `${name}: ${JSON.stringify(response.content)}`); return JSON.parse(response.content[0].text); };
  try {
    await client.connect(transport);
    const listed = await client.listTools();
    for (const name of ['authoring_capabilities', 'search_text', 'replace_text', 'replace_font', 'format_text', 'replace_text_content', 'update_paragraphs', 'edit_selection', 'edit_table', 'edit_image', 'apply_image_edit', 'resize_canvas', 'import_template', 'export_template']) {
      assert.ok(listed.tools.some((tool) => tool.name === name), `missing ${name}`);
    }
    const capabilities = await call('authoring_capabilities');
    assert.equal(capabilities.limits.request_bytes, 100663296);
    assert.equal(capabilities.capacity_profiles.legacy.request_bytes, 4194304);
    assert.equal(capabilities.charts.filter((chart) => chart.create).length, 24);
    const { deck_id } = await call('create_presentation', { title: 'Synthetic expanded API' });
    let revision = 0;
    const mutate = async (name, input) => { const result = await call(name, { deck_id, expected_revision: revision, ...input }); revision = result.revision; return result; };
    await mutate('add_object', { slide_id: 'slide-1', id: 'text', kind: 'text' });
    await mutate('replace_text_content', { slide_id: 'slide-1', id: 'text', text: 'Alpha Beta' });
    const matches = await call('search_text', { deck_id, options: { query: 'Alpha' } });
    assert.equal(matches.matches.length, 1);
    await mutate('replace_text', { options: { search: { query: 'Alpha' }, replacement: 'Omega', replace_all: true } });
    await mutate('format_text', { slide_id: 'slide-1', id: 'text', start: 0, end: 5, style: { italic: true } });
    await mutate('update_paragraphs', { slide_id: 'slide-1', id: 'text', paragraphs: [{ runs: [{ text: 'Rich', style: { bold: true } }], line_spacing: { kind: 'percent', value: 120000 }, tabs: [{ position: 1000 }] }] });
    await mutate('update_text', { slide_id: 'slide-1', element_id: 'text', text: 'Rich preserved' });
    assert.equal((await call('get_deck', { deck_id })).slides[0].elements[0].format.paragraphs[0].runs[0].style.bold, true);
    const before = await call('get_document', { deck_id });
    const copied = await mutate('edit_selection', { slide_id: 'slide-1', operation: { op: 'copy', ids: ['text'], format: 'keep_source_formatting' } });
    assert.equal(revision, before.revision);
    await mutate('edit_selection', { slide_id: 'slide-1', operation: { op: 'paste', id_prefix: 'pasted', dx: 0, dy: 0 }, clipboard: copied.clipboard });
    await call('undo', { deck_id });
    assert.equal((await call('get_document', { deck_id })).hash, before.hash);
    revision = (await call('get_document', { deck_id })).revision;
    const malformed = await client.callTool({ name: 'format_text', arguments: { deck_id, expected_revision: revision, slide_id: 'slide-1', id: 'text', start: 0, end: 1, style: { invented: true } } });
    assert.equal(malformed.isError, true);
    const invalidBundle = structuredClone(copied.clipboard); invalidBundle.elements[0].format.paragraphs[0].runs[0].style.invented = true;
    assert.equal((await client.callTool({ name: 'edit_selection', arguments: { deck_id, expected_revision: revision, slide_id: 'slide-1', operation: { op: 'paste', id_prefix: 'invalid', dx: 0, dy: 0 }, clipboard: invalidBundle } })).isError, true);
    assert.equal((await call('get_document', { deck_id })).hash, before.hash);
    await mutate('add_object', { slide_id: 'slide-1', id: 'table', kind: 'table', rows: 2, columns: 2 });
    await mutate('edit_table', { slide_id: 'slide-1', id: 'table', operations: [{ op: 'set_cell_style', row: 1, column: 1, style: { fill: '00FF00', text_style: { italic: true } } }] });
    await mutate('resize_canvas', { width: 1600, height: 900, mode: 'keep' });
    assert.equal((await call('get_deck', { deck_id })).width, 1600);
    for (const kind of ['potx', 'thmx']) {
      await call('export_template', { deck_id, kind, filename: `synthetic.${kind}` });
      const bytes = await readFile(join(directory, `synthetic.${kind}`));
      assert.equal(bytes.subarray(0, 2).toString(), 'PK');
      const opened = await call('import_template', { kind, base64: bytes.toString('base64') });
      assert.equal(opened.revision, 0);
      assert.equal((await client.callTool({ name: 'export_template', arguments: { deck_id, kind, filename: `synthetic.${kind}` } })).isError, true);
      assert.deepEqual(await readFile(join(directory, `synthetic.${kind}`)), bytes);
    }
    for (const filename of ['../escape.potx', 'wrong.pptx', 'con.potx', 'wrong.thmx']) {
      assert.equal((await client.callTool({ name: 'export_template', arguments: { deck_id, kind: 'potx', filename } })).isError, true);
    }
    const report = { title: 'Synthetic chart', subtitle: '', period: '', source: 'Synthetic fixture', sections: [{ title: 'Bubble', layout: 'chart', chart: { kind: 'bubble', categories: ['1', '2'], series: [{ name: 'Synthetic', values: [2, 3], color: '0017C1', bubble_sizes: [1, 2] }] } }] };
    const chart = await call('compile_report', { report });
    const chartDeck = await call('get_deck', { deck_id: chart.deck_id });
    const chartIndex = chartDeck.slides[0].elements.findIndex((element) => element.type === 'chart');
    assert.equal(chartDeck.slides[0].elements[chartIndex].kind, 'bubble');
    for (const kind of ['funnel', 'waterfall']) {
      const created = await call('create_presentation', { title: 'Synthetic chartEx' });
      await call('add_object', { deck_id: created.deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'extended-chart', kind: 'chart', preset: kind });
      const content = (await call('get_deck', { deck_id: created.deck_id })).slides[0].elements.find((element) => element.type === 'chart');
      assert.equal(content.kind, kind);
      const copied = await call('edit_selection', { deck_id: created.deck_id, expected_revision: 1, slide_id: 'slide-1', operation: { op: 'copy', ids: [content.id], format: 'keep_source_formatting' } });
      await call('edit_selection', { deck_id: created.deck_id, expected_revision: 1, slide_id: 'slide-1', operation: { op: 'paste', id_prefix: 'extended-copy', dx: 0, dy: 0 }, clipboard: copied.clipboard });
      if (kind === 'waterfall') {
        assert.deepEqual(content.options.waterfall_totals, [0, 3]);
        assert.deepEqual(content.series[0].values, [10, 15, -7, 18]);
        for (const indices of [[-1], [32], [0.5], [0, 0]]) {
          copied.clipboard.elements[0].options.waterfall_totals = indices;
          assert.equal((await client.callTool({ name: 'edit_selection', arguments: { deck_id: created.deck_id, expected_revision: 2, slide_id: 'slide-1', operation: { op: 'paste', id_prefix: 'invalid-total', dx: 0, dy: 0 }, clipboard: copied.clipboard } })).isError, true);
        }
      } else {
        const extended = { ...report, sections: [{ title: kind, layout: 'chart', chart: { kind, categories: ['Start', 'End'], series: [{ name: 'Synthetic native', values: [120,40], color: '087F73' }] } }] };
        const compiled = await call('compile_report', { report: extended });
        assert.equal((await call('get_deck', { deck_id: compiled.deck_id })).slides[0].elements.find((element) => element.type === 'chart').kind, kind);
      }
    }
    const options = { primary_axis: { min: 0 }, legend: 'top', data_labels: { show_value: true } };
    await call('apply_transaction', { deck_id: chart.deck_id, expected_revision: 0, operations: [{ op: 'add', path: `/deck/slides/0/elements/${chartIndex}/options`, value: options }] });
    const chartCopy = await call('edit_selection', { deck_id: chart.deck_id, expected_revision: 1, slide_id: 'slide-1', operation: { op: 'copy', ids: [chartDeck.slides[0].elements[chartIndex].id], format: 'keep_source_formatting' } });
    assert.deepEqual(chartCopy.clipboard.elements[0].options, { ...options, data_labels: { show_value: true, show_category_name: false, show_series_name: false, show_percent: false } });
    chartCopy.clipboard.elements[0].options.invented = true;
    assert.equal((await client.callTool({ name: 'edit_selection', arguments: { deck_id: chart.deck_id, expected_revision: 1, slide_id: 'slide-1', operation: { op: 'paste', id_prefix: 'badchart', dx: 0, dy: 0 }, clipboard: chartCopy.clipboard } })).isError, true);
    report.sections[0].chart.invented = true;
    assert.equal((await client.callTool({ name: 'compile_report', arguments: { report } })).isError, true);
    await mutate('add_asset', { slide_id: 'slide-1', id: 'image', size: 32, alt: 'Synthetic image', mime_type: 'image/svg+xml', base64: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>').toString('base64') });
    const beforeImage = await call('get_document', { deck_id });
    const picture = beforeImage.deck.slides[0].elements.find((element) => element.id === 'image');
    const image = await call('edit_image', { base64: picture.base64, mime_type: picture.mime_type, params: { grayscale: true, resize_longest_side: 8 } });
    assert.equal(image.width, 8);
    assert.equal((await call('get_document', { deck_id })).hash, beforeImage.hash);
    await mutate('apply_image_edit', { slide_id: 'slide-1', id: 'image', image });
    await call('undo', { deck_id });
    assert.equal((await call('get_document', { deck_id })).hash, beforeImage.hash);
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});

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
    const catalog = await call('object_catalog'); assert.equal(catalog.charts.length, 24);
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

test('phase6 MCP modern threads and masked manual candidates use strict typed operations', async () => {
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
  const client = new Client({ name: 'phase6-review', version: '1.0.0' });
  const call = async (name, args = {}) => { const result = await client.callTool({ name, arguments: args }); assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content)}`); return result.structuredContent; };
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    assert.equal(tools.find(tool => tool.name === 'modern_comment')?.annotations.readOnlyHint, false);
    assert.equal(tools.find(tool => tool.name === 'set_table_headers')?.annotations.readOnlyHint, false);
    assert.equal((await call('authoring_capabilities')).review.modern_powerpoint_threads, true);
    const { deck_id } = await call('create_presentation', { title: 'Synthetic Phase 6' });
    const draft = { author_name: 'Local', created: '2026-09-19T10:00:00Z', body: [{ runs: [{ text: 'SENTINEL@example.invalid' }] }] };
    const result = await call('modern_comment', { deck_id, slide_id: 'slide-1', expected_revision: 0, operation: { type: 'create', draft, anchor: { kind: 'unknown' } } });
    const deck = await call('get_deck', { deck_id });
    assert.equal(deck.slides[0].review.modern_threads[0].status, 'active');
    const inspection = await call('inspect_document', { deck_id });
    assert.ok(inspection.candidates.some(candidate => candidate.rule === 'email_candidate'));
    assert.equal(JSON.stringify(inspection).includes('SENTINEL'), false);
    assert.equal((await client.callTool({ name: 'modern_comment', arguments: { deck_id, slide_id: 'slide-1', expected_revision: result.revision, operation: { type: 'create', draft: { ...draft, raw_xml: '<x/>' }, anchor: { kind: 'unknown' } } } })).isError, true);
    assert.equal((await client.callTool({ name: 'export_clean_copy', arguments: { deck_id, expected_revision: result.revision, categories: ['email_candidate'], confirmed: true, filename: 'candidate.pptx' } })).isError, true);
  } finally { await client.close(); await transport.close(); }
});

test('MCP review schema and local lifecycle preserve native visuals and explicitly confirmed clean copies', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-review-mcp-'));
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
  const client = new Client({ name: 'review-proof', version: '1.0.0' });
  const call = async (name, args = {}) => { const result = await client.callTool({ name, arguments: args }); assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content)}`); return result.structuredContent; };
  const rejected = async (name, args) => assert.equal((await client.callTool({ name, arguments: args })).isError, true, `${name} must reject`);
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    for (const name of ['format_text_element', 'replace_element_text', 'check_accessibility', 'inspect_document']) assert.equal(tools.find((tool) => tool.name === name)?.annotations.readOnlyHint, true);
    for (const name of ['add_comment', 'reply_comment', 'resolve_comment', 'remove_comment', 'refresh_fields', 'set_accessibility', 'set_reading_order', 'update_notes', 'export_clean_copy', 'replace_deck']) assert.equal(tools.find((tool) => tool.name === name)?.annotations.readOnlyHint, false);
    const capabilities = await call('authoring_capabilities');
    assert.equal(capabilities.review.modern_powerpoint_threads, true);
    assert.equal(capabilities.review.authenticated_authors, false);
    assert.deepEqual(capabilities.review.field_kinds, ['slidenum', 'datetime1']);
    const { deck_id } = await call('create_presentation', { title: 'Synthetic review MCP' });
    let revision = 0;
    const mutate = async (name, input) => { const result = await call(name, { deck_id, expected_revision: revision, ...input }); revision = result.revision; return result; };
    const picture = await call('create_asset', { id: 'vector', mime_type: 'image/svg+xml', size: 64, alt: 'Synthetic red square', base64: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="red"/></svg>').toString('base64') });
    assert.equal(typeof picture.svg, 'string');
    const deck = await call('get_deck', { deck_id });
    const text = { type: 'text', id: 'text', x: 40, y: 40, width: 800, height: 80, text: 'Alpha Beta', font_size: 24, color: '000000', bold: false };
    const gradient = { kind: 'linear', angle: 45, stops: [{ offset: 0, color: 'FF0000', opacity: 1 }, { offset: 1, color: '0000FF', opacity: 0.5 }] };
    const shape = { type: 'rect', id: 'gradient', x: 40, y: 180, width: 200, height: 100, fill: 'FF0000', visual: { gradient } };
    const curve = { type: 'polygon', id: 'curve', x: 300, y: 200, width: 200, height: 100, points: [[0, 0], [0.5, 1], [1, 0]], fill: '00FF00', stroke: '000000', stroke_width: 1, visual: { path: { commands: [{ op: 'move', point: [0, 0] }, { op: 'quadratic', control: [0.5, 1], point: [1, 0] }, { op: 'close' }] } } };
    deck.slides[0].elements = [text, shape, picture, curve];
    deck.slides[0].review = { accessibility: { gradient: { decorative: true } } };
    await mutate('replace_deck', { deck });
    const immutable = await call('format_text_element', { element: text, start: 0, end: 5, style: { italic: true } });
    assert.equal((await call('replace_element_text', { element: immutable, text: 'Alpha Gamma' })).format.paragraphs[0].runs[0].style.italic, true);
    assert.equal((await call('get_document', { deck_id })).revision, revision);
    for (const invalid of [
      { ...deck, width: 319 }, { ...deck, height: 4097 },
      { ...deck, slides: [{ ...deck.slides[0], review: { invented: true } }] },
      { ...deck, slides: [{ ...deck.slides[0], elements: [{ ...shape, visual: { gradient, script: 'not-executed' } }] }] },
      { ...deck, slides: [{ ...deck.slides[0], elements: [{ ...picture, svg: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"><script>not-executed</script></svg>').toString('base64') }] }] },
    ]) await rejected('replace_deck', { deck_id, expected_revision: revision, deck: invalid });
    const field = { id: '{00112233-4455-6677-8899-aabbccddeeff}', kind: 'slidenum' };
    const paragraphs = [{ runs: [{ text: 'Page ' }, { text: '9', field }] }];
    for (const invalidField of [{ ...field, kind: '' }, { ...field, kind: 'invalid\u0000kind' }, { ...field, kind: 'x'.repeat(129) }, { ...field, id: '00112233-4455-6677-8899-aabbccddeefx' }, { ...field, extension: true }]) {
      await rejected('update_paragraphs', { deck_id, expected_revision: revision, slide_id: 'slide-1', id: 'text', paragraphs: [{ runs: [{ text: '9', field: invalidField }] }] });
    }
    const unknownField = { ...field, kind: 'vendor-evaluate' };
    await mutate('update_paragraphs', { slide_id: 'slide-1', id: 'text', paragraphs: [{ runs: [{ text: 'Unchanged cache', field: unknownField }] }] });
    const beforeUnknown = await call('get_document', { deck_id });
    await mutate('refresh_fields', { reference_date: '2026-09-17' });
    assert.equal((await call('get_document', { deck_id })).hash, beforeUnknown.hash);
    assert.equal((await call('get_deck', { deck_id })).slides[0].elements[0].text, 'Unchanged cache');
    assert.deepEqual((await call('get_deck', { deck_id })).slides[0].elements[0].format.paragraphs[0].runs[0].field, unknownField);
    await mutate('update_paragraphs', { slide_id: 'slide-1', id: 'text', paragraphs });
    const beforeField = await call('get_document', { deck_id });
    await mutate('refresh_fields', { reference_date: '2026-09-17' });
    assert.equal((await call('get_deck', { deck_id })).slides[0].elements[0].text, 'Page 1');
    assert.deepEqual((await call('get_deck', { deck_id })).slides[0].elements[0].format.paragraphs[0].runs[1].field, field);
    revision = (await call('undo', { deck_id })).revision;
    assert.equal((await call('get_document', { deck_id })).hash, beforeField.hash);
    await mutate('refresh_fields', { reference_date: '2026-09-17' });
    await mutate('update_notes', { slide_id: 'slide-1', notes: 'PRIVATE_SYNTHETIC_NOTES' });
    const comment = { id: 'first', author: 'PRIVATE_SYNTHETIC_AUTHOR', initials: 'SA', timestamp: '2026-09-17T10:00:00Z', text: 'PRIVATE_SYNTHETIC_COMMENT' };
    await mutate('add_comment', { slide_id: 'slide-1', comment });
    await mutate('reply_comment', { slide_id: 'slide-1', parent_id: 'first', comment: { ...comment, id: 'reply' } });
    await mutate('resolve_comment', { slide_id: 'slide-1', comment_id: 'first', resolved: true });
    await mutate('set_accessibility', { slide_id: 'slide-1', element_id: 'text', metadata: { title: 'Synthetic title', description: 'Synthetic description' } });
    const ordered = await mutate('set_reading_order', { slide_id: 'slide-1', order: ['gradient', 'text', 'vector', 'curve'] });
    assert.match(ordered.warnings.join(' '), /z-order/);
    assert.equal((await call('check_accessibility', { deck_id })).wcag_certified, false);
    const inspection = await call('inspect_document', { deck_id });
    assert.equal(inspection.complete_personal_data_detection, false);
    assert.equal(JSON.stringify(inspection).includes('PRIVATE_SYNTHETIC'), false);
    await call('export_pptx', { deck_id, filename: 'review-source.pptx' });
    const sourceBytes = await readFile(join(directory, 'review-source.pptx'));
    const opened = await call('open_pptx', { base64: sourceBytes.toString('base64') });
    const restored = await call('get_deck', { deck_id: opened.deck_id });
    assert.deepEqual(restored.slides[0].elements.find((element) => element.id === 'gradient').visual.gradient, gradient);
    assert.equal(restored.slides[0].elements.find((element) => element.id === 'vector').svg, picture.svg);
    assert.deepEqual(restored.slides[0].elements.find((element) => element.id === 'curve').visual.path, curve.visual.path);
    assert.equal(restored.slides[0].review.comments[1].parent_id, 'first');
    assert.equal(restored.slides[0].review.comments[0].resolved, true);
    await call('resolve_comment', { deck_id: opened.deck_id, expected_revision: 0, slide_id: 'slide-1', comment_id: 'first', resolved: false });
    assert.equal((await call('get_deck', { deck_id: opened.deck_id })).slides[0].review.comments[0].resolved, false);
    const sourceState = await call('get_document', { deck_id });
    const cleanInput = { deck_id, expected_revision: revision, categories: ['notes'], confirmed: true, filename: 'review-clean.pptx' };
    for (const invalid of [{ ...cleanInput, confirmed: false }, { ...cleanInput, categories: [] }, { ...cleanInput, categories: ['unknown'] }, { ...cleanInput, filename: '../outside.pptx' }, { ...cleanInput, expected_revision: revision - 1 }]) await rejected('export_clean_copy', invalid);
    const clean = await call('export_clean_copy', cleanInput);
    assert.notEqual(clean.new_document_id, sourceState.id);
    assert.equal(clean.document, undefined); assert.equal(clean.base64, undefined);
    const cleanBytes = await readFile(join(directory, 'review-clean.pptx'));
    await rejected('export_clean_copy', cleanInput);
    assert.deepEqual(await readFile(join(directory, 'review-clean.pptx')), cleanBytes);
    assert.deepEqual(await readFile(join(directory, 'review-source.pptx')), sourceBytes);
    assert.equal((await call('get_document', { deck_id })).hash, sourceState.hash);
    assert.equal((await call('get_document', { deck_id })).revision, sourceState.revision);
    const cleanOpened = await call('open_pptx', { base64: cleanBytes.toString('base64') });
    const cleanDeck = await call('get_deck', { deck_id: cleanOpened.deck_id });
    assert.equal(cleanDeck.slides[0].notes, '');
    assert.equal(cleanDeck.slides[0].review.comments.length, 2);
    await mutate('remove_comment', { slide_id: 'slide-1', comment_id: 'first' });
    assert.equal((await call('get_deck', { deck_id })).slides[0].review.comments, undefined);
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});