import test from 'node:test';
import assert from 'node:assert/strict';
import { AislideClient, DocumentSession } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';

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
  const spec = catalog.examples[0].spec;
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
  await reopened.session.updateGraph('slide', { id: 'graph', spec });
  await assert.rejects(() => reopened.session.applyGraph('slide', { id: 'graph', operations: [{ op: 'remove', ids: ['user'] }] }, { expectedRevision: 0 }), /Revision conflict/);
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