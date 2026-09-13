import test from 'node:test';
import assert from 'node:assert/strict';
import { AislideClient, DocumentSession } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';

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