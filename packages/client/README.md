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

`client.createAsset()` is stateless; `session.addAsset()` inserts under revision, cancellation and Undo guards. Accepted inert SVG is retained as a native SVG picture with an inspected transparent PNG fallback, not converted into editable shape paths. Arbitrary SVG, active content and external references are rejected. Native slide copies retain original XML and independently copy chart/workbook resources. Unsupported custom shows/sections or unsafe native copies fail without changing the session. Default `large` permits 256 slides / 32 MiB complete document; explicit `standard` retains 128 / 8 MiB and `legacy` 32 / 2 MiB. Constructor and create/open options accept `capacityProfile`; session operations preserve it. `setCapacityProfile()` verifies before changing the selection. `recoveryEnvelope` and `client.recoverSession()` preserve verified bounded Undo/Redo; legacy document-only recovery starts empty history. See [Phase 5 contracts](../../docs/testing/phase5-recovery-capacity.md).

## Master Presets

```js
const presets = await client.designPresets();
const session = await client.createPresentation('preset-example', 'Design review');
await session.applyDesignPreset('minimal', { expectedRevision: session.revision });
await session.assignLayout('slide-1', 'preset-two-columns');
const exported = await session.exportPresentation();
```

The seven presets define native masters/layouts, palette and font roles, side margins, gutters and content regions. Application retains existing slide content and original masters; custom or edited template conflicts fail before commit. The normal session revision, cancellation and Undo guards apply. New blank slides inherit the selected preset master. New parts on `preset-visual-content` are fitted to the layout's visual region without moving existing content. Reopened documents can add a supported preset structure within native preservation and capacity limits; an existing dedicated preset structure must pass its compatibility checks before replacement. See [preset limits and definitions](../../docs/testing/master-presets.md).

## Guided Authoring

```js
const profiles = await client.bestPracticeProfiles();
const guide = await client.bestPracticeGuide('consulting-decision');
const review = await client.validateGuidedPresentation(input);
if (!review.ready) throw new Error(review.issues.join('; '));
const created = await client.createGuidedPresentation('new-guided-document', input);
const exported = await created.session.exportPresentation();
```

`input` follows the `GuidedInput` schema returned with the guide; the complete [contract and four English profiles](../../docs/authoring/README.md) describe claims, logical ledger, body evidence paths, numeric source declarations and decision issues. [The executable MCP demo](../../tools/guided-demo.mjs) contains complete synthetic inputs for all four profiles.

Creation uses the shared core and returns a new session only on success. Early and late cancellation are rejected. Existing documents are never replaced, and file output is separate. `validation.ready` means compilable input, not verified source truth or a semantically justified conclusion. Review requirements and `model_inference:false` are explicit. The guide's 48 consulting patterns include manual/composed designs; only `native-part` and the consulting-specific `C02`/`C03` templates are automatic.

The complete ledger and source/assumption declarations are stored in notes and need privacy review before redistribution. They are distinct from the live source bindings API. Subsequent normal edits can invalidate reasoning without rerunning guided validation. Part geometry and data remain editable through the usual native metadata and Undo safeguards.

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

For an icon on an existing node, prepare the bytes using the graph-specific helper and replace the complete node specification:

```js
const icon = await client.createGraphIcon({
  base64: svgBase64, mime_type: 'image/svg+xml', alt: 'Service icon',
});
const graph = session.document.parts.find((part) => part.element_id === 'architecture').spec.data.graph;
const node = graph.nodes.find((entry) => entry.id === 'api');
await session.applyGraph(slideId, {
  id: 'architecture', operations: [{ op: 'put_node', node: { ...node, icon } }],
}, { expectedRevision: session.revision });
await session.undo();
```

The helper accepts supplied SVG/PNG/JPEG bytes and returns PNG/JPEG `GraphIcon` data fitted to 256px; it neither fetches a URL nor mutates the session. SVG is not retained as editable paths. Existing small raster bytes are preserved. Set `icon: null` on a `put_node` replacement to remove the icon. Native picture fingerprints participate in stale-metadata checks. `node tools/graphs-demo.mjs --icons` exercises all six node shapes with 18 icons across five slides, native icon replacement and byte-identical Undo.

## Design Helpers

Transactional design and insertion helpers apply to authored documents and the supported native subset of reopened presentations:

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

`client.createObject()` returns a validated element without mutating a session. Session helpers hold the same busy guard across the core transformation and commit, retain undo receipts, and check cancellation before committing. Reopened designs support ID-based master/layout additions, removals and reassignment, with immutable origins and unknown-XML/reference guards. Ordinary native shapes and other objects are editable only within their represented subset; unsupported replacements fail rather than flatten content. See the [authoring model](../../docs/api.md#authoring-model) for rich text, visual styles, paths, master themes and fields.

`client.authoringCapabilities()` and `client.designCapabilities()` expose operation limits and supported subsets. The object catalog has 24 chart kinds: 18 classic and six chartEx; see the [chartEx guide](../../docs/authoring/chart-ex.md) for the histogram Office/schema compatibility exception. Full method signatures and payloads are in [index.d.mts](index.d.mts) and [types.ts](types.ts), including search/replacement, selection, image/table/vector editing, bounded Boolean operations, review and comments. See [capability discovery](../../docs/api.md#capability-discovery), [local proofing](../../docs/authoring/proofing-format-painter.md), [document fonts](../../docs/authoring/fonts.md) and [master fields and themes](../../docs/authoring/master-fields-themes.md) for scoped contracts rather than assuming general Office parity.

`exportProject()` rejects stale source bindings and measured text errors in newly authored documents. It returns PPTX base64 and a hash-bound checkpoint, but does not itself write to disk. The native application and MCP use create-new publication. Browser downloads can require permission for multiple files.

`importPresentation(id, base64)` returns a session retaining the original archive, approximate-preview warnings and per-object writable fields. Edits outside the supported fields fail before state commits. Imported documents cannot detach or change their origin within a transaction; create a new document for authoring.

Sources and checkpoints can contain private input data. Store them with the same care as the source presentation. Unkeyed SHA-256 hashes prove neither identity nor factual truth. Exact project reopening is guaranteed for the same exporter checkpoint; exporter/schema migrations are not yet supported.