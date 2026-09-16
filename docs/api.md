# Shared API Contracts

Every operation is strict JSON with a discriminator `op`. Unknown fields fail. The CLI reads one request from stdin and prints one response; errors use stderr and a nonzero exit code. Tauri and the development adapter share the same dispatch in `aislide-core`.

## Single-file PPTX

`open_presentation` takes `{id, base64}` for one nonencrypted standard Open XML PPTX and returns `{document, warnings, objects, format:"open_xml_pptx"}`. It reads current native slide, master, layout, theme, table, chart and picture XML. It never uses an external scene checkpoint as the source of content. The returned immutable origin identifies the original package for copy-through edits, not a second file the caller must retain.

`export_presentation` takes `{document}` and returns `{base64, filename, layout}` without a checkpoint. Supported edits patch existing XML and preserve other parts. Sources, source bindings, part specifications and stable-ID hints are stored as bounded structured Custom XML; no cached deck is embedded. Existing source staleness checks still apply. Save uses create-new semantics, not silent overwrite.

SDK: `client.openPresentation(id, base64)` returns a session and warnings; `session.exportPresentation()` returns the single-file export. MCP: `open_pptx` opens the caller-supplied bytes; `export_pptx` writes only one file to the operator-approved output directory. `open_project`, `export_project`, `import_pptx` and the legacy SDK equivalents remain compatibility APIs, not the Studio file workflow.

OLE containers, which include encrypted/protected Office files and legacy `.ppt`, receive an explicit format error before ZIP parsing. AISlide does not decrypt, remove labels or alter protection settings. The native reader currently supports Transitional PresentationML, not the separate ISO Strict namespace profile. Reopened master/layout counts stay fixed. Supported slide structure commands are described below; complex text or chart changes outside the representable subset are rejected rather than flattened.

## Document Operations

| Operation | Request fields besides op | Result |
| --- | --- | --- |
| `new_document` | `id`, `deck`, optional `sources`, `bindings`, `report` | Canonical document with revision 0 and content hash |
| `transaction` | `document`, `transaction: {expected_revision, expected_hash, operations}` | Updated document, inverse receipt, changed JSON paths |
| `undo_transaction` | `document`, `expected_revision`, `receipt` | Updated document and a receipt usable for redo |
| `export_project` | `document` | `base64`, `filename`, `checkpoint`, `checkpoint_filename`, layout evidence |
| `open_project` | `base64`, `checkpoint` | Verified document, only if the exact PPTX matches |
| `import_document` | `id`, `base64` | Origin-bound document, warnings, per-object editable fields |

Content JSON Patch paths start with `/deck`, `/sources`, `/bindings`, `/parts`, `/report` or the immutable import origin. A batch has 1-128 operations; a stale revision/hash, failed test/path, oversized intermediate result or invalid final state aborts the batch. Revisions increase on real changes and Undo/Redo; no-op transactions do not create history. Deleting a part root also removes its metadata in that transaction, so Undo restores both. Core receipts are not signed capabilities and undergo the same validation as edits.

## Workspace Commands

| Core operation | Fields besides op | Result |
| --- | --- | --- |
| `create_presentation` | `id`, `title` | Revision-zero document with one blank slide and the default design |
| `edit_slides` | `document`, `expected_revision`, `operations` | Atomic transaction and inverse receipt |
| `edit_elements` | Same, plus `slide_id` | Atomic top-level element operation |
| `create_asset` | `id`, `base64`, `mime_type`, `alt`, `size` | Validated picture element; SVG is converted to PNG |

Slide operations, in a batch of 1-128:

- `{op:"insert",id,after?,title,layout_id?}` inserts after the specified slide, or appends when omitted. An omitted layout produces a blank slide; an explicit layout creates its placeholders.
- `{op:"duplicate",slide_id,id}` creates a copy immediately after its source. Part metadata and source bindings follow the new slide ID; stale part metadata blocks duplication.
- `{op:"remove",slide_id}` removes the slide and its part/binding records; the last remaining slide cannot be removed.
- `{op:"move",slide_id,index}` moves to a zero-based index in the final list.
- `{op:"rename",slide_id,title}` changes its displayed title, not the contents of text boxes.

Slides remain limited to 1-32. Native insertion creates only the added slide/resources and relationships; existing slide bytes remain untouched when their content is unchanged. Native duplication preserves opaque slide XML and independently copies mutable resources such as chart XML and embedded workbooks. It may share immutable images and design resources. A bounded origin-relative `native_source_id` is used internally for in-session copies; callers should use `edit_slides`, not set it directly.

Native slide deletion removes its slide XML, dedicated notes, relationship parts and their content-type entries. Deletion is refused if a remaining object references those parts. Other shared or opaque package resources and source documents are retained; deletion is not secure redaction. Sections/custom slide shows must be handled in PowerPoint before structural changes. Copy traversal is capped at 128 related resources and 8 MiB. Original files and import origins remain unchanged.

Element operations are `{op:"duplicate",id,new_id}`, `{op:"remove",id}` or `{op:"order",id,index}`. They apply to the selected slide's top-level objects. Duplication remaps descendant IDs and connection references and copies bindings/part metadata. Removal also removes connectors that lose their target. Unsupported individual native copies fail rather than flatten custom formatting; duplicating the slide is the preservation-oriented alternative. All operations are undoable and require a current revision.

Assets accept `image/svg+xml`, `image/png` or `image/jpeg`. `size` is the displayed longest side, 8-640px; aspect ratio is retained. Raster files use the existing 1 MiB, 4096px and 64 MiB decode limits. SVG is limited to 256 KiB, 2,048 elements, XML depth 32, viewport 4096px, reference depth 48 and expanded reference cost 4,096. Cycles, missing/wrong-type fragment targets, external resources, scripts, `foreignObject`, text, embedded images, styles, filters and unsupported effects are rejected. Outlined paths, basic shapes, gradients, clipping and masks are supported within those limits. Raster output has a 1024px longest side, transparency and a 1 MiB limit, and is decoded again as PNG before use. The SVG is not embedded in PPTX or rendered as HTML. These budgets are not an OS process sandbox.

SDK: `client.createPresentation(id,title?,options?)`, `session.editSlides(operations,options?)`, `session.editElements(slideId,operations,options?)`, `client.createAsset(input,options?)`, `session.addAsset(slideId,input,options?)`. Session options retain `expectedRevision` and cancellation. MCP exposes the same names with `add_asset` for insertion; mutations take `deck_id`, `expected_revision` and, for assets/elements, `slide_id`. `create_asset` is stateless. No additional filesystem/network authority is exposed.

## Metadata Parts

| Operation | Request fields besides op | Result |
| --- | --- | --- |
| `part_catalog` | None | Version, 108 presets, Rust-derived `PartSpec` schema, style and default bounds |
| `create_part` | `id`, `spec`, optional `theme` | Validated ordinary `Element::Group`; does not create persistent metadata |
| `insert_part` | `document`, `expected_revision`, `slide_id`, `id`, `spec` | Atomic `TransactionResult`, including part metadata and undo receipt |
| `update_part` | Same as insert | Replaces a current metadata part, retaining its root position and size |

`PartSpec` is `{version:1, preset, title, subtitle?, data}`. Preset IDs are `<category>/balanced`, `<category>/focus` or `<category>/labeled`. Root IDs are nonempty and at most 40 characters; title/subtitle limits are 80/120. The catalog provides a valid synthetic example for every preset. Unknown fields and unsupported preset/data combinations fail.

`data.kind` selects a strict union: `chart` (categories, series, x_axis/y_axis), `items` (label/detail/value, center), `tree` (id/label/parent nodes), `network` (nodes and indexed edges), `matrix` (rows, columns, rectangular cells), `groups` (named item groups), `timeline` (periods and indexed tasks), `waterfall` (steps, totals, unit), or `map` (named longitude/latitude/value points). See [parts limits and categories](testing/parts-library.md).

New parts normally use theme-linked native objects at `{x:64,y:144,width:1152,height:512}`. In `preset-visual-content`, insertion instead fits the native group to that layout's `preset-visual-region`, preserving aspect ratio and normalizing child coordinates, fonts and strokes. Updates preserve the fitted coordinate space and outer placement. The ordinary compiler's text fitting bottoms out at 12px; the explicit visual-region fit follows the model's 8px floor and can require a wider layout for dense content. Chart values remain raw in embedded XLSX, including percentage stacks. This is deterministic compilation, not model inference.

`Document.parts` is optional when empty. An instance contains `slide_id`, `element_id`, `spec`, `render_sha256`, optional `native_sha256`, and `stale`. Hashes cover rendered children and original native group/resources, including chart workbooks. Root-only movement/resizing in Studio is allowed; mismatched manual/native edits mark the part stale and block semantic update. Missing fingerprints on existing native roots also mark stale. There is no automatic regeneration, no cryptographic authentication, and benign external XML reserialization can conservatively mark stale. Ordinary native objects remain available even without metadata.

SDK: `client.partCatalog()`, `client.createPart({id,spec,theme?})`, `session.addPart(slideId,{id,spec},options?)`, `session.updatePart(slideId,{id,spec},options?)`. Session options accept `expectedRevision` and cancellation; late transport success after cancellation cannot commit. MCP: `part_catalog`, `create_part`, `add_part`, `update_part`; mutations require `deck_id`, `expected_revision`, `slide_id`, `id`, `spec` and use the same core/session gates.

## Architecture Graphs

Graphs have a separate catalog, not an additional preset counted among the 108 parts.

| Core operation | Fields besides op | Result |
| --- | --- | --- |
| `graph_catalog` | None | Six shapes, ports, routes, limits, schemas, three synthetic examples |
| `create_graph_icon` | `base64`, `mime_type`, optional `alt` | Validated `GraphIcon`; SVG to PNG, larger PNG/JPEG fitted to 256px, no document mutation |
| `create_graph` | `id`, `spec`, optional `theme` | Native group for preview; no persistent metadata |
| `transform_graph` | `spec`, `operations` | Validated candidate specification; no document mutation |
| `insert_graph` / `update_graph` | `document`, `expected_revision`, `slide_id`, `id`, `spec` | Atomic transaction, metadata and inverse receipt |
| `apply_graph` | Same identity fields, `operations` instead of `spec` | Applies operations to a current managed graph in one transaction |

`GraphSpec` is `{version:1,title,subtitle?,nodes,edges?,groups?}`. Coordinates are 1152x512, with nodes/boundaries below the 88px title band. Minimum size is 64x40; nodes default to 176x80. There are 1-48 nodes, at most 64 edges and 8 boundaries. IDs are 1-24 ASCII letters/digits/hyphens/underscores, unique across all three collections. Root ID limit is 40 characters. Title/subtitle limits are 80/120; node labels 160, edge/boundary labels 64. The rendered scene still obeys 256 elements per slide and document size limits; not every combination of collection maxima fits.

- Node: `{id,label,kind?,x,y,width?,height?,fill?,stroke?,color?,font_size?,group?,icon?}`. Kinds: `rectangle`, `rounded_rectangle`, `ellipse`, `diamond`, `cylinder`, `cloud`. Font size 12-40, default 18; colors accept RGB/theme references. Default fill/outline/text: `@lt1`/`@accent1`/`@dk1`.
- Edge: `{id,source,target,source_port?,target_port?,label?,route?,color?,arrow?,start_arrow?,dashed?}`. Ports: `auto`, `top`, `left`, `bottom`, `right`. Route: `straight` (default) or `elbow`. End arrow defaults true; start arrow/dashed false; color `@dk2`. Endpoints must reference distinct existing nodes.
- Boundary: `{id,label,x,y,width,height,fill?,stroke?}`. Defaults `@lt2`/`@dk2`. A member must fit below the 40px heading with 8px side/bottom padding. Boundaries cannot nest.

`GraphIcon` is `{base64,mime_type,alt?}` with PNG/JPEG bytes, not a path, URL or raw SVG. Set `icon` to null or omit it to remove an icon. Each payload uses the existing 1 MiB/4096px/64 MiB decoder bounds; the graph additionally limits total encoded icon data to 3 MiB. Document/metadata/response budgets can reject a graph before these maxima. Use `create_graph_icon` before assigning imported images: SVG is checked by the existing inert subset and rasterized at a 256px longest side; larger PNG/JPEG images are downsampled without changing their format, while smaller rasters retain their original bytes. Normal `create_asset` behavior is unchanged. Alt text is at most 500 characters.

The core positions a native picture next to the fitted label within the node's shape insets, preserving aspect ratio at up to 48 graph pixels. Shape, picture and label remain separate native objects in the graph root; connections still attach to the shape. Resizing or moving the node in the graph editor moves its icon and label together. Direct Office movement of only the shape does not move those other objects automatically. Icon colors are the imported raster colors, not live theme bindings; replace the icon to recolor it.

Operation batches contain 1-128 entries. `put_node`, `put_edge`, `put_group` carry a complete typed `node`, `edge` or `group`. `move` takes `{ids,dx,dy}`; `remove` takes `{ids}`; `align` takes `{ids,alignment}` with left/center/right/top/middle/bottom; `layout` takes `{columns}` in 1-8. Selection is 1-120 unique existing IDs. Alignment requires at least two nodes. Group movement moves members once, even when both are selected. Removing a node removes incident edges; removing a group ungroups members. Grid preserves sizes and fails when they do not fit; with boundaries, all nodes must belong to a group. Validation is atomic.

Native output uses ordinary shapes, separate label text and attached `p:cxnSp` objects with `straightConnector1` or `bentConnector2/3/4`. Shape-specific connection indices are independent of logical port direction. Standard adjustment values/flips retain reverse and vertical routes. Raw connector `routing` accepts only representable 2-4 normalized points plus `start_arrow`/`dashed`; arbitrary waypoints, curves and unsupported formulas fail closed. Routing is deterministic midpoint routing, not obstacle avoidance. In Office, moving only a native node shape does not automatically move its separate text label.

Managed graphs use `Document.parts` with preset `diagram/custom` and data `{kind:"diagram",graph:spec}`. They share native/render fingerprints, root placement preservation and Undo/Redo with parts. External edits can conservatively mark metadata stale; graph updates then fail rather than overwrite manual content. Ordinary native objects remain editable without metadata.

SDK: `client.graphCatalog()`, `client.createGraphIcon({base64,mime_type,alt?})`, `client.createGraph({id,spec,theme?})`, `client.transformGraph(spec,operations)`, `session.addGraph(slideId,{id,spec},options?)`, `session.updateGraph(...)`, `session.applyGraph(slideId,{id,operations},options?)`. Options include `expectedRevision` and cancellation. MCP uses `add_graph` for `insert_graph`; other names are unchanged. Mutations require `deck_id`, `expected_revision`, `slide_id`, `id` and payload. `get_graph` takes `deck_id`, `slide_id`, `id` and returns `{deck_id,revision,slide_id,element_id,spec,stale}` without unrelated sources. No new filesystem/network authority is granted. See [verification](testing/graphs-and-dads.md).

## Source And Generation Operations

`ingest` takes `input: {name, format, base64, ocr?, ocr_language?, attribution?}`. Formats are `csv`, `json`, `xlsx`, `markdown`, `text`, `pdf`, `png`, `jpeg`. Attribution fields are `citation`, `url`, `license`, optional `derived_from_sha256` and `transformation`. URLs are metadata only. The result preserves raw-byte identity and a separate extracted-content hash, source tables with exact locators, plain text, page/word evidence, and limitations.

`data_report` takes a source document and `mapping: {title, period, table_index, category_column, value_columns, row_start, row_count, chart_kind}`. Indices are zero-based. It creates exactly twelve slides, native charts/tables/process graphics, and field bindings; it is deterministic source compilation, not AI generation. Missing/formula/nonnumeric selected cells fail rather than becoming zero.

`generate` takes `input: {prompt, source_text?, slide_count, allow_remote?, max_repairs?, outline?}`. Each outline entry is `{title, layout}`. Model configuration comes only from the host environment. `max_repairs` accepts 0 or 1, default 0; repair is a separately authorized model call for report validation, not HTTP retry. Provenance includes model, local/remote mode, source hash, duration, attempts and `verified:false`. Model output never grants filesystem/network authority.

The core's report schema is generated from Rust types in schema mode and further constrained by the requested count/outline. Not every OpenAI-compatible server supports every JSON Schema keyword; runtime validation remains authoritative. The pinned local llama.cpp qualification includes a regression for `items` masking `prefixItems`.

## Graphics And Inspection

- `object_catalog`: no fields; returns the allowlisted preset shapes, nine chart kinds and table limits.
- `create_object`: `id`, `kind` (`text`, `shape`, `table`, `chart`, `line`, `arrow`), optional `preset`, `rows`, `columns`; returns a validated element. Shape presets are OOXML names from the catalog; chart presets are chart kinds. Tables are 1-12 rows by 1-8 columns. Factory chart values are synthetic.
- `design_defaults`: no fields; returns a theme, one master and four layouts without changing a document.
- `design_presets`: no fields; returns seven original `{id,name,design,rules}` presets. Rules include side margin, gutter, heading/body sizes and named layout regions.
- `apply_design_preset`: `deck`, `preset_id`; accepts `public`, `minimal`, `stylish`, `pop`, `dynamic`, `trust` or `luxury`. Adds or replaces a verified dedicated preset master and seven layouts, retaining original masters and slide content. Foreign/edited templates, conflicting IDs and insufficient capacity fail before commit. See [preset details](testing/master-presets.md).
- `update_design`: `deck`, `design`; validates the complete design and propagates inherited placeholder geometry/style while preserving text.
- `apply_theme`: `deck`, `theme`; binds previous-palette RGB values to theme slots and retains unrelated custom colors. Does not install fonts or fetch resources.
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

`deck.design` is optional and contains `{theme, masters, layouts}`. Each master is `{id, name, background, elements}`; each layout is `{id, name, master_id, background?, elements}`. Background colors and element colors accept six-digit RGB or `@dk1`, `@lt1`, `@dk2`, `@lt2`, `@accent1` through `@accent6`, `@hlink`, `@folHlink`. Theme slots themselves must be RGB, not references. Theme fonts are `{major, minor, east_asian, complex_script}`.

Slides optionally contain `layout_id`, `inherit_background`, `hide_master_graphics`. Text and shape elements accept `format: {italic?, underline?, alignment?, vertical?, bullet?, font_family?, hyperlink?, placeholder?, inherit_layout?}`. Alignment is left/center/right/justify; vertical is top/middle/bottom; bullet is none/bullet/numbered. Fonts may be explicit names or `@major`/`@minor`. Hyperlinks are absolute HTTP(S)/mailto URLs without embedded credentials or control characters; the application never fetches or activates them. Plain text only, not arbitrary rich HTML.

`polygon` elements contain bounds, 3-4096 normalized finite `[x,y]` points in `[0,1]`, fill, stroke and stroke_width. They export as a closed native DrawingML path. Shape/polygon fill accepts `none`. The native reader supports simple literal move/line/close paths, not curves, formulas or multiple paths; unsupported geometry is preserved. Reopened polygon geometry can move/resize, but point/style replacement is rejected. `percent_stacked_column` is the tenth chart kind and requires nonnegative values with a positive total in every category.

Placeholders are top-level text elements with `{kind, index}`. Kinds are title/body/subtitle/footer/date/slide_number. An inherited slide placeholder must match its layout geometry and style. Use the design/layout operations, or set `inherit_layout:false` before individual changes. These pure core operations return a candidate deck; commit it using `transaction` for revision/hash checks, source binding checks and undo. SDK session methods perform both steps under one busy guard.

Imported origins remain immutable. Advanced design/object/format changes that cannot be mapped to supported original XML fields are rejected, including fields added to a JSON object that were absent before. Use authored documents or an exact AISlide project checkpoint for advanced design editing, not a regenerated approximation of an arbitrary import.

## MCP Mapping

MCP retains up to eight document and source handles. `ingest_source` returns a source handle; `compile_data_report` consumes it. `get_document` includes revision and sources, while `get_deck` retains the legacy scene-only shape. `apply_transaction` requires a revision. `update_text`, `undo`, `redo`, `add_picture`, `add_diagram`, `measure_layout`, `import_pptx`, `open_project`, `export_project` and legacy `export_pptx` use the shared document/session behavior.

`object_catalog`, `design_defaults` and `design_presets` are read-only MCP tools; `design_presets` wraps its array in `{presets}` for MCP structured content. `add_object`, `update_design`, `apply_design_preset`, `apply_theme`, and `assign_layout` require `deck_id` and `expected_revision`, plus their operation-specific inputs. `add_object` also requires `slide_id`. `update_text` accepts top-level text boxes and preset-shape text. All these mutations are undoable and reject stale revisions.

Only `--output-dir` enables MCP file output. Filenames are restricted to plain `.pptx` names; no generic path or shell operation exists. Model tools declare external-world behavior because operator-approved remote inference is possible. Imported relationships remain inaccessible regardless of model permissions.