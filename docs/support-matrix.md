# PoC Support Matrix

The independently implemented exporter follows public OPC/OOXML specifications using generic XML/ZIP libraries. This matrix describes actual support, not universal compatibility.

**The current normal workflow is one standard PPTX.** The historical import table below describes the legacy `import_pptx` path. `open_presentation` additionally reads masters/layouts/themes, all ten native chart kinds, preset shapes, simple polygons and text formatting directly from XML, and supports bounded differential editing. Reopened structure counts/order and unrepresentable complex styles remain protected. See [single-file operation](testing/single-file.md) for the current UI and preservation contract.

| Surface | Create/export | Import preview | Non-destructive import edit |
| --- | --- | --- | --- |
| Text | Native text/shape text; multiline, fonts, bold/italic/underline, alignment, bullets/numbering and inert authored hyperlinks | Direct geometry and text; mixed formatting approximated | Top-level single plain run per paragraph; same paragraph count; geometry |
| Rectangles | Native shapes and fill | Direct rectangles | Geometry only |
| Preset shapes | 40 native presets, editable text, fill/outline, rotation | Not a full preset/style importer | Unsupported edits rejected; original parts retained |
| Tables | Native editable cells; bounded rectangular data | Text and first-run size; theme/style approximation | Geometry only; original cells/styles preserved |
| Charts | Native column/bar/line/pie/doughnut/area/scatter/stacked column/stacked bar/100% stacked column plus embedded editable XLSX | Original bar/column/line cache subset; other kinds may be omitted or approximated | Geometry only; original chart/workbook preserved |
| Pictures | PNG/JPEG, alt text, crop and geometry | Bounded embedded PNG/JPEG | Geometry only; original image preserved |
| Groups | Native nested groups, maximum depth 8 | Zero-origin supported transforms | Read-only; preserved |
| Connectors | Native lines/arrows with sibling shape references | Supported line geometry and references | Read-only; preserved |
| Masters/themes | 1-8 native masters, 1-32 layouts, common text/shapes/pictures, placeholders; twelve theme color slots and heading/body/script fonts | Inherited master content may be absent | Imported design parts preserved, not editable |
| SmartArt, OLE, audio/video, effects, unknown extensions | Not authored | Preserved but not executed; may be absent from preview | No editing or activation |
| Slide ordering | New scenes support reorder/duplicate | Original order resolved through root slide list | Original package slide structure is read-only |
| Notes | Native speaker notes and source attribution | Best-effort plain text | Original notes preserved |

All import warnings must remain visible. Preview success can mean zero supported visual objects; it is not a visual-fidelity score. Source packages are retained and no-op saves return their exact bytes. Supported patches preserve every non-target part payload. Unsupported edits fail rather than silently regenerating a lossy copy.

Authored slide placeholders follow layout geometry and whole-frame formatting. Manual geometry/style edits detach inheritance; text-only edits preserve it. Layout reset reapplies template formatting without replacing matching text. Switching layouts or deleting a template preserves obsolete placeholder content as independent text. Assigning a layout does not reflow unrelated freeform objects, which can overlap and need manual adjustment. Footer/date/slide-number placeholder markers are not automatic date/number field generation.

Studio previews preset outlines and text regions approximately; shape-specific OOXML adjustment handles, shape text insets, complete theme effects, and mixed-run text are not implemented. PowerPoint retains the actual preset geometry. Pie/doughnut require one non-negative series with a positive total; scatter categories are finite numeric X coordinates. Chart factories clearly label their values synthetic. See [authoring verification](testing/authoring.md).

## Workspace Operations

Studio starts with one clean empty slide. The synthetic twelve-slide report is available only through the explicit **New report** command. **Browse presets** in the master editor offers seven independently authored native master/7-layout sets, with color/font roles, side margins, gutters and content/part regions. Application preserves original masters and slide content, rejects edited/foreign preset templates or capacity conflicts, and participates in Undo. Reopened documents need an existing compatible preset structure. See [preset behavior and limits](testing/master-presets.md).

The current single-file workflow supports blank new presentations and insert/duplicate/delete/reorder/rename commands on authored and supported reopened slides, shared by Studio, SDK and MCP. Native master/layout counts remain fixed. Slide duplication preserves native XML and gives copied charts independent embedded workbooks. Structural editing rejects sections/custom slide shows; deleting a slide referenced elsewhere is rejected. Source files are never overwritten. Deleted slide/notes parts are removed, but shared media, opaque data and source documents can remain; this is not a redaction feature.

Studio offers right-click menus for canvas, slide thumbnails, layers, objects and graph nodes/connections; keyboard invocation is Shift+F10 or the context-menu key. Relevant commands remain on buttons/forms for non-pointer use. New/Open and desktop close protect unsaved changes. Unapplied text/property drafts must be applied or cancelled before saving. Save and Save As publish a new file; browser completion means a download was initiated, not that its final destination was independently verified. Autosave and overwriting the opened source remain unsupported.

**Insert icons** provides every canonical icon in Lucide React 1.43.0: 1,818 icons, without duplicate aliases, with a filter for its 42 official categories. Category and keyword search intersect; both reset the page. Search covers the full catalog by name and official tags, including PascalCase and hyphenated names; the existing English/Japanese tags are retained for the featured entries. Only 60 choices are rendered per page. Color, stroke and size controls remain available. File selection, canvas drop and paste accept PNG/JPEG or the bounded inert SVG subset. Figma **Copy as SVG** is supported as SVG text, not as arbitrary Figma clipboard data or a `.fig` document. SVG paths are rendered locally to a transparent PNG at a 1024px longest side; output is movable/resizable/croppable, not a vector-path editor. Imported text, effects, embedded images and external references are rejected; export outlined paths or PNG from the design tool when needed. There is no Figma account API or access-token requirement. The source asset's license remains the user's responsibility. See [workspace verification](testing/workspace-ux.md).

## Parts Library

The current single-file path supports 108 presets in 36 categories. Shared core, SDK, MCP and Studio create ordinary editable groups, not slide-sized raster images or SmartArt. Charts retain native editable workbooks; geographic land outlines use simple native polygons. Metadata travels inside the PPTX and is optional for PowerPoint editing. Insert/update/delete and their metadata participate in the same revisioned Undo/Redo.

Horizontal/vertical flow, tree and cycle presets now use filled process panels, safe-inset arrows, subtree-aware blocks and segmented cycles. The 108 existing preset IDs remain; display names describe the revised drawings. Relationship labels use a bounded collision-avoiding placement search. All default presets and 108 inserted Japanese examples have been checked; excessive density can still be rejected rather than overlapped. See [the refresh evidence](testing/parts-library.md#2026-09-16-refresh).

Parts accept bounded typed data and use installed-font fitting with a 12px minimum. All 108 catalog defaults have generated, reopened and rendered successfully; this does not prove every permitted input combination or universal Office parity. Percentage stacks retain raw numbers and show shares; waterfalls require reconciled totals; ties use competition ranking. The area part currently accepts one series. Pictograms use 5% units; Venn/relationship diagrams are nominal, not computed area/causal models. General map-label collision avoidance, country borders and geocoding are not provided.

The preset Text and visual layout additionally fits newly inserted parts/graphs into its right-hand native region, normalizing child coordinates, text and strokes together. Updates retain the fitted canvas and placement. This explicit reduction can reach the model's 8px font floor; dense labels may require a wider layout or fewer items. Other objects and existing content are not automatically reflowed.

Manual changes that disagree with stored metadata mark it stale and prevent semantic replacement, while preserving native content. Unsupported polygon point/style replacement after reopening fails closed. Insertion does not reflow existing objects. Detailed bounds, examples, provenance and verification are in the [parts guide](testing/parts-library.md).

## Guided Authoring

Core, SDK and MCP provide four English best-practice profiles and deterministic creation from a structured evidence-linked outline: consulting decisions, technical explanation, event presentation and reports. Guided creation makes a new document, not a replacement for an imported deck. It uses existing parts and a native headline/evidence layout; consulting multi-page decks additionally have dedicated C02 summary and C03 closing grids with the same 3-6 issue IDs, analysis references, owners, timing and criteria.

The 48 consulting patterns are retained as selection guidance with explicit capabilities. Only C02/C03 and `native-part` are automatic layouts; other compound patterns require composition or manual design. Guides are available through `best_practice_profiles` and `best_practice_guide`; `validate_guided_presentation` and `create_guided_presentation` enforce bounded inputs, declared numeric evidence, full-headline support references, basic headline rules and measured text fit. They do not invent values or contact a model. Unknown values remain `xx` in qualitative bodies rather than numeric charts.

`ready` is a compilation result, not proof that sources are true, a recommendation is justified, all prose quantities are sourced, all graphical conventions hold, or Office rendering is identical. Human-review requirements remain explicit. Notes retain the supplied ledger and evidence and need privacy review before redistribution. Later ordinary edits do not maintain a live proof of guideline conformance. See [the full input contract and limitations](authoring/README.md).

## Architecture Graphs

Studio, SDK and MCP support six native shapes (rectangle, rounded rectangle, ellipse, diamond, cylinder, cloud), attached straight/right-angle connectors, start/end arrows, dashed lines, labels, theme colors, boundary groups, movement/resizing, alignment and grid. React Flow supplies canvas interaction; document behavior remains in Rust. One PPTX retains optional metadata and supports native reopen, update and Undo/Redo.

Nodes also support an optional icon beside the label. Select a node and use **Choose icon**, **Change icon** or the remove-icon control; the same commands are available from its context menu. The shared full Lucide catalog and SVG/PNG/JPEG file input are reused. Cancel retains the current node form. Accepting an icon applies the current form and icon together as one graph-local Undo/Redo edit, followed by one document transaction when applying the graph. Graph history is capped at 30 entries and 4 MiB per direction.

Graph-specific imports prepare images at a maximum 256px longest side; SVG becomes PNG and smaller PNG/JPEG bytes are preserved. The icon fits its node at up to 48 graph pixels without changing the connector target. Image colors are fixed, not theme-linked. No Figma account access, standalone icon-shaped nodes, SVG path editing or fetching external assets is provided. Raw API `GraphIcon` values must contain validated PNG/JPEG data and obey existing image/document limits. Dense labels may need a wider node.

Limits: 1-48 nodes, 64 edges, 8 boundaries; 1152x512 below an 88px title band; the rendered slide remains limited to 256 elements. Shapes have separate label text and optional pictures within the root group. PowerPoint shape movement keeps connectors attached but does not move its separate label or icon automatically. Graph-editor movement moves all three and recomputes connections together.

No draw.io XML interchange, self-loops, nested boundaries, arbitrary/curved paths, automatic obstacle/label collision avoidance or general Office parity is claimed. Grid with boundaries requires all nodes to have membership. Insertions do not reflow other slide content. Unsupported native connector paths fail closed; external edits preserve native content and make mismatching metadata stale. See the [graph/UI report](testing/graphs-and-dads.md).

Application chrome is DADS-inspired, not a certified or fully conformant DADS implementation. Fonts, actions, focus, spacing and controls are adapted; slide themes remain unchanged. Automated accessibility checks are not a complete WCAG audit.

## Evidence Intake

| Format | Supported behavior | Limitations |
| --- | --- | --- |
| CSV | Header/record parsing, original strings and record/column locators | UTF-8; no delimiter guessing or numeric imputation |
| JSON | Scalar record arrays or header/row matrices, JSON pointers | Nested values rejected; missing fields remain null |
| XLSX | Bounded sheets, cached values, sheet/cell locators | No formula evaluation, external relationships or macros; dates require review |
| Text/Markdown | Plain text evidence | Never rendered as executable HTML |
| PDF | Bounded text extraction with page locators | No page rendering or table inference; scanned pages need separate image input |
| PNG/JPEG | Decode validation and raster metadata; optional Windows OCR | Installed OCR language required; words/regions remain unverified |

Intake is capped at 2 MiB input; rasters at 1 MiB, 4096 pixels per dimension and a decoder allocation budget. Source tables allow at most 1000 rows, 32 columns, 8 sheets and 16000 total data cells. Data-report mapping selects 1-32 rows and 1-6 numeric series explicitly. XLSX cells/dimensions are preflighted before allocating ranges. PDF loading and extraction use per-stream limits; this is not an OS-level process sandbox.

## Safety And Portability

- JSON request/response: 4 MiB; revisioned content: 2 MiB. Import document capacity is lower than the raw PPTX envelope because the original archive and preview are retained together.
- Scenes: 1-32 slides at 1280x720; 256 nodes per slide, 2048 total. Simple inspection/roundtrip supports up to 256 slides. Group coordinate transforms preserve child units.
- New project export checks geometry, source bindings, measured text overflow and missing glyphs. Font fallback is reported. Neither chart-label fitting nor Office-native text parity is proven by these checks.
- Raw low-level `export` and deterministic CLI `generate` are structural tools; they do not imply the project-level measured-layout gate.
- Publication uses exclusive file creation; no original/source file is overwritten. Two-file publication is not crash-atomic. Existing destinations are rejected before publication; racing collisions are reported as partial publication without deleting public paths.
- Hashes, source locators and inverse receipts provide integrity and concurrency checks, not authentication. Checkpoints retain local source content and must be protected accordingly.
- Model HTTP and in-flight CLI requests support cancellation. Native blocking OCR/PDF work has bounded input but is not a hard-cancellable, isolated worker process.
- Current native qualification is Windows 11 ARM64 running an x64 GNU Tauri build, with a native ARM64 CPU model server. Native ARM64/MSVC application builds, macOS/Linux distribution and signed installers remain unqualified.

ZIP64, encrypted/legacy presentations, non-UTF-8 edited XML and signed-package edits are rejected. Source OPC relationships are never fetched and imported content is never executed. PowerPoint checks use disposable copies and are optional, never a runtime dependency.