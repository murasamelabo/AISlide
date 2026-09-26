# Shared API Contracts

Every operation is strict JSON with a discriminator `op`. Unknown JSON request properties fail validation; this is distinct from preserving unknown native XML or unevaluated dynamic-field kinds. The CLI reads one request from stdin and prints one response; errors use stderr and a nonzero exit code. Tauri and the development adapter share the same dispatch in `aislide-core`.

## Capacity Profiles

Requests default to fixed `large`: 256 slides, 8192 total elements, 32 MiB complete UTF-8 document including immutable origin, 96 MiB JSON request/response, and 16 MiB raw archive. `capacity_profile: "standard"` retains 128 slides / 8 MiB document / 16 MiB wire / 8 MiB archive; `legacy` retains 32 slides / 2048 elements / 2 MiB document / 4 MiB wire. All limits apply together. Profiles are request-local, including intermediate transactions and responses, not a Deck field or global mutable context. SDK creation/open options accept `capacityProfile`; sessions retain it, and `setCapacityProfile()` validates current document and both history chains before committing a change. Unknown/null/custom/unlimited profiles reject. Capacity profiles do not themselves expand model/guided input grammar; guided authoring has a separate explicit `authoring.slide_limit` opt-in described below. Independent image/source/static-output limits remain unchanged. See [Phase 5 budgets and verification](testing/phase5-recovery-capacity.md).

MCP results above 64 KiB omit the duplicate `structuredContent`; parse `content[0].text` when it is absent. The fully serialized outer tool response remains byte-bounded.

## Single-file PPTX

`open_presentation` takes `{id, base64}` for one nonencrypted standard Open XML PPTX and returns `{document, warnings, objects, format:"open_xml_pptx"}`. It reads current native slide, master, layout, theme, table, chart and picture XML. It never uses an external scene checkpoint as the source of content. The returned immutable origin identifies the original package for copy-through edits, not a second file the caller must retain.

`export_presentation` takes `{document}` and returns `{base64, filename, layout}` without a checkpoint. Supported edits patch existing XML and preserve other parts. Sources, source bindings, part specifications and stable-ID hints are stored as bounded structured Custom XML; no cached deck is embedded. Existing source staleness checks still apply. Save uses create-new semantics, not silent overwrite.

SDK: `client.openPresentation(id, base64)` returns a session and warnings; `session.exportPresentation()` returns the single-file export. MCP: `open_pptx` opens the caller-supplied bytes; `export_pptx` writes only one file to the operator-approved output directory. `open_project`, `export_project`, `import_pptx` and the legacy SDK equivalents remain compatibility APIs, not the Studio file workflow.

OLE containers, which include encrypted/protected Office files and legacy `.ppt`, receive an explicit format error before ZIP parsing. AISlide does not decrypt, remove labels or alter protection settings. The native reader currently supports Transitional PresentationML, not the separate ISO Strict namespace profile. Reopened designs support ID-based master/layout additions, removals and reassignment within the modeled subset and capacity limits. Unknown XML is preserved, and edits that would invalidate retained references or replace unsupported content are rejected. Supported slide structure commands are described below; complex text or chart changes outside the representable subset are rejected rather than flattened.

## Static Export and Recovery

`export_static` accepts `{document, options}` and verifies the complete document before calling the shared `export_static::export_static` renderer. Stale source bindings reject. No file, original package, document revision or history is changed. Unknown request/options fields reject.

Options are `format: "png" | "jpeg" | "pdf"` (default PNG), `page_indices?: number[] | null` (zero-based output order; omitted/null means all), `scale` (0.01-16, default 1), `transparent` (default false), `jpeg_quality` (1-100, default 90), `jpeg_matte` (three RGB bytes, default white), `max_output_bytes` (1-33554432; lower-only), and `deny_warnings` (default false). Selections contain 1-32 unique existing pages, including pages beyond 32 in a larger deck. Output edges are capped at 8192 px; the core also bounds working/cumulative raster allocation. Static bundles retain their separate legacy-sized envelope: usable binary output is less than 3 MiB even though the default general protocol is 96 MiB and core output may request 32 MiB. Oversized bundles fail without returning partial files; reduce scale or select fewer pages.

The result is `{files, warnings, office_parity_verified:false, pdf_rasterized:false, pdf_text_outlined, pdf_editable_text:false, pdf_tagged, pdf_searchable_text, pdf_selectable_text, pdf_semantic_overlay, pdf_ua_certified:false}`. The five format-dependent flags are true for PDF and false for images. Each file has `{filename, base64, byte_length, mime_type, page_indices, width, height}`. PNG/JPEG produce one image per page, named `report-page-NNN.png` or `.jpg` using the original one-based page number. PDF produces `report.pdf` with all selected pages in order. PDF paper size is the scene size at 96 dpi, independent of preview scale. Visible text remains outlined, not editable; an invisible positioned Unicode layer provides search/selection. This is not PDF/A, PDF/UA or WCAG certification. Pictures remain embedded rasters and Office visual parity is unverified. Render warnings include code, page index, element ID and message.

PDF semantics use the renderer's shaped clusters, baselines, widths and group/rotation transforms, including rich text and table cells. `Identity-H` fonts and `ToUnicode` maps encode Unicode, including Japanese; no installed or document font programs are embedded or redistributed. Existing document-font fsType validation and license-consent gating remain in force. Semantic working data has an additional cumulative 8 MiB bound. Missing glyphs/font fallback and clipped overflow retain renderer warnings and `deny_warnings` behavior. Fully clipped clusters are omitted; complex-script cluster selection and partially clipped glyphs require viewer review. Appearance is unchanged by the invisible layer, but it is not an editable-font PDF.

The PDF contains `StructTreeRoot`, `MarkInfo`, per-page `StructParents`, a `ParentTree` and marked content with MCIDs. Each selected page has a `Sect`, text has paragraph tags, and tables have correctly parented row/header/data-cell tags with merge spans. Explicit `slide.review.table_headers` controls semantics: unknown/none (including missing metadata) produce TD only; first_row produces TH with Scope Column, first_column with Scope Row, and both uses Scope Both at the corner. Visual first-row styling never implies semantic headers. Complex merged associations require manual review. `slide.review.reading_order` controls top-level logical order; absent metadata uses scene order. Groups use child order. Decorative/hidden content is excluded from semantic reading. Figure descriptions use review metadata or picture `alt`, with nonpainting tagged geometry proxies; original visual artwork is marked Artifact. Alt-text adequacy, document language, inherited-layer reading order and assistive-technology behavior are not certified.

The renderer supports represented basic forms of all 24 chart kinds, plus six SVD trendline types, forecasts, error bars, secondary axes, log/reverse scales and bounded custom ticks. Numeric formatting is limited and reports General fallback; unsupported combinations reject rather than disappear. Eight default WordArt presets use shaped glyph outlines. See [shared chart/WordArt display contracts](authoring/chart-wordart-presentation.md). PNG/JPEG render modeled effect filters. PDF rasterizes effect-bearing top-level objects at 2x with `PDF_EFFECT_RASTERIZED`; other visual objects and outlines remain vector. Visible text inside such an affected object/group is rasterized with it; the semantic layer remains separate. `pdf_rasterized:false` is not a promise of an entirely raster-free PDF. No Office visual parity is implied.

SDK: `session.exportStatic(options?, requestOptions?)` is read-only, serialized with other session operations and rejects early/late cancellation. `client.verifyRecovery(document, requestOptions?)` calls core `verify_recovery` and returns a deeply frozen copy. `client.recoverPresentation(document, requestOptions?)` additionally creates a separate `DocumentSession` with empty Undo/Redo history. The core verifier calls `document::verify`, including shape, canonical hash, sources and native-origin checks; storage hashes are never a substitute. Hashes are unkeyed consistency checks, not authentication. Protected/unsupported origins are not decrypted, detached or bypassed.

MCP: `export_static({deck_id, filename, options})` requires explicit format and an exact matching `.pdf`, `.png` or `.jpg` extension. It publishes only beneath the operator-approved output directory using the existing exclusive-file publisher. Multiple images append `-page-NNN` to the requested basename; one file keeps the requested name. Every final name is checked before publication. Racing failures can leave a partial bundle: confirmed published paths are reported and public paths are never deleted. Legacy `verify_recovery({document_json})` and `recover_presentation({document_json})` retain document-only validation and empty-history restoration. `get_session_recovery` / `recover_session({recovery_json})` exchange a versioned document/profile/receipt envelope, up to 48 MiB UTF-8 and subject to the selected profile and separate history bounds. None accesses local recovery storage.

Studio's Export PDF and images panel supports current/all pages, individual downloads and sequential Download all images. It uses the existing `downloadBytes`; native saves retain `createNew:true`. Prepare never opens a window or prints. Prepare print exports at most 32 opaque PNG pages at 96 dpi into an owned React portal under the same document body, using only local PNG blob images and explicit paper size. Print media CSS hides the application only while printing. There is no iframe; CSP `frame-src 'none'` is unchanged. Only a subsequent Open print dialog click calls the owning window's `window.print()`. The system dialog controls confirmation/cancellation; no printer selection, silent job, arbitrary file/shell IPC or Office automation is exposed to SDK/MCP. Direct print is raster, not the searchable PDF; use a PDF viewer for vector output. Changing options/unmounting revokes preview URLs. On 2026-09-19, native UI Automation verified the owned PID/command marker, localized Print/Cancel controls and beforeprint/afterprint, then selected Cancel only. No job was submitted; physical printing remains unverified. Evidence: `.artifacts/g35-directprint-20260919/native-print-proof.json`.

Studio recovery policy v2 starts OFF, reads its remembered consent, and persists only after explicit consent to plaintext original/source/history storage. Committed states are debounced into at most five entries, 48 MiB each / 100 MiB total; each Undo/Redo stack is strictly 30 receipts / 4 MiB UTF-8. Oversized receipts create a visible history boundary. Windows uses a separate bounded worker and fixed AppData directory; pinned directory/ancestor handles prevent rename races, with open-file validation before creation. Non-Windows native storage fails closed; web uses separate IndexedDB v2. Core validates both chains on disposable states before restoration, then Studio applies its existing unsaved-change guard. Generations/CAS and consent epochs protect concurrent save/delete; cancel/dispose suppress late writes/callbacks. Seven-day expiry runs only on opening an enabled store, not while closed. Disabling retains copies. Legacy v1 copies require explicit legacy restore/discard and are never automatically migrated/deleted. Recovery never overwrites the original PPTX. See [the full storage contract](testing/phase5-recovery-capacity.md#3-opt-in-storage-policy-v2).

Verification: `crates/aislide-core/tests/static_api.rs`, targeted static/recovery tests in `tools/client.test.mjs` and `tools/mcp.test.mjs`, and `tests/e2e/output-recovery-workflow.spec.ts` exercise the actual product/CLI/MCP boundaries. The existing recovery module suite additionally covers storage budgets, consent races and scheduler disposal. No Office or native printing parity is claimed.

The historical static gates passed 31 `export_static` and 4 `static_api` tests, including all-24 PNG/PDF output, histogram final-interval labels and pie/doughnut legends. On 2026-09-19, 11 focused PDF tests passed after the table-parent/header-policy repair; focused web/native print checks passed separately. Two native WebView profiles exercised recovery persist/restart/restore and both histories before the latest storage-security repair; seven storage unit tests passed after that repair. These are bounded checks, not current full-product totals. Final core/Node/UI/native/Office/install gates remain pending in the [completion plan](planning/editing-completion.md). Selecting at most 32 pages remains required; omitting selection can exceed that limit. Recovery is not source AutoSave. The [histogram Office/schema exception](authoring/chart-ex.md) is unchanged.

## Document Operations

### Typed Authoring Batches

`apply_operations({document,expected_revision,expected_hash,operations})` accepts
1-128 strict typed operations across existing slides, including mixed managed
parts, graphs and ordinary element edits. Managed variants use the same
`parts::state::change_in_deck` helper as individual insertion/update calls,
updating native-editable objects and their `PartInstance` together in the
batch's working deck/metadata. They do not create a new document or perform
full document verification for every item. The input document is verified,
each intermediate scene is preflighted, and the final commit still passes
document transaction/native preservation guards.

A changed batch produces one revision and one inverse Undo receipt; failure,
including an invalid final specification, leaves the original document and
history unchanged. A no-op adds no history. The document may contain at most
128 metadata parts TOTAL, including managed graphs, in every capacity profile;
this is not an allowance per batch or per slide. All other scene, document,
wire, archive and history caps still apply together. No automatic fallback to
unmanaged groups occurs on failure or timeout. The call does not render a live
preview or require a preview candidate. Batching avoids per-item requests,
not validation or transport costs; no measured speedup is claimed for this
workflow. Raw JSON Patch `transaction` remains a separate API.

Every operation has `op` and `slide_id`, plus the following fields:

| `op` | Payload | Behavior |
| --- | --- | --- |
| `add_elements` | `elements` | Append 1-128 complete typed `Element` roots with final content, geometry and supported formatting; normal scene budgets still apply |
| `add_part` | `id`, `spec: PartSpec` | Insert editable objects and managed metadata; use `spec.layout` for final placement |
| `update_part` | `id`, `spec: PartSpec` | Replace a current managed part's specification and rendering atomically; stale metadata rejects |
| `add_graph` | `id`, `spec: GraphSpec`, optional `layout: PartLayout \| null` | Insert a managed `diagram/custom` part with the requested placement |
| `update_graph` | `id`, `spec: GraphSpec` | Regenerate a current managed graph, preserving its existing `PartLayout`; a `layout` field is not accepted |
| `set_frame` | `id`, `frame` | Set geometry without changing font or stroke sizes |
| `set_text_style` | `ids`, `style` | Overlay a nonempty partial `RunStyle` on 1-128 distinct text/shape targets, their defaults and existing runs; retain unspecified run and paragraph attributes |
| `set_slide_background` | `color` | Set RGB/theme color and disable background inheritance |
| `set_connector` | `id`, `connector`, optional `frame` | Replace all connector settings, optionally changing its frame |
| `set_picture_crop` | `id`, `crop` | Replace a picture's crop |
| `set_hyperlink` | `id`, required `link` | Set a validated text/shape URL; explicit `null` clears it, an absent field is an error |
| `set_shape_adjustment` | `id`, `adjustment` | Set a supported typed shape adjustment; not arbitrary DrawingML formulas |
| `add_picture` | `id`, `base64`, `mime_type`, `alt`, optional `frame`, `crop` | Decode PNG/JPEG and insert at the supplied frame/crop in the same transaction; omission retains the fitted picture defaults |

`Frame` is `{x,y,width,height}`: finite nonnegative coordinates, positive
dimensions, each bounded by 4096 scene pixels, with final page/group bounds
also enforced. Nested target coordinates are parent-local. Resizing a group
normalizes child geometry and its view dimensions recursively, but leaves
fonts and strokes unchanged. Absolute table column/row tracks scale
proportionally; relative tracks retain their proportions. Text-frame edits
detach layout inheritance. Locked/hidden targets or traversed parent groups
reject, as do locked/hidden descendants encountered during group resizing.
Geometry edits can make managed part metadata stale; use `updatePart` for
semantic regeneration of a current part, not to overwrite stale manual edits.

`ConnectorSettings` requires `{color,stroke_width,arrow}` and optionally
`flip_v`, `start`, `end`, `routing`. It is a **replacement**, not a partial
overlay: omitted endpoints/routing clear their previous values and omitted
`flip_v` becomes false. Existing visual properties remain unchanged. The
existing connection/routing representability checks still apply.
`set_text_style` is the partial-style API; existing `apply_format` /
`session.applyFormat` remains the compatible-object format painter.

Partial style updates do not bypass native text guards. Imported rich text can
reject frame-default style changes even when the same partial update works on
authored text; use the supported rich-range/paragraph APIs where appropriate.
Partial inherited native transforms and other unsafe updates still reject.
Shape adjustments support only `{name:"adj",value}` for `roundRect` (0-50000),
`chevron` and `triangle` (0-100000). Multiple named guides and arc adjustments
are not supported.

SDK: `session.applyOperations(operations,options?)`,
`addElements(slideId,elements,options?)`,
`setFrames(slideId,[{id,frame}],options?)`,
`setTextStyle(slideId,{ids,style},options?)`,
`setSlideBackground(slideId,color,options?)`,
`setConnector(slideId,{id,connector,frame?},options?)`,
`setPictureCrop(slideId,{id,crop},options?)`,
`setHyperlink(slideId,{id,link},options?)`,
`setShapeAdjustment(slideId,{id,adjustment},options?)`, and
`addPicture(slideId,{id,base64,mime_type,alt,frame?,crop?},options?)`.
`AuthoringOptions` accepts `expectedRevision`, `expectedHash` and `signal`;
omitted guards use the session's current revision/hash. Existing session busy,
late-cancellation and bounded-history rules apply. `setFrames` sends one batch
of `set_frame` operations, not separate requests.

MCP `apply_operations` substitutes `deck_id` for `document` and requires
`expected_revision`, `expected_hash` and `operations`. Non-managed convenience
tools use the operation names above, plus `set_frames` with `frames:[{id,frame}]`;
their `expected_hash` is optional and their revision is required, except that
`add_picture` retains optional revision for legacy callers. Supply both guards
for stale-edit protection. Complete elements are preferred over inserting
factory defaults and patching every field. Batch first, then preview/preflight
explicitly selected pages.

For reusable parts/graphs, prefer managed `add_part` / `add_graph` in this
batch over separate per-item calls. Existing individual MCP `add_part`,
`update_part`, `add_graph` and `update_graph` remain compatible. SDK callers
use the existing `session.applyOperations`; no new batch method is needed.
The existing SDK `AuthoringOperation` union includes these members (shown as
a documentation-local subset, not a new SDK export):

```ts
type ManagedAuthoringOperation =
	| { op: 'add_part'; slide_id: string; id: string; spec: PartSpec }
	| { op: 'update_part'; slide_id: string; id: string; spec: PartSpec }
	| { op: 'add_graph'; slide_id: string; id: string; spec: GraphSpec; layout?: PartLayout | null }
	| { op: 'update_graph'; slide_id: string; id: string; spec: GraphSpec };
```

Supply `PartSpec.layout` or the batch graph's top-level `layout` before
insertion so the part fitter handles geometry and text, instead of blindly
scaling a completed group's frame/fonts afterward. Generic child edits may
make metadata stale and block later semantic updates, even later in the same
batch; that rejection rolls back the entire batch.

`create_part` / `create_graph` return structured ordinary groups, not managed
instances. Passing those outputs to `add_elements` / `session.addElements`
remains an EXPLICIT UNMANAGED choice only, not the recommended reuse path or
an automatic timeout workaround. See the [managed MCP acceptance sequence](authoring/README.md#managed-batch-mcp-acceptance).

### Managed Timeouts And Progress

The Node core bridge retains a 20-second budget for small ordinary requests.
Let U be the maximum of rounded-up slide count / 8, nested element count / 256,
and encoded request bytes / 1 MiB. The scan is bounded to 256 slides and 8192
elements. For U > 1, the document budget is `min(180, 20 + U * 10)` seconds.
The base is the maximum of that budget, 120 seconds for requests **greater
than 4 MiB** or recovery verification/preparation, and the operation minimum:
60 seconds for `open_presentation` / `import_document`, or
`60 + 5 * (selectedPages - 1)` for `preview_presentation` (1-8 pages).
Composite `preview_slide_revision` and `prepare_delivery` receive at least
120 seconds because they perform additional rendering/export work.
For example, a 42-slide/1344-element document gets 80 seconds for ordinary
work and 95 seconds for an eight-page preview. These limits cover bridge
execution; timeout messages separately report measured elapsed time,
including serialization and process-exit delay. Read-only timeouts report
interrupted reads rather than an uncommitted edit.
For `apply_operations`, N counts only top-level
`add_part`, `update_part`, `add_graph` and `update_graph` entries, not graph
nodes or child elements. Valid batches contain at most 128 operations.
Individual core `insert_part`, `update_part`, `insert_graph`, `update_graph`
and `apply_graph` count as N=1.

For N>0, the timeout in seconds is `min(300, max(base, 60 + N * 5))`.
For N=0 it remains the base. With the ordinary 20-second base, one managed
operation gets 65 seconds, 21 get 165 seconds, and 128 get 300 seconds.
One managed operation in a request greater than 4 MiB gets 120 seconds.
Existing `generate`, `text_assist` and `segment_image` budgets remain
310 seconds. These are finite execution limits, not latency guarantees,
unlimited execution or a retry loop. Full-document transport remains;
no persistent stateful core or differential protocol is introduced.

MCP mutations, including direct element/frame/note edits and imports, plus
preview/measurement/preflight/accessibility/validation operations optionally
emit standard `notifications/progress`
when `tools/call` params include `_meta.progressToken` (a string or safe
integer, including zero); the handler receives it as `extra._meta`.
`_meta` belongs beside `name` and `arguments`, not inside strict tool
arguments. Cheap metadata-only reads do not start a progress timer.
An initial notification and subsequent notifications on a five-second timer
report elapsed time only. Slow in-flight sends skip ticks. No `total` or
estimated completion percentage is supplied, and messages contain no slide
text, specifications, source data or other private content. Notifications
stop on completion, failure or abort; optional notification failure does not
fail the mutation. No token means no progress notifications.

SDK `AbortSignal`, session serialization and late-cancellation guards remain.
The MCP client's own overall timeout may need to be at least the server
budget, with transport margin and progress-based timeout reset where
supported. Some clients ignore progress; a heartbeat does not extend the
server's finite budget or guarantee client completion. Inspect current
revision/hash after a client-side timeout before deciding on a new call;
do not blindly retry or downgrade to unmanaged output.

### Visual Authoring Loop

The shared core additionally exposes the following bounded read/preview/apply operations. See [guided authoring](authoring/README.md#visual-review-and-revisions) for limits and the complete MCP workflow.

| Core operation | Inputs besides op | Result |
| --- | --- | --- |
| `preview_presentation` | `document`, optional `options:{page_indices,max_dimension,layout,max_output_bytes,format,overflow}` | `revision`, `hash`, requested/actual max dimension, quality-reduction flag, ordered page IDs/image coordinates, PNG/JPEG images with base64/size/SHA-256 and renderer warnings |
| `preflight_presentation` | `document`, optional `options:{page_indices,min_font_size}` | Bounded findings with IDs/scopes/bounds/evidence/suggestions; no mutation |
| `preview_slide_revision` | `document`, `expected_revision`, `expected_hash`, `slide_id`, `edits`, optional `max_dimension` | Base/candidate hashes, before/after previews, affected IDs and stale part/source impact; no mutation |
| `apply_slide_revision` | Same base and edits, plus `candidate_hash`, without `max_dimension` | Normal atomic transaction result with one inverse Undo receipt |

Previews use the existing shared static renderer with 2MiB encoded/4MiB wire budgets, 160-1600px edges and 1-8 selected pages. `format` is `png` (default) or `jpeg` (quality 90, lossy). `overflow` is `shrink` (default) or `error`. Only encoded-byte overflow retries at the original size, then floor(75%) and floor(56.25%), minimum 160px, at most three distinct attempts. All pages are retained; other errors do not trigger retry. `requested_max_dimension`, `actual_max_dimension` and `quality_reduced` disclose dimension reduction, accompanied by `PREVIEW_DOWNSCALED`. JPEG selection itself is not a dimension reduction. Strict `overflow:"error"` forbids shrinking. Exhaustion returns suggestions, not partial pages or a promised resolution that will fit. Before/after images reserve 1MiB each. No network fetch, automatic font installation or file write is performed. Stale bindings are reported for inspection; export still rejects them. Diagnostics remain heuristics, not Office parity or full accessibility certification.

SDK methods: `session.previewPresentation(options?,requestOptions?)`, `preflightPresentation(options?,requestOptions?)`, `previewSlideRevision(slideId,edits,{expectedRevision?,expectedHash?,maxDimension?,signal?})`, and `applySlideRevision(slideId,edits,{expectedRevision,expectedHash,candidateHash,signal?})`. Session reads serialize with writes and reject early/late cancellation. Pure core candidates carry no session authority; callers supply and recheck the base and candidate hashes.

MCP substitutes `deck_id` for `document`. Preview tools return metadata in the first text block and standard MCP `image` blocks; `include_images:false` omits images. Existing JSON-only tool responses remain unchanged. `preview_slide_revision` retains only bounded edit instructions under an opaque process-local candidate ID (16 candidates, ten-minute expiry, 128KiB each). `apply_slide_revision` accepts that ID plus the exact base revision/hash rather than replacement edits. Stale/closed-deck candidates release capacity; applied candidates are single-use. No-op applies do not add history. `author_presentation` and `aislide://authoring/workflow` expose the workflow through the official MCP prompt/resource APIs.

Typed edits: `translate{ids,dx,dy}`, `align{ids,alignment,relative_to}`, `set_text_frame{id,x,y,width,height}`, `replace_text{id,text}`, `update_part{id,spec}`, and `update_graph{id,spec}`. One slide, 1-16 edits and up to 32 unique targets per selection; no arbitrary JSON Patch, file path, XML or source authority. Nested text coordinates remain parent-local. Locked/hidden targets and native loss reject. Generic child edits may make managed metadata stale; the preview reports this. Existing raw `transaction` remains a separate lower-level API.

### Delivery Preparation

`prepare_delivery({document,expected_revision,expected_hash,options?})` returns `{revision,hash,files,manifest}` without filesystem access or document changes. SDK: `session.prepareDelivery(options?,{expectedRevision?,expectedHash?,signal?})`; the existing session read lock and early/late cancellation checks apply. Each file has `kind`, server-independent `suffix`, `mime_type`, `page_indices`, `byte_length`, `sha256`, `base64` and optional image dimensions. Native PPTX export checks, including immutable origin and stale source-binding rejection, remain authoritative.

Options: `page_indices?`, `pdf:false`, `preview:"contact_sheet"` (`pages` / `none` also accepted), `notes:false`, `source_report:false`, `preflight:true`, `max_dimension:1280` (160-1600), `min_font_size:16` (8-48), and lower-only `max_output_bytes` (up to 32MiB). All options are strict. The complete PPTX is always included. Supplemental visual output/preflight is limited to eight selected pages; notes and source metadata span the whole deck. The report excludes raw source text, table rows, raw binding values and original bytes, but attributions/locators may still be sensitive. Separate plaintext notes and source-report output require explicit opt-in; the PPTX itself remains an ordinary unredacted export.

MCP `finalize_presentation({deck_id,expected_revision,expected_hash,name,options?,include_images?})` prepares the same bundle, binds safe filenames, verifies hashes and budgets, then exclusively publishes beneath the startup-approved output root. At most 13 files including the manifest; 32MiB decoded total and at most 4MiB MCP response (profile limits also apply). A thumbnail plus actual file paths, hashes, sizes, check scope and manifest path are returned. `include_images:false` suppresses only the response image. Every destination is checked, all temporary files staged, and the manifest published last. Existing files are never replaced. No directory, URL or arbitrary file path is accepted from the request.

The manifest format is `aislide.delivery`, version 1. It records actual producer/core versions and MCP transport, document revision/hash, file hashes, visual page scope, checks, findings and limitations. `complete` is publication success, not quality approval; preflight can report findings. No source authenticity/freshness, semantic truth, accessibility certification or Office parity is asserted. Files are not a crash-atomic group. A structured `BUNDLE_PUBLICATION_FAILED` result lists exact successful paths and pending filenames with a `not_published`, `partially_published` or `published_with_error` status. Published files are never cleaned up automatically; only owned temporary paths are removed. Cancellation after a link may leave outputs; inspect the manifest and hashes before retrying with a new name. Root checks do not establish hard immunity to hostile local path races. See [delivery workflow and example](authoring/README.md#delivery-bundles).

### Master Import

`inspect_master_source({kind,base64})` accepts explicitly selected non-macro
`pptx` or `potx` bytes. It returns `source_sha256`, dimensions, master and slide
entries (`id`, `name`, `importable`, rejection `reason`, and master
`layout_count`), warnings and `office_visual_parity:false`. Inspection is
read-only and creates no document handle or stored source.

`preview_master_import({document,expected_revision,expected_hash,input})`
validates an append-only design change through the ordinary transaction
guards, without accepting it into a session. Input contains `kind`, `base64`,
`source_sha256`, `mode:"masters"|"slides"`, 1-8 distinct `ids`, an ASCII
alphanumeric/underscore/hyphen `prefix` of 1-32 bytes and a nonblank `name`
of at most 60 characters. The response contains the base revision/hash,
candidate hash, complete candidate `design`, added `master_ids`, `layout_ids`,
unique `preview_slides`, warnings and `office_visual_parity:false`.

`import_masters` takes the same arguments plus `expected_candidate_hash`,
recomputes the candidate and returns the normal transaction result. The
immutable target origin, slide content, source records and bindings stay
unchanged. A design-less authored target gains a blank default master before
the selected additions. ID collisions, source/base/candidate mismatch, page
dimension mismatch, eight-master/32-layout totals and selected capacity
budgets reject before commit. One inverse receipt restores the prior design.

Existing-master mode copies supported masters with their owned layouts and
explicit source themes. Sample-slide mode creates one master/layout per
selected slide, with visible inherited artwork and remapped connector IDs.
Ordinary text stays fixed; existing text placeholders remain placeholders.
Unsupported XML, shape styles, theme effects or resource relationships fail
closed. Source fonts, notes, comments and citation metadata are not imported.
Protection is not stripped, relationships are not fetched, and no original
file is modified. This does not certify arbitrary Office-template fidelity.

SDK: `client.inspectMasterSource(input,options?)`,
`session.previewMasterImport(input,options?)` and
`session.importMasters(input,candidateHash,options?)`; options include
`signal`, `expectedRevision` and `expectedHash` for session operations.
Existing serialization, late-cancellation and Undo checks apply. MCP uses
the same names, replacing `document` with `deck_id`, and strict schemas;
results are JSON-only. See [Studio and MCP workflow](authoring/README.md#import-masters-from-pptx-or-potx).

The development bridge also accepts the operator-only `AISLIDE_CORE_BINARY`
environment variable at process startup, for testing a separately built CLI
while another connection uses the default executable. It must be an absolute
local path, not a relative/UNC path or a request field. The default remains
`target/debug/aislide[.exe]`; no host configuration or active connection is
changed automatically. Configure this explicitly in a spawned MCP client's
environment when needed, because the official SDK uses an environment allowlist.

| Operation | Request fields besides op | Result |
| --- | --- | --- |
| `new_document` | `id`, `deck`, optional `sources`, `bindings`, `report` | Canonical document with revision 0 and content hash |
| `transaction` | `document`, `transaction: {expected_revision, expected_hash, operations}` | Updated document, inverse receipt, changed JSON paths |
| `undo_transaction` | `document`, `expected_revision`, `receipt` | Updated document and a receipt usable for redo |
| `export_project` | `document` | `base64`, `filename`, `checkpoint`, `checkpoint_filename`, layout evidence |
| `open_project` | `base64`, `checkpoint` | Verified document, only if the exact PPTX matches |
| `import_document` | `id`, `base64` | Origin-bound document, warnings, per-object editable fields |

Content JSON Patch paths start with `/deck`, `/sources`, `/bindings`, `/parts`, `/report` or the immutable import origin. A batch has 1-128 operations; a stale revision/hash, failed test/path, oversized intermediate result or invalid final state aborts the batch. Revisions increase on real changes and Undo/Redo; no-op transactions do not create history. Deleting a part root also removes its metadata in that transaction, so Undo restores both. Core receipts are not signed capabilities and undergo the same validation as edits.

### Authored Slide Import

`import_slides({document,expected_revision,expected_hash,source,source_slide_ids,prefix,after?})`
imports 1-128 distinct existing source slide IDs in selection order into a
same-canvas target, after an existing target slide or at the end when `after`
is omitted/null. `source` is a verified document whose `origin` must be absent
(`None` in Rust). The prefix is 1-24 ASCII letters, digits, underscores or
hyphens; generated slide/master/layout IDs avoid target collisions. Target
capacity and the eight-master/32-layout totals still apply. The transaction
produces one Undo receipt and does not change the source document.

Matching master/theme/layout content is reused; unmatched required design is
copied with the source's effective theme. Selected slides retain notes,
supported review metadata, current authored part metadata and evidence
bindings with their referenced source records. Stale/native part fingerprints,
stale bindings, conflicting source hashes, native slide references and modern
comment threads reject. All source embedded fonts require explicit embedding
and editing license acknowledgement; matching family/style entries must have
matching bytes and consent. Font budgets remain enforced. Differing source
notes/handout masters cannot replace the target's auxiliary design. The
target's compiled `report` metadata is cleared because the combined deck is
no longer that report. Review retained notes/evidence before redistribution.

SDK: `session.importSlides(sourceDocument,{source_slide_ids,prefix,after?},options?)`
uses `AuthoringOptions`. MCP requires
`{deck_id,expected_revision,expected_hash,source_deck_id,source_slide_ids,prefix,after?}`.
`source_deck_id` must be an existing authored session handle; MCP accepts
neither a source file path nor arbitrary source document JSON. This is
separate from master import. Opening a native PPTX does not make it an
authored source: native cross-package slide copying remains unsupported.
Native targets retain their immutable origin and ordinary preservation
guards, which can reject otherwise valid authored additions.

## Workspace Commands

| Core operation | Fields besides op | Result |
| --- | --- | --- |
| `create_presentation` | `id`, `title` | Revision-zero document with one blank slide and the default design |
| `edit_slides` | `document`, `expected_revision`, `operations` | Atomic transaction and inverse receipt |
| `edit_elements` | Same, plus `slide_id` | Atomic top-level element operation |
| `create_asset` | `id`, `base64`, `mime_type`, `alt`, `size` | Validated picture element; accepted inert SVG is retained with a PNG fallback |

Slide operations, in a batch of 1-128:

- `{op:"insert",id,after?,title,layout_id?}` inserts after the specified slide, or appends when omitted. An omitted layout produces a blank slide; an explicit layout creates its placeholders and fills every title placeholder with `title`. An empty title clears the placeholder's sample text; other placeholders are unchanged.
- `{op:"duplicate",slide_id,id}` creates a copy immediately after its source. Part metadata and source bindings follow the new slide ID; stale part metadata blocks duplication.
- `{op:"remove",slide_id}` removes the slide and its part/binding records; the last remaining slide cannot be removed.
- `{op:"move",slide_id,index}` moves to a zero-based index in the final list.
- `{op:"rename",slide_id,title}` changes its displayed title, not the contents of text boxes.

Slides are limited to 1-256 under the default `large` capacity profile, 1-128 under explicit `standard`, or 1-32 under `legacy`. The request-local provenance limit is 296 identities across profiles; complete-document and XML budgets still apply. Native insertion creates only the added slide/resources and relationships; existing slide bytes remain untouched when their content is unchanged. Native duplication preserves opaque slide XML and independently copies mutable resources such as chart XML and embedded workbooks. It may share immutable images and design resources. Slides carrying modern comments cannot be duplicated. A bounded origin-relative `native_source_id` is used internally for in-session copies; callers should use `edit_slides`, not set it directly.

Native slide deletion removes its slide XML, dedicated notes, relationship parts and their content-type entries. Deletion is refused if a remaining object references those parts. Other shared or opaque package resources and source documents are retained; deletion is not secure redaction. Sections/custom slide shows must be handled in PowerPoint before structural changes. Copy traversal is capped at 128 related resources and 8 MiB. Original files and import origins remain unchanged.

Element operations are `{op:"duplicate",id,new_id}`, `{op:"remove",id}` or `{op:"order",id,index}`. They apply to the selected slide's top-level objects. Duplication remaps descendant IDs and connection references and copies bindings/part metadata. Removal also removes connectors that lose their target. Unsupported individual native copies fail rather than flatten custom formatting; duplicating the slide is the preservation-oriented alternative. All operations are undoable and require a current revision.

Assets support PNG/JPEG, inert SVG and a bounded EMF/WMF record subset converted locally to SVG. `size` is the displayed longest side, 8-640px; aspect ratio is retained. Raster files use the existing 1 MiB, 4096px and 64 MiB decode limits. SVG is limited to 256 KiB, 2,048 elements, XML depth 32, viewport 4096px, reference depth 48 and expanded reference cost 4,096. Safe text and embedded PNG/JPEG are supported within validation limits; cycles, missing/wrong-type fragment targets, external resources, scripts, `foreignObject`, styles, filters and unsupported effects reject. Outlined paths, basic shapes, gradients, clipping and masks remain supported. Raster output has a 1024px longest side, transparency and a 1 MiB limit, and is decoded again as PNG before use. Accepted SVG bytes are retained in the picture's optional `svg` field and embedded through the native SVG picture extension alongside the PNG fallback. This is not arbitrary SVG/metafile support, HTML execution or conversion into editable shape paths. These budgets are not an OS process sandbox.

SDK: `client.createPresentation(id,title?,options?)`, `session.editSlides(operations,options?)`, `session.editElements(slideId,operations,options?)`, `client.createAsset(input,options?)`, `session.addAsset(slideId,input,options?)`. Session options retain `expectedRevision` and cancellation. MCP exposes the same names with `add_asset` for insertion; mutations take `deck_id`, `expected_revision` and, for assets/elements, `slide_id`. `create_asset` is stateless. No additional filesystem/network authority is exposed.

## Guided Authoring

| Operation | Inputs | Result |
| --- | --- | --- |
| `best_practice_profiles` | None | Four purpose-specific English profiles and default color |
| `best_practice_guide` | `profile_id` | English common/profile guide, pattern capabilities, limits and Rust-derived `GuidedInput` schema |
| `validate_guided_presentation` | `input: GuidedInput` | Pure preflight `{ready,issues,review_required,semantic_truth_verified:false,office_visual_parity:false}` |
| `create_guided_presentation` | `id`, `input: GuidedInput` | New `{document,validation,profile_id,model_inference:false}`; no existing document or file is replaced |

Profiles are `consulting-decision`, `technical-explainer`, `event-talk`, and `status-report`. The [guided authoring contract](authoring/README.md) defines the evidence, headline ledger, numeric JSON pointers and decision issue fields. All actual document behavior is computed in Rust; no model or source URL is contacted. `ready` means compilable input and measured text layout, not factual or semantic verification. Complete notes retain the supplied ledger/evidence and require privacy review before redistribution.

Optional `input.authoring` controls reading/projection context, comfortable/compact density, standard/relaxed spacing, body font floor (12-40 scene pixels), headline size (28-64) and font family. Omission preserves legacy rendering; `{}` opts into profile defaults. Supplying only `headline_style` and/or `slide_limit` does not activate typography overrides. Existing `brand_color` remains the palette override. Per-slide `speaker_notes` append up to 4000 Unicode scalars without dropping the evidence ledger, subject to the combined 8000-scalar limit. These are creation settings, not a retained styling policy for later part regeneration.

`authoring.headline_style` is `sentence` (default) or `keyword`.
Keyword mode waives only Japanese consulting headline length and adjacent
sentence-form variation checks. Valid `sentence_form`, logical support,
evidence, numeric declarations, forbidden dash/self-reference checks and the
consulting numeric-claim limit still apply. `authoring.slide_limit` is an
integer 32-128, default 32; actual slides must number 1 through that limit and
fit the selected capacity profile. For a 39-page technical outline use
`profile_id:"technical-explainer"` with
`authoring:{headline_style:"keyword",slide_limit:39}`; this sets a ceiling,
not a request to generate missing pages. Omitted options retain prior behavior.

All profiles accept `native-part` with an existing `PartSpec`. Consulting multi-page inputs require 3-6 stable issues, an opening `C02` summary and closing `C03` decision grid. Both are dedicated native templates with purpose-sized columns; analysis references must point to real body pages. The 48-item consulting catalog is selection guidance with explicit `native-template`, `composition-required`, or `guidance-only` status, not 48 implemented automatic templates.

MCP exposes the same four operation names. It allocates an opaque new `deck_id` only after successful validation and cancellation checks, within the existing eight-deck limit. File output still requires a separate `export_pptx` call. SDK creation returns `{session,validation,profile_id,model_inference:false}` through `client.createGuidedPresentation(id,input,options?)`; getter/validator methods are `bestPracticeProfiles`, `bestPracticeGuide` and `validateGuidedPresentation`. Use the returned revision; creation and later manual edits are not a continuously maintained semantic-proof record.

## Metadata Parts

| Operation | Request fields besides op | Result |
| --- | --- | --- |
| `part_catalog` | None | Version, 108 presets, Rust-derived `PartSpec` schema, style and default bounds |
| `create_part` | `id`, `spec`, optional `theme` | Validated ordinary `Element::Group`; does not create persistent metadata |
| `insert_part` | `document`, `expected_revision`, `slide_id`, `id`, `spec` | Atomic `TransactionResult`, including part metadata and undo receipt |
| `update_part` | Same as insert | Regenerates a current metadata part; retains placement unless an explicit layout changes it |

`PartSpec` is `{version:1, preset, title, subtitle?, data, layout?}`. Preset IDs are `<category>/balanced`, `<category>/focus` or `<category>/labeled`. Root IDs are nonempty and at most 40 characters; title/subtitle limits are 80/120. The catalog provides a valid synthetic example for every preset. Unknown fields and unsupported preset/data combinations fail.

`data.kind` selects a strict union: `chart` (categories, series, x_axis/y_axis), `items` (label/detail/value, center), `tree` (id/label/parent nodes), `network` (nodes and indexed edges), `matrix` (rows, columns, rectangular cells), `groups` (named item groups), `timeline` (periods and indexed tasks), `waterfall` (steps, totals, unit), or `map` (named longitude/latitude/value points). See [parts limits and categories](testing/parts-library.md).

`PartData` with `kind:"matrix"` also accepts `corner_label?: string` for the
row/column heading intersection. Omission defaults to the empty string;
`null` and nonstrings reject. The limit is 48 Unicode scalars, with XML 1.0
character validation and the usual layout bounds/fit checks. It applies to
both `matrix` and `contrast` in all three variants: `balanced`, `focus` and
`labeled`. Balanced/focus render editable native text; labeled uses the
native table's top-left cell. For example, use `"corner_label":"Criterion"`;
source-provided Japanese labels are also supported with suitable fonts.
Empty labels are omitted from canonical serialization and retain the old
blank rendering, hashes and generated IDs. A nonempty label changes the
specification and its derived rendering identities normally.

New parts normally use theme-linked native objects at `{x:64,y:144,width:1152,height:512}`. In `preset-visual-content`, insertion instead fits the native group to that layout's `preset-visual-region`, preserving aspect ratio and normalizing child coordinates, fonts and strokes. Updates preserve the fitted coordinate space and outer placement. The ordinary compiler's text fitting bottoms out at 12px; the explicit visual-region fit follows the model's 8px floor and can require a wider layout for dense content. Chart values remain raw in embedded XLSX, including percentage stacks. This is deterministic compilation, not model inference.

An explicit `layout:{x,y,width,height,show_title?}` bypasses automatic preset
region fitting. Coordinates must be finite and nonnegative, dimensions
positive, and both the frame values and right/bottom edges within 4096px;
insertion must also fit the actual slide. `show_title` defaults true. False
removes the part title/subtitle and maps its body below the former 88px band
into the requested frame. Geometry and absolute table tracks are normalized;
text fitting enforces a 12px floor and rejects measured overflow or missing
glyphs. It does not guarantee that every preset fits every region. Some chart
axis labels above that 88px boundary make title removal reject; retain the
title or compose explicitly. Chart-internal typography is not newly qualified
by this text/table measurement. Omitted `layout` preserves the existing
placement path. Retain the layout specification when regenerating a part.

This placement override applies to part creation/direct insertion. Guided
creation subsequently fits its part into the guided body region, so use
direct insertion or complete typed elements for exact slide coordinates.

`Document.parts` is optional when empty. An instance contains `slide_id`, `element_id`, `spec`, `render_sha256`, optional `native_sha256`, and `stale`. Hashes cover rendered children and original native group/resources, including chart workbooks. Root-only movement/resizing in Studio is allowed; mismatched manual/native edits mark the part stale and block semantic update. Missing fingerprints on existing native roots also mark stale. There is no automatic regeneration, no cryptographic authentication, and benign external XML reserialization can conservatively mark stale. Ordinary native objects remain available even without metadata.

SDK: `client.partCatalog()`, `client.createPart({id,spec,theme?})`, `session.addPart(slideId,{id,spec},options?)`, `session.updatePart(slideId,{id,spec},options?)`. Session options accept `expectedRevision` and cancellation; late transport success after cancellation cannot commit. MCP: `part_catalog`, `create_part`, `add_part`, `update_part`; mutations require `deck_id`, `expected_revision`, `slide_id`, `id`, `spec` and use the same core/session gates.

For mixed reusable content, prefer [typed managed batches](#typed-authoring-batches)
through `session.applyOperations` / MCP `apply_operations`. Individual managed
MCP tools retain their legacy revision-only arguments, not `expected_hash`.
Read metadata through MCP `get_document({deck_id})` and its `parts` array;
there is no registered `get_part` or `get_parts` tool. This full-document read
can include sources/origin; use `get_graph` for a narrower graph read. Export
and reopen retain supported part metadata, but not the in-memory Undo history.

## Architecture Graphs

Graphs have a separate catalog, not an additional preset counted among the 108 parts.

| Core operation | Fields besides op | Result |
| --- | --- | --- |
| `graph_catalog` | None | Six shapes, ports, routes, limits, schemas, three synthetic examples |
| `architecture_icons` | None | Read-only `{version:1,release,configured,message,providers,icons}` compiled catalog |
| `architecture_icon_assets` | `ids`: 1-60 distinct catalog strings, each at most 256 bytes | Read-only `{icons:[{id,base64,mime_type,alt,width,height}]}`; PNG bytes from the consented local pack |
| `create_graph_icon` | `base64`, `mime_type`, optional `alt` | Validated `GraphIcon`; SVG to PNG, larger PNG/JPEG fitted to 256px, no document mutation |
| `create_graph` | `id`, `spec`, optional `theme` | Native group for preview; no persistent metadata |
| `transform_graph` | `spec`, `operations` | Validated candidate specification; no document mutation |
| `insert_graph` / `update_graph` | `document`, `expected_revision`, `slide_id`, `id`, `spec` | Atomic transaction, metadata and inverse receipt |
| `apply_graph` | Same identity fields, `operations` instead of `spec` | Applies operations to a current managed graph in one transaction |

`GraphSpec` remains backward-compatible version 1: `{version:1,title,subtitle?,show_title?,nodes,edges?,groups?}`. Coordinates are absolute within 1152x512, with nodes/boundaries below the 88px title band by default. `show_title:false` omits title/subtitle objects and permits content from y=0; omission retains the band. Minimum size is 64x40; card nodes default to 176x80. There are 1-48 nodes, at most 64 edges and 16 boundaries, with boundary depth at most four. IDs are 1-24 ASCII letters/digits/hyphens/underscores, unique across all three collections. Root ID limit is 40 characters. Title/subtitle limits are 80/120; node labels 160, edge/boundary labels 64. The rendered scene still obeys 256 elements per slide and document size limits; not every combination of collection maxima fits.

- Node: `{id,label,detail?,detail_font_size?,text_align?,heading_bold?,kind?,x,y,width?,height?,fill?,stroke?,color?,font_size?,group?,icon?,presentation?}`. Kinds: `rectangle`, `rounded_rectangle`, `ellipse`, `diamond`, `cylinder`, `cloud`. Font size 12-40, default 18; colors accept RGB/theme references. Default fill/outline/text: `@lt1`/`@accent1`/`@dk1`. `presentation` is `card` (default) or `icon`; icon mode requires an icon and retains the six kinds and four logical ports.
- Edge: `{id,source,target,source_port?,target_port?,label?,route?,color?,arrow?,start_arrow?,dashed?}`. Ports: `auto`, `top`, `left`, `bottom`, `right`. Route: `straight` (default) or `elbow`. End arrow defaults true; start arrow/dashed false; color `@dk2`. Endpoints must reference distinct existing nodes.
- Boundary: `{id,label,x,y,width,height,fill?,stroke?,parent?,icon?}`. Defaults `@lt2`/`@dk2`, with solid outlines. A member or child boundary must fit below its parent's 40px heading with 8px side/bottom padding. Missing parents, cycles, excess depth and containment violations reject. Core coordinates stay absolute; React Flow uses relative child positions only in the UI, with stable outer-first rendering.

Node `detail` is a nonblank description of at most 240 characters, rendered as
separate editable native body text below the heading with an 8px gap, not
flattened into an image. `detail_font_size` requires detail, accepts 12-40 and
must not exceed `font_size`; default is `max(12,font_size*0.8)`.
`text_align` accepts left/center/right, defaulting to left with detail and
center without it. `heading_bold` defaults true; detail text is not bold.
Omission retains the previous label-only presentation. Small frames or long
text can fail fitting. `diagram/custom` parts also accept an explicit
`PartSpec.layout`; its `show_title` controls the rendered graph title, with
the old band removed from coordinates when hiding a previously titled graph.

`GraphIcon` is `{base64,mime_type,alt?}` with PNG/JPEG bytes, not a path, URL or raw SVG. Set `icon` to null or omit it on a complete replacement to remove an icon; an icon-mode node must also return to `presentation: "card"`. Each payload uses the existing 1 MiB/4096px/64 MiB decoder bounds; total encoded node and boundary icons share a 3 MiB budget. Document/metadata/response budgets can reject a graph before these maxima. Use `create_graph_icon` for generic imported images: inert SVG becomes PNG at a 256px longest side, larger PNG/JPEG images are downsampled in their format, and smaller rasters retain their bytes. Prepared cloud PNGs bypass this helper to preserve originals up to 512px. Normal `create_asset` behavior is unchanged. Alt text is at most 500 characters.

Omitted presentation preserves legacy card output, including the aspect-fitted picture beside the label at up to 48 graph pixels. Icon mode places a picture above an editable label with a transparent native connection anchor; Studio service icons start at 160x140 with 16px labels. Shape/anchor, picture and label remain separate native objects in the graph root. Logical boundary nesting is metadata, not nested PowerPoint groups. Graph-editor movement moves the complete node; direct Office movement of only its shape does not move the label/picture. Vendor colors and proportions remain unchanged, not live theme bindings.

Operation batches contain 1-128 entries. `put_node`, `put_edge`, `put_group` carry a complete typed `node`, `edge` or `group`. `move` takes `{ids,dx,dy}`; `remove` takes `{ids}`; `align` takes `{ids,alignment}` with left/center/right/top/middle/bottom; `layout` takes `{columns}` in 1-8. Selection is 1-120 unique existing IDs. Alignment requires at least two nodes. Moving an ancestor moves every descendant once, including multi-selection. Removing a node removes incident edges; removing boundaries promotes direct children to the nearest remaining parent without changing absolute bounds. Grid preserves sizes and fails when they do not fit; with boundaries, all nodes must belong to a group. Grid is disabled/rejected for nested boundaries. Validation is atomic.

Native output uses ordinary shapes, separate label text and attached `p:cxnSp` objects with `straightConnector1` or `bentConnector2/3/4`. Shape-specific connection indices are independent of logical port direction. Standard adjustment values/flips retain reverse and vertical routes. Raw connector `routing` accepts only representable 2-4 normalized points plus `start_arrow`/`dashed`; arbitrary waypoints, curves and unsupported formulas fail closed. Routing is deterministic midpoint routing, not obstacle avoidance. In Office, moving only a native node shape does not automatically move its separate text label.

Managed graphs use `Document.parts` with preset `diagram/custom` and data `{kind:"diagram",graph:spec}`. They share native/render fingerprints, root placement preservation and Undo/Redo with parts. External edits can conservatively mark metadata stale; graph updates then fail rather than overwrite manual content. Ordinary native objects remain editable without metadata only within the supported native subset; unknown XML still guards against unsupported replacement.

The recommended mixed-batch `add_graph` accepts top-level
`layout?: PartLayout | null`; `update_graph` accepts no layout and preserves
the existing part layout. This is distinct from `GraphSpec.show_title`.
Use `get_document({deck_id})` and its `parts` array to inspect `spec.layout`, since
`get_graph` returns the graph specification and stale status, not PartLayout.
Legacy individual graph tools keep their existing payloads without a layout
argument. Managed graphs count toward the same 128-entry metadata total.

SDK: `client.graphCatalog()`, `client.createGraphIcon({base64,mime_type,alt?})`, `client.createGraph({id,spec,theme?})`, `client.transformGraph(spec,operations)`, `session.addGraph(slideId,{id,spec},options?)`, `session.updateGraph(...)`, `session.applyGraph(slideId,{id,operations},options?)`. Options include `expectedRevision` and cancellation. MCP uses `add_graph` for `insert_graph`; other names are unchanged. Mutations require `deck_id`, `expected_revision`, `slide_id`, `id` and payload. `get_graph` takes `deck_id`, `slide_id`, `id` and returns `{deck_id,revision,slide_id,element_id,spec,stale}` without unrelated sources. No new filesystem/network authority is granted. See [verification](testing/graphs-and-dads.md).

Catalog SDK methods are `client.architectureIcons(options?)` and `client.architectureIconAssets(ids,options?)`; MCP uses the two read-only core names. `configured` checks consent for this exact catalog, not all assets. Each asset batch rechecks consent and all selected file hashes, PNG magic/dimensions, 1 MiB per-file and 4 MiB raw-batch limits before any return. Strip `id`, `width` and `height` before passing assets to strict `GraphIcon`. Requests accept no paths, URLs or manifests. Only the host may set `AISLIDE_ICON_PACK_ROOT` to an absolute local version directory. Lexically invalid paths and Windows remote/unknown drives reject before filesystem probes; subsequent ancestor/opened-file checks reject symlinks and reparse points. There is no runtime network access or external-relationship fetch. See [setup, rights and concurrent-filesystem limitations](authoring/cloud-icons.md).

## Source And Generation Operations

`ingest` takes `input: {name, format, base64, ocr?, ocr_language?, attribution?}`. Formats are `csv`, `json`, `xlsx`, `markdown`, `text`, `pdf`, `png`, `jpeg`. Attribution fields are `citation`, `url`, `license`, optional `derived_from_sha256` and `transformation`. URLs are metadata only. The result preserves raw-byte identity and a separate extracted-content hash, source tables with exact locators, plain text, page/word evidence, and limitations.

`data_report` takes a source document and `mapping: {title, period, table_index, category_column, value_columns, row_start, row_count, chart_kind}`. Indices are zero-based. It creates exactly twelve slides, native charts/tables/process graphics, and field bindings; it is deterministic source compilation, not AI generation. Missing/formula/nonnumeric selected cells fail rather than becoming zero.

MCP `compile_report` compiles a supplied `ReportInput` deterministically. Its
seven fixed section layouts are `cover`, `metrics`, `table`, `columns`,
`statement`, `chart` and `process`; they are not a general PPTX replication
engine. Use guided technical outlines, parts/native graphs or complete typed
elements according to the required evidence structure and placement control.
Design presets remain optional, unchanged styles. Typed batches do not add
SVG caching, a stateful core, a general report-layout catalog, or preflight
overlap aggregation, finding prioritization or contrast analysis.

`generate` takes `input: {prompt, source_text?, slide_count, allow_remote?, max_repairs?, outline?}`. Each outline entry is `{title, layout}`. Model configuration comes only from the host environment. `max_repairs` accepts 0 or 1, default 0; repair is a separately authorized model call for report validation, not HTTP retry. Provenance includes model, local/remote mode, source hash, duration, attempts and `verified:false`. Model output never grants filesystem/network authority.

The core's report schema is generated from Rust types in schema mode and further constrained by the requested count/outline. Not every OpenAI-compatible server supports every JSON Schema keyword; runtime validation remains authoritative. The pinned local llama.cpp qualification includes a regression for `items` masking `prefixItems`.

## Graphics And Inspection

Review operations `modern_comment`, `set_table_headers`, `check_accessibility`, `inspect_document` and `export_clean_copy` share the core/SDK/MCP contracts in [Phase 6 review](authoring/review-phase6.md). Modern threads are optional and separate from legacy comments; repeated rich-body edit/reopen preserves end-paragraph namespaces, while native thread/reply array reordering rejects rather than being silently ignored. PII candidates are masked, numeric-location manual findings and never cleanup categories. Inspection does not return content-derived paths. PDF consumes explicit table-header metadata; anchor, old-client omission and Office/assistive-technology limits remain explicit.

- `object_catalog`: no fields; returns the allowlisted preset shapes, 24 chart kinds (18 classic and six chartEx) and table limits. See the [chartEx contract and histogram compatibility exception](authoring/chart-ex.md); the Office-compatible histogram encoding has two Open XML schema errors per histogram resource.
- `create_object`: `id`, `kind` (`text`, `shape`, `table`, `chart`, `line`, `arrow`), optional `preset`, `rows`, `columns`; returns a validated element. Shape presets are OOXML names from the catalog; chart presets are chart kinds. Tables are 1-12 rows by 1-8 columns. Factory chart values are synthetic.
- `design_defaults`: no fields; returns a theme, one master and four layouts without changing a document.
- `design_presets`: no fields; returns seven original `{id,name,design,rules}` presets. Rules include side margin, gutter, heading/body sizes and named layout regions.
- `apply_design_preset`: `deck`, `preset_id`; accepts `public`, `minimal`, `stylish`, `pop`, `dynamic`, `trust` or `luxury`. Adds or replaces a verified dedicated preset master and seven layouts, retaining original masters and slide content. Foreign/edited templates, conflicting IDs and insufficient capacity fail before commit. See [preset details](testing/master-presets.md).
- `update_design`: `deck`, `design`; validates the complete design and propagates inherited placeholder geometry/style while preserving text.
- `apply_theme`: `deck`, `theme`; binds previous-palette RGB values to theme slots and retains unrelated custom colors. Does not install fonts or fetch resources.
- `set_master_theme` / `set_design_field`: owner-scoped theme overrides and dynamic-field/footer updates; see the [typed master-field contract](authoring/master-fields-themes.md#interfaces). `design_capabilities` reports the supported design subset and limitations.
- `assign_layout`: `deck`, `slide_id`, `layout_id`; assigns/resets a layout, retains matching text, creates missing placeholders and detaches obsolete ones.
- `create_picture`: `id`, `base64`, `mime_type`, `alt`; returns a decoded, fitted picture element.
- `create_diagram`: `id`, `steps`; returns native rectangles, text and connected arrows in a group.
- `compile`: `report`; returns a scene and issues. `sample` returns the explicitly synthetic twelve-slide input.
- `validate`: `deck`; returns structural status and a canonical deck with serde defaults applied.
- `measure_layout`: `deck`; reports text/table metrics, fonts, overflow and missing glyphs, never Office parity.
- `inspect`: `base64`; lists simple top-level text runs.
- `patch_text`: `base64`, `part`, `shape_id`, `run_index`, `expected`, `text`; returns a new archive or an optimistic conflict.
- `import_pptx`: `base64`; approximate scene, warnings and writable fields.
- `save_import`: `base64`, `deck`; original bytes for no-op or narrowly patched original package.
- `roundtrip`: `base64`; inspected no-op archive. `package_manifest`: `base64`; all part byte lengths and SHA-256 values.
- `export`: `deck`; structural-only PPTX. `provider_status` and `ocr_status` report configuration/capability, not content quality.

## Authoring Model

`deck.design` is optional and contains `{theme, masters, layouts}`. Each master is `{id, name, background, elements, theme?}`; each layout is `{id, name, master_id, background?, elements}`. A master's optional theme overrides the default design theme for its layouts and slides; omitting it restores inheritance. Background colors and element colors accept six-digit RGB or `@dk1`, `@lt1`, `@dk2`, `@lt2`, `@accent1` through `@accent6`, `@hlink`, `@folHlink`. Theme slots themselves must be RGB, not references. Theme fonts are `{major, minor, east_asian, complex_script}`. See [master fields and themes](authoring/master-fields-themes.md) for owner isolation and native preservation limits.

Slides optionally contain `layout_id`, `inherit_background`, `hide_master_graphics`. Text and shape elements accept `format: {italic?, underline?, alignment?, vertical?, bullet?, font_family?, hyperlink?, placeholder?, inherit_layout?, paragraphs?}`. Alignment is left/center/right/justify; vertical is top/middle/bottom; bullet is none/bullet/numbered. Fonts may be explicit names or `@major`/`@minor`. Hyperlinks are absolute HTTP(S)/mailto URLs without embedded credentials or control characters; the application never fetches or activates them.

`format.paragraphs` represents rich runs and paragraph formatting, not HTML. It supports run styles, numbering, indentation, spacing and tab stops within bounded typed inputs. When paragraphs are present, the element's `text` must equal their run texts joined with newline separators. Unknown or unrepresentable native rich content remains protected rather than flattened. Font measurement and rendering do not establish Office typography parity.

Optional `visual` styles represent supported transforms, visibility/locking, gradients, effects, picture masks and text warps; applicability and bounds depend on element type. These are modeled DrawingML properties, not arbitrary CSS or full Office effect support.

`polygon` elements contain bounds, 3-4096 normalized finite `[x,y]` points in `[0,1]`, fill, stroke and stroke_width. Shape/polygon fill accepts `none`. Straight polygons export as a closed native DrawingML path. Optional `visual.path` adds bounded move/line/quadratic/cubic/close commands and supported compound contours with holes; `points` must match its complete point/control-point list. Native point, path and style edits require representable original geometry and affected XML. Boolean operations, including fragment, adaptively flatten supported curves with a 0.25px target before polygon clipping; this is not exact arbitrary curved geometry. Arbitrary formulas, arcs and multiple separate native path records remain unsupported and preserved.

Placeholders are top-level text elements with `{kind, index}`. Kinds are title/body/subtitle/footer/date/slide_number. An inherited slide placeholder must match its layout geometry and style. Use the design/layout operations, or set `inherit_layout:false` before individual changes. These pure core operations return a candidate deck; commit it using `transaction` for revision/hash checks, source binding checks and undo. SDK session methods perform both steps under one busy guard.

Master/layout-owned fields support dynamic slide numbers (`slidenum`), explicit-date caches and ordinary footer placeholder text. `refresh_fields` evaluates `datetime1..13` for `en-US`, with caller `reference_date` and optional `reference_time` (`HH:mm:ss`, required for datetime8..13). Other locales, missing times and unknown kinds retain caches and return warnings. Known field instances retain inheritance after native reopen without replacing their UUIDs, caches or raw paragraph extensions. See the [field contract](authoring/master-fields-themes.md#fields).

`update_rich_notes` accepts `{document,expected_revision,slide_id,paragraphs}` and atomically synchronizes mandatory `Slide.notes` with optional `notes_paragraphs`. Notes allow 8,000 Unicode scalars; ordinary rich text still allows 4,000. `update_auxiliary_design` accepts `{document,expected_revision,design}` where design is `{width,height,notes_master?,handout_master?}` and each master is `{name,background,theme,elements}`. These are separate auxiliary masters, not slide masters requiring layouts. Both return normal transaction results and support Undo. MCP uses `deck_id`; SDK exposes `updateRichNotes` and `updateAuxiliaryDesign`. Native unsupported paragraph edits, ambiguous masters, auxiliary removal and native notes-page resizing fail closed. See [native preservation and UI limits](authoring/master-fields-themes.md#rich-notes-and-auxiliary-masters).

Imported origins remain immutable. Supported design, ordinary-shape, rich-text, visual and path edits use native preservation checks against the original package. A newly modeled property is not automatically forbidden solely because it was absent before, but unsupported original XML cannot be silently replaced. Check the returned object capabilities and operation-specific limits; a preview is not authority to rewrite arbitrary imported content. No external checkpoint is required for supported single-file edits.

## Capability Discovery

Core/MCP `authoring_capabilities` and SDK `client.authoringCapabilities()` report supported authoring operations and limits, not a guarantee that an arbitrary imported object is writable. The [SDK method declarations](../packages/client/index.d.mts) and [payload types](../packages/client/types.ts) define search/replacement, selection, image/table/vector editing, bounded Boolean operations, review and comments. Their native preservation, revision and cancellation requirements still apply.

See the [current support boundaries](support-matrix.md#current-single-file-path), [local proofing and text-format painter](authoring/proofing-format-painter.md), [local-only model assistance and segmentation](authoring/local-ai.md), [opt-in document fonts](authoring/fonts.md), [master fields and themes](authoring/master-fields-themes.md), and [chartEx contract](authoring/chart-ex.md). Capability discovery itself performs no inference and certifies neither model quality nor Office/font compatibility. Optional proofread/translation and U2NetP segmentation use the separate local-only contracts, review/revision/cancellation guards and rights requirements.

## MCP Mapping

MCP retains up to eight document and source handles. `ingest_source` returns a source handle; `compile_data_report` consumes it. `get_document` includes revision and sources, while `get_deck` retains the legacy scene-only shape. `apply_transaction` requires a revision. `update_text`, `undo`, `redo`, `add_picture`, `add_diagram`, `measure_layout`, `import_pptx`, `open_project`, `export_project` and legacy `export_pptx` use the shared document/session behavior.

`object_catalog`, `design_defaults` and `design_presets` are read-only MCP tools; `design_presets` wraps its array in `{presets}` for MCP structured content. `add_object`, `update_design`, `apply_design_preset`, `apply_theme`, and `assign_layout` require `deck_id` and `expected_revision`, plus their operation-specific inputs. `add_object` also requires `slide_id`. `update_text` accepts top-level text boxes and preset-shape text. All these mutations are undoable and reject stale revisions.

Only `--output-dir` enables MCP file output. Each export accepts a bounded plain filename with the exact extension required by its registered operation: `.pptx` for presentation and clean-copy exports, `.potx` or `.thmx` matching the template kind, and `.png`, `.jpg` or `.pdf` matching the static format. Legacy `export_project` accepts a `.pptx` name and derives its paired `.aislide.json` checkpoint name; it does not accept an arbitrary checkpoint path. No generic path or shell operation exists. Model tools declare external-world behavior because operator-approved remote inference is possible. Imported relationships remain inaccessible regardless of model permissions.