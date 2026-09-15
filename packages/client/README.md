# AISlide Client

This private, transport-independent client is shared by Studio and MCP. Rust owns schemas, layouts, transactions, validation, and PPTX behavior; the client only manages transport and bounded session history. It is not published to npm.

The normal file workflow is `client.openPresentation(id, pptxBase64)` and `session.exportPresentation()`. It uses one standard PPTX and retains optional sources/bindings internally. Current native XML is always authoritative. `openProject()` / `exportProject()` below remain legacy exact-checkpoint APIs and are not required for normal editing.

```js
const opened = await client.openPresentation('document-id', pptxBase64);
const session = opened.session;
const exported = await session.exportPresentation();
// exported.base64 is one .pptx; there is no external checkpoint.
```

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

`session.document` returns a clone. `transact()` uses RFC 6902 JSON Patch against `{deck, sources, bindings, parts?, report, origin?}`, with explicit revision and hash preconditions in Rust. Operations on a batch are atomic. Undo/Redo receipts also pass through the normal transaction checks; they are not authorization capabilities. History is limited to 30 receipts and an approximate serialized-size cap, and is not persisted in checkpoints.

## Workspace Commands

```js
const session = await client.createPresentation('workspace', 'Design review');
await session.editSlides([
  { op: 'insert', id: 'icons', after: 'slide-1', title: 'Service icons' },
], { expectedRevision: session.revision });
await session.addAsset('icons', {
  id: 'provided-icon', base64: svgBase64, mime_type: 'image/svg+xml',
  alt: 'Provided service icon', size: 96,
}, { expectedRevision: session.revision });
await session.editElements('icons', [
  { op: 'duplicate', id: 'provided-icon', new_id: 'icon-copy' },
], { expectedRevision: session.revision });
await session.undo();
const exported = await session.exportPresentation();
const reopened = await client.openPresentation('workspace-reopened', exported.base64);
await reopened.session.editSlides([
  { op: 'duplicate', slide_id: 'icons', id: 'icons-copy' },
  { op: 'move', slide_id: 'icons-copy', index: 0 },
]);
```

`client.createAsset()` is stateless; `session.addAsset()` inserts under revision, cancellation and Undo guards. SVG becomes an inspected transparent PNG, not an embedded SVG/vector object. Native slide copies retain original XML and independently copy chart/workbook resources. Unsupported custom shows/sections or unsafe native copies fail without changing the session. Use the [workspace API](../../docs/api.md#workspace-commands) for limits and source-preservation details.

## Metadata Parts

The catalog contains 108 original layouts across 36 categories, each with a typed synthetic example. Given an existing session and a slide with room for a part:

```js
const catalog = await client.partCatalog();
const preset = catalog.presets.find((entry) => entry.id === 'vertical-bar-graph/balanced');
const spec = structuredClone(preset.example);
spec.title = 'Quarterly volume';
spec.data.categories = ['Q1', 'Q2', 'Q3'];
spec.data.series = [{ name: 'Volume', values: [12, 24, 18] }];
const slideId = session.document.deck.slides[0].id;
await session.addPart(slideId, { id: 'quarterly', spec }, { expectedRevision: session.revision });
spec.data.series[0].values[0] = 16;
await session.updatePart(slideId, { id: 'quarterly', spec }, { expectedRevision: session.revision });
await session.undo();
const exported = await session.exportPresentation();
const reopened = await client.openPresentation('reopened', exported.base64);
console.log(reopened.session.document.parts[0].stale);
```

`client.createPart({id,spec,theme?})` returns a native group for preview without attaching semantic metadata. Use session `addPart`/`updatePart` for persistent metadata, revision guards and history. Root placement is retained on update. Native XML and embedded workbook changes make mismatching metadata stale; update then fails instead of replacing manual content. Deleting a part removes its metadata in the same undoable transaction. Data, source content and metadata remain document-private even though only one PPTX file is required. See the [parts guide](../../docs/testing/parts-library.md) for supported inputs and limits.

## Architecture Graphs

Given a session and an existing slide with room for a graph:

```js
const catalog = await client.graphCatalog();
const spec = structuredClone(catalog.examples[0].spec);
spec.title = 'Service architecture';
const slideId = session.document.deck.slides[0].id;
await session.addGraph(slideId, { id: 'architecture', spec }, { expectedRevision: session.revision });
await session.applyGraph(slideId, {
  id: 'architecture',
  operations: [{ op: 'move', ids: [spec.nodes[0].id], dx: 24, dy: 16 }],
}, { expectedRevision: session.revision });
const exported = await session.exportPresentation();
const reopened = await client.openPresentation('architecture-reopened', exported.base64);
console.log(reopened.session.document.parts.find((part) => part.element_id === 'architecture').stale);
await session.undo();
```

`client.createGraph({id,spec,theme?})` previews native elements; `client.transformGraph(spec,operations)` validates a candidate without modifying a session. Use `updateGraph` for a complete specification or `applyGraph` for node/edge/boundary operations. Both preserve root placement and reject stale metadata. Limits are in the [API contract](../../docs/api.md#architecture-graphs). A full official-MCP example is [tools/graphs-demo.mjs](../../tools/graphs-demo.mjs).

## Design Helpers

Authored documents also expose transactional design and insertion helpers:

```js
const design = await client.designDefaults();
const catalog = await client.objectCatalog();
await session.updateDesign(design, { expectedRevision: session.revision });
const slideId = session.document.deck.slides[0].id;
await session.assignLayout(slideId, 'title-content', { expectedRevision: session.revision });
await session.addObject(slideId, { id: 'callout', kind: 'shape', preset: 'wedgeRoundRectCallout' }, { expectedRevision: session.revision });
const theme = structuredClone(design.theme);
theme.colors.accent1 = 'B53055';
await session.applyTheme(theme, { expectedRevision: session.revision });
console.log(catalog.charts);
```

`client.createObject()` returns a validated element without mutating a session. Session helpers hold the same busy guard across the core transformation and commit, retain undo receipts, and check cancellation before committing. Design operations are for authored documents; imported-origin writable-field restrictions still apply.

`exportProject()` rejects stale source bindings and measured text errors in newly authored documents. It returns PPTX base64 and a hash-bound checkpoint, but does not itself write to disk. The native application and MCP use create-new publication. Browser downloads can require permission for multiple files.

`importPresentation(id, base64)` returns a session retaining the original archive, approximate-preview warnings and per-object writable fields. Edits outside the supported fields fail before state commits. Imported documents cannot detach or change their origin within a transaction; create a new document for authoring.

Sources and checkpoints can contain private input data. Store them with the same care as the source presentation. Unkeyed SHA-256 hashes prove neither identity nor factual truth. Exact project reopening is guaranteed for the same exporter checkpoint; exporter/schema migrations are not yet supported.