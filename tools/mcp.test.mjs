import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile, readdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { guidedExamples } from './guided-demo.mjs';

test('P1 MCP finalization publishes a traceable new bundle without changing the session', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-finalize-mcp-'));
  const client = new Client({ name: 'finalization-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
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

test('P0 MCP guided options, diagnostics and staged revisions form a guarded visual loop', async () => {
  const client = new Client({ name: 'authoring-loop-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
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
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
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
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
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
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
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
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
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
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
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
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
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
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
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
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
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