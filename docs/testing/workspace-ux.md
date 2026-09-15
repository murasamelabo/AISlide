# Workspace Usability Upgrade

Date: 2026-09-15. The right-click, file/slide-operation and icon-import extension is implemented and locally verified. After confirming that they changed the repository to **Public**, the owner explicitly authorized a public source push. The agent does not change visibility, billing or the project license. Local input materials and generated deliverables are excluded from publication.

## User Operations

- Right-click canvas, slide thumbnails, layers, objects and graph nodes/connections for context-specific commands. Shift+F10, the context-menu key, arrows, Home/End and Escape operate the menus; dismissing restores focus. Coordinates are clamped to the viewport.
- New presentation creates one blank slide. New slide, duplicate, remove, move and rename work on authored and supported reopened PPTX files. The last slide cannot be removed. Graph/part metadata and source bindings participate in the same Undo transaction.
- Save As accepts a filename; Save preserves the opened source by publishing a new file. Browser output starts a download; native output uses the OS save dialog and verifies published bytes. New/Open/desktop close protect unsaved changes with Save / Discard / Cancel. Apply or cancel pending inline/property drafts before saving.
- Shortcuts: Ctrl/Cmd+N new presentation, O open, S save, Shift+S save as, M new slide, D duplicate, Z undo, Shift+Z or Y redo. Native text-input shortcuts remain local to the field.
- Insert icons provides 36 searchable Lucide icons, color, stroke and size. Import SVG/PNG/JPEG files, drag them onto the canvas or paste supported clipboard data. Figma Copy as SVG can also go into the SVG text field. No account credentials or `.fig` parsing are involved.

The existing DADS-inspired typography, action colors, focus and target sizes remain. Icons from design tools become transparent PNG pictures; vector-path editing and original SVG preservation are not claimed.

## Safety And Persistence

Document behavior stays in `aislide-core`: `create_presentation`, `edit_slides`, `edit_elements`, `create_asset`. SDK and MCP share revision/hash checks, bounded history and cancellation-before-commit. Frontend asset batches validate every input before one insertion transaction; a failed member inserts nothing.

Native slide structure editing preserves retained native XML. Copied charts and workbooks are independent; image/theme resources may be shared. Deleted slide/notes parts and relationships are removed. A remaining reference to a deleted slide/notes blocks deletion. Sections/custom slide shows block structural editing. Shared/opaque resources and retained source evidence can remain in the package, so deletion is not a privacy-redaction operation. External relationships are preserved but never fetched or executed by AISlide.

SVG uses `resvg` 0.48.1 with default features disabled after strict XML validation. Allowed shapes/paths/gradients/clips/masks are bounded by 256 KiB input, 2,048 elements, XML depth 32, reference depth 48 and expanded reference cost 4,096. Typed fragment targets, cycles and excessive fan-out are checked before parsing/rendering. Scripts, external references, styles, embedded images/text, filters and other unsupported effects fail closed. Output is transparent PNG at a 1024px longest side, at most 1 MiB, then rechecked by the existing raster decoder. This is not an OS-level sandbox.

## Observed Checks

| Surface | Observed evidence |
| --- | --- |
| Native slide structure | Unit/integration checks for blank creation, insertion, duplication, ordering, deletion, rename, stable IDs, opaque XML retention and byte-identical Undo |
| Copied chart independence | RED shared original chart/workbook; GREEN distinct copied native chart and embedded XLSX with preserved payload |
| Slide deletion | RED leftover slide/notes; GREEN removed parts and valid reopen |
| SVG | Actual transparent pixel checks; active/external input rejection; cyclic/missing/wrong-type/excessive reference tests |
| SDK/MCP | Real CLI create, icon import, element/slide commands, export/reopen, stale-revision rejection and Undo |
| Browser UX | Eleven focused tests: file lifecycle, unapplied drafts, clicked-object menus, icons/SVG, graph menus, atomic drop, shortcuts, 1440px/1200px/390px layout and scoped axe |
| Existing browser regression | Final complete run: all 50 passed, including 108 preset previews |
| Native file workflow | Real OS save dialog: requested name, cancel retaining dirty state, new-file save, single-PPTX reopen with 3 slides and 4 pictures; native close/cancel retains the window |

Final distinct automated test count: **214 passed**, without counting repeated runs twice.

| Gate | Passed |
| --- | --- |
| Rust workspace | 128 |
| Node bridge, SDK, MCP, extraction and generation | 27 |
| Studio browser | 50 |
| Generation browser | 4 |
| Native unit | 4 |
| Native WebView | 1 |

Frontend and embedded Tauri builds passed. The main frontend JavaScript chunk is 310.29 kB uncompressed after deferring icon and graph editors. Build, lint, encoding and source-publication checks are separate from Office compatibility and test coverage percentages. After normalizing one source BOM, all 19 single-file integration tests compiled and passed again.

The initial browser regressions were a too-small document-name button, a changed initial sample filename, and an existing reopen test that needed the new explicit discard confirmation. The filename and button were fixed; the confirmation test was updated without weakening source-provenance checks. Native testing additionally exposed a 701-1299px rule hiding the document name and dirty indicator; removing that rule and adding the 1200px check fixed it. Invalid simulated right-click coordinates and a lazy-loading wait were also corrected in the new test fixtures.

The native test owns its spawned process, disposable WebView profile and temporary output directory. It sends `WM_CLOSE` only to that process; no extra Tauri close permission is added. On this Windows installation, UI Automation omitted filename/save controls that Win32 exposed. The test uses the measured dialog class and child IDs, exact process ownership and bounded window messages to verify cancel/save/reopen. Drop and paste use injected WebView DOM events, not a claim of physical drag or OS clipboard automation.

SVG reference expansion and local unapplied drafts were identified during excerpt-level security/React review and reproduced as RED tests before correction. Review agents did not have direct filesystem access, so these reviews are not independent full-source audits. No measured coverage percentage is claimed.

CodeScene was not configured in this environment. No structural health score or CodeScene pass is claimed; executable tests, lint and scoped reviews provide the recorded checks. Automated axe checks are not a complete WCAG audit.

## Office Qualification

Run [the synthetic MCP demo](../../tools/workspace-demo.mjs) to reproduce creation, native reopen/edit, copied-chart independence, graph metadata and source-preservation assertions. The observed output contains 5 slides, 2 charts, 5 PNG pictures, 1 group and 2 connectors.

- Source PPTX SHA-256: `b08d2b4b669688139e2ecb4e75306ef36bb9c1551d1ccc830e9d1318870151e8`.
- Edited PPTX SHA-256: `20ddbb88a3402215e19b0e80a7fe97d861953e1f9140f8b1df440cd55e2b28d4` (135,232 bytes).
- Official Open XML schema validation passed.
- PowerPoint opened a disposable copy and rendered all 5 slides. Both embedded workbooks accepted temporary edits; both connectors retained attachments and followed movement of their begin-connected shape.
- All five slide captures were visually inspected, including four Lucide icons, a gradient SVG raster and independently edited/original chart values. The original presentation hash remained unchanged.

This qualifies the generated example only. It is neither an end-target connector movement test nor a general Office visual-parity guarantee. Generated PPTX files and Office captures remain local; the generator and recorded evidence are the source publication artifacts.

## Verification Commands

`node tools/cargo.mjs test --workspace`, `npm run core:build`, `npm run build`, `npm run lint`, `npm run test:e2e`, `npm run test:generation:e2e`, `npm run test:bridge`, `npm run test:sdk`, `npm run test:mcp`, `npm run test:extraction`, `npm run test:generation`, `npm run test:native -- --locked`, `npm run tauri:build`, `npm run test:native:e2e`, `npm run encoding:check`.

Native rebuilding can conflict with running executables or `WebView2Loader.dll` on Windows GNU builds. Existing user windows were preserved by renaming the loaded generated files before rebuilding; no user window was terminated. Vite continues to exclude Rust target outputs from its watcher.

## Asset Notices

Lucide icons retain their ISC license. The picker uses the existing `lucide-react` dependency and does not copy Figma libraries or third-party slide engines. Imported design assets retain their own terms; supplying them is not a transfer of rights. DADS attribution and font notices remain in the [design reference](graphs-and-dads.md#design-reference).

Locked Rust dependency metadata reports `resvg`/`usvg` 0.48.1 as Apache-2.0 OR MIT and `tiny-skia`/`tiny-skia-path` 0.12.0 as BSD-3-Clause. Their terms do not select the AISlide project license.