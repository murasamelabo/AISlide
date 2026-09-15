# Parts Library

Verified locally on 2026-09-14, Windows 11 ARM64, Microsoft Edge and Microsoft 365 PowerPoint. The implementation is shared by Rust core, Studio, SDK and the official-SDK MCP server. This is a bounded research PoC, not a full PowerPoint replacement.

## Use

1. Open **Parts library** from the ribbon and choose a category and layout.
2. Enter title, labels, values and other fields. Use **JSON** for full structured input; **Preview JSON** validates the current text. Invalid input remains editable and cannot replace the document.
3. Select **Insert part**. The result is an ordinary native group at `(64,144)` with size `1152x512` in the `1280x720` slide. Existing objects are not rearranged.
4. Select the group and use **Edit part data** to change metadata or layout. Root position and size are retained; Undo/Redo includes the data and native objects together.
5. **Save PPTX** creates one new standard PPTX. **Open PPTX** restores current native content and optional internal metadata. No external JSON file is needed for editing or redistribution of the presentation.

Category browsing retains its draft within the dialog. Changing layouts preserves authored data; untouched examples switch to the selected layout's example. Clicking the current JSON tab does not reset unfinished input. Array additions duplicate the first entry as an editable seed, so unique tree IDs, series lengths and optional values still require valid input. JSON can add optional fields not present in the example. Closing the dialog is not autosave.

The examples are synthetic and deterministic. They do not represent factual data or AI-generated artwork. Layouts use theme colors/fonts, restrained outlines, fixed content bands and measured text; changing the document theme affects theme-linked parts.

## Catalog

Every category has `balanced`, `focus` and `labeled` preset IDs with distinct layouts. The exact names and valid input examples are returned by `part_catalog`.

| Set | Categories |
| --- | --- |
| Charts and quantitative graphics | Pie/doughnut; columns; stacked columns; 100% stacked columns; horizontal bars; area; waterfall; line; pictogram; scatter |
| Structure and process | Tree; pyramid; horizontal flow; vertical flow; cycle; before/after; geographic map; puzzle/honeycomb; radial |
| Relationships and comparison | Relationship network; matrix; Venn; formula; small groups; many groups; item comparison; scale comparison |
| Strategy and lists | TAM/SAM/SOM; layers; triangle; steps; Gantt; vertical list; horizontal list; enumeration; ranking |

The 36 user-supplied [Slideland graph](https://www.slideland.tech/docs/graph/pie-chart) and [schematic](https://www.slideland.tech/docs/schematic/tree) category pages informed category vocabulary only. No slide artwork, template, CSS, source code or PPTX engine was copied. `cross` denotes formulas and `grow` denotes TAM/SAM/SOM analysis, not generic cross/growth pictures.

## Data Contract

`PartSpec = {version:1, preset, title, subtitle?, data}`. The schema is generated from Rust types and strict runtime validation is authoritative. Limits apply before geometry generation; subtype/preset rules can be tighter than this table.

| Data kind | Bounds and rules |
| --- | --- |
| `chart` | 1-12 categories, 1-4 series, rectangular finite values; numeric X for scatter; pie/area one series; percentage stacks nonnegative with positive category totals |
| `items` | 2-12 labeled items, optional detail/value and center; renderer-specific counts, including exactly three triangle/TAM-SAM-SOM entries |
| `tree` | 2-12 unique nodes, exactly one root, existing parents, no cycles, depth at most four |
| `network` | 2-6 nodes, 1-12 valid non-self indexed edges |
| `matrix` | 2-4 rows and columns with matching rectangular cells |
| `groups` | 2-6 groups with 1-5 items each |
| `timeline` | 2-12 periods, 1-8 tasks; zero-based start inclusive/end exclusive; progress 0-1 |
| `waterfall` | 2-10 steps; explicit totals reconcile with accumulated changes within floating-point tolerance |
| `map` | 1-8 points; longitude -180..180, latitude -85..85, optional nonnegative marker value |

All numeric inputs are finite and within +/-1e15. Text has bounded lengths; title/subtitle are at most 80/120 characters. Font measurement reduces text to a minimum of 12px, then rejects content that still cannot fit or lacks glyphs. There is no font download. Document limits remain 32 slides, 256 nodes per slide, 2048 total, 128 part instances and 2 MiB revisioned content.

Native charts retain raw values in embedded XLSX. Percentage-stack plots and focal metrics display shares without replacing the inputs. Waterfalls use editable rectangles, connectors and labels rather than a native waterfall chart. Tied ranks use competition ranking (1,1,3). Formula layouts use bounded addition/multiplication, never expression evaluation. Pictogram dots/tiles use 5% units and identify partial units while retaining the exact value label.

## Persistence And Safety

Part metadata is structured Custom XML under `urn:aislide:provenance:1`; it contains the specification and fingerprints, not a cached slide/deck. Current native XML remains authoritative. Without metadata, the PowerPoint-native objects remain editable, but semantic part editing is unavailable.

Fingerprints compare native group XML, chart/image payloads and embedded chart workbooks against the original package. Unknown group extensions, workbook changes, direct child edits and missing native fingerprints make an existing part stale. Semantic update then fails instead of overwriting those changes. Benign external reserialization can also conservatively mark stale. Hashes are unkeyed integrity hints, not authentication or factual verification.

Studio root movement/resizing preserves part metadata. Deleting a part removes its metadata in the same transaction, and Undo restores both. Existing external missing-root metadata is retained as stale, not silently used to recreate content. Reopened simple polygons support geometry edits; unsupported point/style replacement fails closed. Unknown imported objects/resources are preserved and never executed or fetched.

## Verification

| Gate | Result |
| --- | --- |
| `node tools/cargo.mjs test --workspace` | 110 passed, including 11 parts and 13 standalone-PPTX tests |
| Combined Node bridge/publication/SDK/MCP/extraction/generation tests | 23 passed |
| `npm run test:e2e` | 30 passed, including all 108 previews, 1440px/390px fit and automated axe checks |
| `npm run test:generation:e2e` | 4 passed against a synthetic loopback fixture |
| `npm run test:native -- --locked` | 3 passed |
| `npm run test:native:e2e` | 1 passed in an owned WebView/profile; existing user windows preserved |
| `npm run build`, `npm run lint`, `npm run tauri:build` | Passed |

Total at this historical checkpoint: 171 automated tests. The current aggregate and publication scope are in [workspace verification](workspace-ux.md). The last Venn geometry-only change was followed by the full Rust suite and all six parts browser cases, including the 108-preview pass. The other successful integration/UI gates precede that geometry-only change. Coverage percentage was not measured. Automated axe results are not a complete WCAG audit. Screenshot checks wait for fonts, inspect text overflow and nonblank pixels; no stored visual-diff baseline or universal Office parity is claimed.

Regressions were reproduced before their fixes: source paragraph/table formatting loss on replacement, JSON draft reset, missing native fingerprints, stale metadata after root deletion, percentage focal values, tied ranks, Venn label/outline intersection and immediate native cancellation. Float geometry is preserved across Rust/Node JSON using `serde_json`'s `float_roundtrip` feature, without weakening content hashes. Preview work is serialized and obsolete results discarded; aborting a fetch does not prematurely release the backend gate.

Native cancellation before request registration is retained once for the exact window/operation ID, bounded to 64 entries and 30 seconds. Wrong IDs against an active request return false, and the global busy guard remains. It is not a multi-window work queue. The actual immediate-cancel WebView test passed after the race was reproduced twice.

The [synthetic sample generator](../../tools/parts-demo.mjs) writes evidence with exact PPTX hashes, native object counts and 108 preset IDs. Each of four decks has 27 slides; all observed samples reopened with current metadata and byte-identical no-op save. All four passed official Open XML schema validation and PowerPoint opening/rendering; all 24 embedded chart workbooks accepted temporary edits. The first, second and fourth decks are byte-identical to the already verified samples; the changed third deck was revalidated and all its slides rendered again. An additional reopened-and-updated column part deck passed schema/Office checks and showed the changed title/value. Every source hash remained unchanged by verification. Generated decks, captures and detailed runtime evidence remain local and are not included in source publication.

The overview JPEGs are derived from actual Studio previews, not PowerPoint renders. The updated desktop executable was launched without remote debugging in a separate WebView profile; PID 16856, nonzero window handle and responding status were verified. The existing user Studio PID 35172 was left running. The development URL `http://127.0.0.1:4173/` returned HTTP 200 at handoff. These process IDs are observations, not durable launch identifiers.

Review-agent feedback was excerpt-based because those sessions lacked filesystem tools. Confirmed findings were checked and repaired with local regressions; this is not an independent full-source or security audit. Hosted GitHub Actions remains unverified due to the previously reported account billing/spending restriction. No commit, push, publication, project-license selection or protection-policy change was performed for this follow-up.

## Limits

- Area presets currently accept one series. Native chart editing uses the supported chart subset rather than all Office chart options.
- Venn intersections and relationship diagrams are nominal; they do not solve quantitative areas or infer causality.
- Maps use generalized coastlines, not administrative boundaries, geocoding or political assertions. Close location labels may overlap and require editing.
- Not every allowed count/label combination has been exhaustively tested; the 108 defaults and selected boundaries have. Rejected dense input needs shortening or fewer items.
- A part does not automatically fit around existing slide objects. SmartArt conversion, animation and full mixed-run text editing remain outside scope.
- Current source content and metadata are private document content. Internal storage is not a privacy filter or sanitization step.

## Asset Provenance

[world-land.geojson](../../crates/aislide-core/src/parts/world-land.geojson) contains Natural Earth 1:110m land polygons downloaded on 2026-09-14 local time from [natural-earth-vector](https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_110m_land.geojson).

SHA-256: `9e0729ee253ca7d7a5c4ae9395fb1902264c5377c52e224d13dd85010e2835d9`.

[Natural Earth terms](https://www.naturalearthdata.com/about/terms-of-use/) place its raster and vector data in the public domain and allow modification and redistribution. The source GeoJSON bytes are retained; the renderer reads outer rings, projects them equirectangularly, and normalizes native polygon coordinates to millionths. No runtime network fetch is used. Country borders and interior holes are not represented by this simple land-outline renderer. The project itself still has no selected redistribution license.

## Self-Assessment

| Axis | Score | Evidence and remaining improvement |
| --- | --- | --- |
| Accuracy | 4/5 | Core/browser/native/Office evidence covers current samples; arbitrary Office typography and input combinations are not proven. |
| Completeness | 4/5 | All 36 categories have three presets, shared APIs and single-file persistence; dense map labels still need manual adjustment. |
| Clarity | 4/5 | Usage, provenance and failure boundaries are documented; generic structured-field labels remain less approachable than domain-specific editors. |
| Actionability | 4/5 | Running desktop/browser and four standalone sample decks are available; committing/pushing still requires authorization. |
| Conciseness | 4/5 | Sample guide is separate from API/test details; repeated safety explanations across documents can be reduced in future documentation maintenance. |

Overall 4.0/5. Highest-value follow-ups are category-aware data-entry controls, map-label collision handling, and broader theme/font qualification. User agreement with this assessment is not assumed; the sample decks and explicit limits make it reviewable.