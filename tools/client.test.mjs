import test from 'node:test';
import assert from 'node:assert/strict';
import { AislideClient, DocumentSession } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';
import { guidedExamples } from './guided-demo.mjs';

test('guided SDK profiles create editable documents and reject early or late cancellation', async () => {
  const client = new AislideClient(requestCore);
  assert.equal((await client.bestPracticeProfiles()).profiles.length, 4);
  for (const input of guidedExamples()) {
    assert.equal((await client.bestPracticeGuide(input.profile_id)).language, 'en');
    const result = await client.createGuidedPresentation(`guided-${input.profile_id}`, input);
    assert.equal(result.validation.ready, true);
    assert.equal(result.validation.semantic_truth_verified, false);
    assert.equal(result.session.document.deck.slides.length, input.slides.length);
    const snapshot = result.session.document; snapshot.deck.title = 'Changed copy';
    assert.equal(result.session.document.deck.title, input.title);
    assert.equal(result.session.canUndo, false);
    const saved = await result.session.exportPresentation();
    const opened = await client.openPresentation(`reopen-${input.profile_id}`, saved.base64);
    assert.ok(opened.session.document.parts.every((part) => !part.stale));
  }
  let calls = 0;
  let finish;
  const response = new Promise((resolveReply) => { finish = resolveReply; });
  const delayed = new AislideClient(async () => { calls += 1; return response; });
  const early = new AbortController(); early.abort();
  await assert.rejects(() => delayed.createGuidedPresentation('early', guidedExamples()[0], { signal: early.signal }), /cancelled/i);
  assert.equal(calls, 0);
  const late = new AbortController();
  const pending = delayed.createGuidedPresentation('late', guidedExamples()[0], { signal: late.signal });
  const rejected = assert.rejects(pending, /cancelled/i);
  late.abort(); finish({ document: { version: 1, id: 'late', revision: 0, hash: 'unused' }, validation: { ready: true } });
  await rejected;
  assert.equal(calls, 1);
});

test('design preset SDK shares native layouts, preservation, revision guards and undo', async () => {
  const client = new AislideClient(requestCore);
  const presets = await client.designPresets();
  assert.equal(presets.length, 7);
  for (const preset of presets) {
    const session = await client.createPresentation(`preset-${preset.id}`, 'Preset example');
    const original = session.document;
    await session.applyDesignPreset(preset.id, { expectedRevision: 0 });
    assert.equal(session.document.deck.design.masters.length, 2);
    await assert.rejects(() => session.applyDesignPreset(preset.id, { expectedRevision: 0 }), /Revision conflict/);
    await session.undo();
    assert.equal(session.document.hash, original.hash);
    await session.redo();
    await session.assignLayout('slide-1', 'preset-cover');
    await session.editSlides(preset.design.layouts.filter((layout) => layout.id !== 'preset-cover').map((layout) => ({ op: 'insert', id: layout.id, title: layout.name, layout_id: layout.id })));
    const exported = await session.exportPresentation();
    const opened = (await client.openPresentation(`reopen-${preset.id}`, exported.base64)).session;
    assert.equal(opened.document.deck.design.theme.colors.accent1, preset.design.theme.colors.accent1);
    assert.equal(opened.document.deck.slides[0].layout_id, 'preset-cover');
    assert.equal(opened.document.deck.design.layouts.length, 11);
    assert.equal(opened.document.deck.slides.length, 7);
    await opened.applyDesignPreset('minimal');
    assert.equal(opened.document.deck.slides[0].elements.find((entry) => entry.id === 'title').text, 'Presentation title');
  }
});

test('new slides use the applied preset blank layout without adding sample text', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('preset-new-slide');
  await session.applyDesignPreset('public');
  await session.editSlides([{ op: 'insert', id: 'next', after: 'slide-1', title: 'Next slide' }]);
  assert.equal(session.document.deck.slides[1].layout_id, 'preset-blank');
  assert.deepEqual(session.document.deck.slides[1].elements, []);
});

test('design preset visual regions place new parts without moving body text', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('preset-part-layout', 'Visual layout');
  await session.applyDesignPreset('minimal');
  await session.assignLayout('slide-1', 'preset-visual-content');
  const body = session.document.deck.slides[0].elements.find((entry) => entry.id === 'body');
  const region = session.document.deck.design.layouts.find((entry) => entry.id === 'preset-visual-content').elements.find((entry) => entry.id === 'preset-visual-region');
  const catalog = await client.partCatalog();
  const spec = catalog.presets.find((entry) => entry.id === 'flow/balanced').example;
  await session.addPart('slide-1', { id: 'visual-part', spec });
  const part = session.document.deck.slides[0].elements.find((entry) => entry.id === 'visual-part');
  assert.ok(part.x >= region.x && part.y >= region.y);
  assert.ok(part.x + part.width <= region.x + region.width + 0.01);
  assert.ok(part.y + part.height <= region.y + region.height + 0.01);
  assert.equal(part.view_width, part.width);
  assert.equal(part.view_height, part.height);
  const labels = part.children.filter((entry) => entry.type === 'text');
  assert.ok(labels[0].y + labels[0].font_size * 1.35 < labels[1].y);
  assert.deepEqual(session.document.deck.slides[0].elements.find((entry) => entry.id === 'body'), body);
  const exported = await session.exportPresentation();
  const opened = (await client.openPresentation('visual-part-reopened', exported.base64)).session;
  assert.equal(opened.document.parts[0].stale, false);
  await opened.updatePart('slide-1', { id: 'visual-part', spec: { ...spec, title: 'Edited visual' } });
  const updated = opened.document.deck.slides[0].elements.find((entry) => entry.id === 'visual-part');
  assert.equal(updated.x, part.x);
  assert.ok(Math.abs(updated.view_width - updated.width) < 0.001);
  assert.ok(Math.abs(updated.children[0].font_size - labels[0].font_size) < 0.01);
});

test('editor SDK creates blank files, edits native slides and inserts SVG assets with undo', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('editor-sdk', 'New presentation');
  assert.equal(session.document.deck.slides.length, 1);
  assert.deepEqual(session.document.deck.slides[0].elements, []);
  await session.editSlides([{ op: 'insert', id: 'asset-slide', after: 'slide-1', title: 'Icons' }]);
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 4h16v16H4z" fill="#0017c1"/></svg>';
  await session.addAsset('asset-slide', { id: 'figma-icon', base64: Buffer.from(svg).toString('base64'), mime_type: 'image/svg+xml', alt: 'Provided icon', size: 96 });
  const saved = await session.exportPresentation();
  const reopened = (await client.openPresentation('editor-reopen', saved.base64)).session;
  assert.equal(reopened.document.deck.slides[1].id, 'asset-slide');
  assert.equal(reopened.document.deck.slides[1].elements[0].mime_type, 'image/png');
  await reopened.editSlides([{ op: 'duplicate', slide_id: 'asset-slide', id: 'copied-slide' }, { op: 'move', slide_id: 'copied-slide', index: 0 }]);
  assert.equal(reopened.document.deck.slides[0].id, 'copied-slide');
  await assert.rejects(() => reopened.editSlides([{ op: 'remove', slide_id: 'asset-slide' }], { expectedRevision: 0 }), /Revision conflict/);
  await reopened.undo();
  assert.equal((await reopened.exportPresentation()).base64, saved.base64);
});

test('graph SDK creates, edits and reopens native graph metadata with undo', async () => {
  const client = new AislideClient(requestCore);
  const catalog = await client.graphCatalog();
  const spec = structuredClone(catalog.examples[0].spec);
  const picture = await client.createGraphIcon({ mime_type: 'image/svg+xml', base64: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><circle cx="12" cy="12" r="10" fill="#0017c1"/></svg>').toString('base64'), alt: 'API icon' });
  spec.nodes[1].icon = { base64: picture.base64, mime_type: picture.mime_type, alt: picture.alt };
  const session = await client.createDocument({ id: 'graph-sdk', deck: { version: 1, title: 'Graph', width: 1280, height: 720, slides: [{ id: 'slide', title: 'Graph', background: '@lt1', notes: '', elements: [] }] } });
  await session.addGraph('slide', { id: 'graph', spec });
  const original = session.document;
  await session.applyGraph('slide', { id: 'graph', operations: [{ op: 'move', ids: ['api'], dx: 0, dy: 64 }] }, { expectedRevision: 1 });
  assert.equal(session.document.parts[0].spec.data.graph.nodes[1].y, 284);
  await session.undo(); assert.equal(session.document.hash, original.hash);
  await session.redo();
  const exported = await session.exportPresentation();
  const reopened = await client.openPresentation('graph-sdk-open', exported.base64);
  assert.equal(reopened.session.document.parts[0].stale, false);
  assert.deepEqual(reopened.session.document.parts[0].spec.data.graph.nodes[1].icon, spec.nodes[1].icon);
  assert.equal(reopened.session.document.deck.slides[0].elements[0].children.filter((element) => element.type === 'picture').length, 1);
  await reopened.session.updateGraph('slide', { id: 'graph', spec });
  await assert.rejects(() => reopened.session.applyGraph('slide', { id: 'graph', operations: [{ op: 'remove', ids: ['user'] }] }, { expectedRevision: 0 }), /Revision conflict/);
  await reopened.session.applyGraph('slide', { id: 'graph', operations: [{ op: 'put_node', node: { ...spec.nodes[1], icon: null } }] });
  assert.equal(reopened.session.document.deck.slides[0].elements[0].children.filter((element) => element.type === 'picture').length, 0);
  await reopened.session.undo();
  assert.deepEqual(reopened.session.document.parts[0].spec.data.graph.nodes[1].icon, spec.nodes[1].icon);
});

test('metadata parts share catalog, revisioned updates, undo and single PPTX persistence', async () => {
  const client = new AislideClient(requestCore);
  const catalog = await client.partCatalog();
  assert.equal(catalog.presets.length, 108);
  const spec = catalog.presets.find((preset) => preset.id === 'vertical-bar-graph/balanced').example;
  const deck = { version: 1, title: 'Parts', width: 1280, height: 720, slides: [{ id: 'slide', title: 'Parts', background: 'FFFFFF', notes: 'Synthetic fixture', elements: [] }] };
  const session = await client.createDocument({ id: 'parts-sdk', deck });
  await session.addPart('slide', { id: 'part', spec });
  const inserted = session.document;
  const updated = structuredClone(spec); updated.title = 'Edited part';
  await session.updatePart('slide', { id: 'part', spec: updated }, { expectedRevision: 1 });
  assert.equal(session.document.parts[0].spec.title, 'Edited part');
  await assert.rejects(() => session.updatePart('slide', { id: 'part', spec }, { expectedRevision: 0 }), /Revision conflict/);
  await session.undo(); assert.deepEqual(session.document.deck, inserted.deck);
  await session.redo();
  const exported = await session.exportPresentation();
  const reopened = await client.openPresentation('reopened-parts', exported.base64);
  assert.equal(reopened.session.document.parts[0].stale, false);
  await reopened.session.updatePart('slide', { id: 'part', spec });
  assert.equal(reopened.session.document.parts[0].spec.title, spec.title);
});

test('fractional part geometry remains stable through JSON transport', async () => {
  const client = new AislideClient(requestCore);
  const catalog = await client.partCatalog();
  const preset = catalog.presets.find((preset) => preset.id === 'water-fall/balanced');
  const session = await client.createDocument({ id: 'parts-fractions', deck: { version: 1, title: 'Fractions', width: 1280, height: 720, slides: [{ id: 'slide', title: 'Fractions', background: '@lt1', notes: 'Synthetic fixture', elements: [] }] } });
  await session.addPart('slide', { id: 'part', spec: preset.example });
  await session.updatePart('slide', { id: 'part', spec: { ...preset.example, title: 'Updated fractions' } });
  assert.equal(session.document.parts[0].stale, false);
  const exported = await session.exportPresentation();
  const reopened = await client.openPresentation('fraction-reopen', exported.base64);
  assert.equal(reopened.session.document.parts[0].stale, false);
  assert.equal((await reopened.session.exportPresentation()).base64, exported.base64);
});

test('one PPTX opens, edits and saves without a checkpoint file', async () => {
  const client = new AislideClient(requestCore);
  const compiled = await client.request({ op: 'compile', report: await client.request({ op: 'sample' }) });
  const session = await client.createDocument({ id: 'single-pptx-client', deck: compiled.deck });
  const exported = await session.exportPresentation();
  assert.equal(exported.checkpoint, undefined);
  const opened = await client.openPresentation('single-pptx-reopened', exported.base64);
  assert.equal(opened.session.document.deck.slides.length, 12);
  assert.equal(opened.session.document.origin.native, true);
  const title = opened.session.document.deck.slides[0].elements.findIndex((element) => element.id === 'title');
  const original = opened.session.document;
  await opened.session.transact([{ op: 'replace', path: `/deck/slides/0/elements/${title}/text`, value: 'One editable PPTX\nNo sidecar' }]);
  const updated = await opened.session.exportPresentation();
  const verified = await client.openPresentation('edited-single-pptx', updated.base64);
  assert.equal(verified.session.document.deck.slides[0].elements[title].text, 'One editable PPTX\nNo sidecar');
  await opened.session.undo();
  assert.equal(opened.session.document.hash, original.hash);
  await assert.rejects(() => client.openPresentation('protected', Buffer.from([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]).toString('base64')), /encrypted.*legacy/);
});

test('authoring APIs share revision guards, theme inheritance and undo', async () => {
  const client = new AislideClient(requestCore);
  const design = await client.designDefaults();
  const catalog = await client.objectCatalog();
  assert.equal(catalog.shapes.length, 40);
  const deck = { version: 1, title: 'API authoring', width: 1280, height: 720, slides: [{ id: 'slide-1', title: 'API', background: 'FFFFFF', notes: 'Synthetic API fixture', elements: [] }] };
  const session = await client.createDocument({ id: 'authoring-api', deck });
  await session.updateDesign(design, { expectedRevision: 0 });
  await session.assignLayout('slide-1', 'title-content', { expectedRevision: 1 });
  await session.addObject('slide-1', { id: 'native-ellipse', kind: 'shape', preset: 'ellipse' }, { expectedRevision: 2 });
  assert.equal(session.document.deck.slides[0].elements.at(-1).preset, 'ellipse');
  const theme = structuredClone(design.theme); theme.colors.accent1 = 'B53055';
  await session.applyTheme(theme, { expectedRevision: 3 });
  assert.equal(session.document.deck.design.theme.colors.accent1, 'B53055');
  await assert.rejects(() => session.assignLayout('slide-1', 'blank', { expectedRevision: 0 }), /Revision conflict/);
  assert.equal(session.revision, 4);
  await session.undo();
  assert.equal(session.document.deck.design.theme.colors.accent1, design.theme.colors.accent1);
  const exported = await session.exportProject();
  assert.equal((await client.openProject(exported)).document.hash, session.document.hash);
});

test('portable client ingests, edits, undoes and restores a source-bound project without a GUI', async () => {
  const client = new AislideClient(requestCore);
  const source = await client.ingest({ format: 'csv', name: 'fixture.csv', base64: Buffer.from('Quarter,Value\nQ1,-2\nQ2,0\nQ3,12\n').toString('base64') });
  const result = await client.dataReport(source, { title: 'Provided values', period: 'Fixture', table_index: 0, category_column: 0, value_columns: [1], row_start: 0, row_count: 3, chart_kind: 'column' });
  const session = await client.createDocument({ id: 'client-fixture', deck: result.compiled.deck, report: result.report, sources: [source], bindings: result.bindings });
  const snapshot = session.document;
  snapshot.deck.title = 'External mutation';
  assert.equal(session.document.deck.title, 'Provided values');
  const index = session.document.deck.slides[3].elements.findIndex((element) => element.type === 'chart');
  await session.transact([{ op: 'replace', path: `/deck/slides/3/elements/${index}/x`, value: 80 }]);
  assert.equal(session.revision, 1);
  await assert.rejects(() => session.transact([{ op: 'replace', path: '/deck/title', value: 'Stale' }], { expectedRevision: 0 }));
  assert.equal(session.revision, 1);
  await session.undo();
  assert.equal(session.document.deck.slides[3].elements[index].x, 64);
  await session.redo();
  assert.equal(session.document.deck.slides[3].elements[index].x, 80);
  const bundle = await session.exportProject();
  assert.equal(Buffer.from(bundle.base64, 'base64').subarray(0, 2).toString(), 'PK');
  const restored = await client.openProject(bundle);
  assert.deepEqual(restored.document, session.document);
  assert.equal(restored.document.sources[0].sha256, source.sha256);
  const controller = new AbortController(); controller.abort();
  const before = session.document;
  await assert.rejects(() => session.transact([{ op: 'replace', path: '/deck/title', value: 'Cancelled' }], { signal: controller.signal }));
  assert.deepEqual(session.document, before);
});

test('a late transport success after cancellation never commits session state', async () => {
  const initial = { version: 1, id: 'cancellation-fixture', revision: 0, hash: 'before', deck: { title: 'Before' }, sources: [], bindings: [] };
  let finish;
  const pendingReply = new Promise((resolveReply) => { finish = resolveReply; });
  const session = new DocumentSession(async () => pendingReply, initial);
  const controller = new AbortController();
  const pending = session.transact([{ op: 'replace', path: '/deck/title', value: 'After' }], { signal: controller.signal });
  const rejected = assert.rejects(pending, /cancelled/i);
  controller.abort();
  finish({ document: { ...initial, revision: 1, hash: 'after', deck: { title: 'After' } }, receipt: { document_id: initial.id, after_hash: 'after', inverse: [] } });
  await rejected;
  assert.deepEqual(session.document, initial);
  assert.equal(session.canUndo, false);
  assert.equal(session.busy, false);
});