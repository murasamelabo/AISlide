# AISlide Client

This private, transport-independent client is shared by Studio and MCP. Rust owns schemas, layouts, transactions, validation, and PPTX behavior; the client only manages transport and bounded session history. It is not published to npm.

```js
import { AislideClient } from './packages/client/index.mjs';
import { requestCore } from './tools/core-client.mjs';

const client = new AislideClient(requestCore);
const source = await client.ingest({
  name: 'provided.csv', format: 'csv',
  base64: Buffer.from('Quarter,Value\nQ1,10\nQ2,12\n').toString('base64'),
});
const result = await client.dataReport(source, {
  title: 'Provided values', period: 'Synthetic example', table_index: 0,
  category_column: 0, value_columns: [1], row_start: 0, row_count: 2,
  chart_kind: 'column',
});
const session = await client.createDocument({
  id: crypto.randomUUID(), deck: result.compiled.deck, report: result.report,
  sources: [source], bindings: result.bindings,
});
await session.transact([
  { op: 'replace', path: '/deck/slides/0/background', value: 'EDF3F0' },
], { expectedRevision: session.revision });
await session.undo();
const exported = await session.exportProject();
const restored = await client.openProject(exported);
console.log(restored.revision, restored.document.sources[0].sha256);
```

The transport is `(request, { signal }?) => Promise<JSON>`. The Node bridge runs the fixed compiled CLI without a shell. Studio supplies its same-origin development HTTP or Tauri IPC transport. An aborted or failed request never commits client state; calls are serialized per session.

`session.document` returns a clone. `transact()` uses RFC 6902 JSON Patch against `{deck, sources, bindings, report, origin?}`, with explicit revision and hash preconditions in Rust. Operations on a batch are atomic. Undo/Redo receipts also pass through the normal transaction checks; they are not authorization capabilities. History is limited to 30 receipts and an approximate serialized-size cap, and is not persisted in checkpoints.

`exportProject()` rejects stale source bindings and measured text errors in newly authored documents. It returns PPTX base64 and a hash-bound checkpoint, but does not itself write to disk. The native application and MCP use create-new publication. Browser downloads can require permission for multiple files.

`importPresentation(id, base64)` returns a session retaining the original archive, approximate-preview warnings and per-object writable fields. Edits outside the supported fields fail before state commits. Imported documents cannot detach or change their origin within a transaction; create a new document for authoring.

Sources and checkpoints can contain private input data. Store them with the same care as the source presentation. Unkeyed SHA-256 hashes prove neither identity nor factual truth. Exact project reopening is guaranteed for the same exporter checkpoint; exporter/schema migrations are not yet supported.