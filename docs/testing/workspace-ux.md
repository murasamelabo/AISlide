# Workspace Usability Upgrade

Date: 2026-09-15. The right-click, file/slide-operation and icon-import extension is implemented and locally verified. After confirming that they changed the repository to **Public**, the owner explicitly authorized a public source push. The agent does not change visibility, billing or the project license. Local input materials and generated deliverables are excluded from publication.

## Resizable Workspace Panels

**Current final qualification (2026-09-20):** the [cloud/workspace candidate](graphs-and-dads.md#2026-09-20-cloud-and-workspace-qualification) passed one 80-case, five-file Edge run with zero failures, skips or retries, superseding the older combined 48-case results retained below as history. Build/typecheck and lint passed with zero errors and five pre-existing warnings. One final development-native run passed all three cases (Fit/wheel/pane resizing, existing authoring with a local synthetic model fixture, cloud icons/nesting), with unchanged user settings and original timeouts after one test-only dialog-readiness fix. This is not fresh process-restart recovery or Office coverage. Evidence: `.artifacts/cloud-final-browser-20260920-8b615d/handoff.md` and `.artifacts/cloud-desktop-final-20260920-a91f7c/handoff.md`. Qualified candidate; publication and regular installation recorded separately. The NSIS candidate is built, not yet normally installed or publicly committed.

On 2026-09-19, a follow-up regression measured only **175.984px** of slide height at 1200 x 768 with an object selected and Inspector shown. Compact application chrome now gives **373.984px**, with Inspector either shown or hidden in this height-limited example. The main ribbon and selection toolbar scroll horizontally instead of wrapping. Zoom, Inspector visibility and workspace reset remain next to the canvas. Button targets remain 44px; document dimensions, aspect ratio and slide typography are unchanged.

The slide list, Inspector and notes have focusable resize separators. Drag their grips, use the axis arrows (8px, Shift: 32px), Home/End for bounds, or Enter/double-click to reset that pane. Escape, pointer cancellation, capture loss and focus loss cancel the gesture. The reset icon restores all three dimensions. Notes show the complete text in a scrollable preview, not an editor. Errors use the same fixed-during-notification region without moving the canvas.

Defaults are 192px left, 288px right and 44px notes. Desktop bounds reserve a 320px canvas column; notes reserve 264px for the heading, canvas, footer and separator. Stored dimensions are additionally bounded to 128-360px, 224-480px and 44-360px respectively. Below 801px the side separators are hidden and the existing stacked Inspector/carousel layouts remain; notes can still be resized with touch. Window changes clamp the displayed sizes without overwriting the preferred sizes, including when a clamped drag is cancelled.

Only three finite numeric sizes are stored under `aislide.workspace-panels.v1` in localStorage. Oversized/malformed data or extra fields fall back to defaults. Writes happen on completed gestures, keyboard changes and reset, not pointer moves. Blocked storage falls back to in-memory behavior. No deck, selection, filename, recovery setting, document transaction or Undo entry is persisted by this feature.

The eight focused Edge cases in [authoring-qa.spec.ts](../../tests/e2e/authoring-qa.spec.ts) passed without retries: default visible size, all three real mouse drags, exact saved-PPTX bytes and zero core requests during view changes, manual zoom, reload, keyboard bounds/reset, cancellation/capture/blur, malformed/blocked storage, and touch at 390 x 844 and 800 x 600. A scoped separator axe check also passed; this is not full WCAG certification. The cancellation/default cases passed again after preserving pre-clamp preferences and preventing Inspector child collapse. Evidence: `.artifacts/resizable-workspace-red-59P2f6`, `.artifacts/resizable-workspace-interactions-Z8WSy8`, and `.artifacts/resizable-workspace-cancel-p5TSAB`.

Final coverage is **48 distinct passing browser cases** (eight new plus the prior 40). The first run passed 22 and stopped when the existing desktop dialog axe analysis exceeded its unchanged 30-second limit. That case passed unchanged alone, then all 25 unrun cases passed. This is combined coverage, not a clean single-run 48/48 result. Build/typecheck and lint passed; only five pre-existing warnings remain (three Fast Refresh, two unrelated BOM edits), along with the existing bundle-size warning. Logs, full command arguments, measured bounds, screenshots and preserved source/binary hashes are in `.artifacts/resizable-workspace-final-vKjud0` and `.artifacts/resizable-workspace-remaining-f4uMuG`. The failed-run artifacts are retained.

Reproduce with `node node_modules/@playwright/test/cli.js test tests/e2e/authoring-qa.spec.ts --grep "workspace panels" --retries=0`, using a fresh `AISLIDE_TEST_PORT`, `CI=1` and `AISLIDE_REAL_AI_TEST=1`. The initial UI implementation did not run native qualification; the subsequent bounded gate is recorded below. Regular installation and Office qualification remain separate. Existing user windows, preview servers, recovery/local-AI edits and setup documentation were not changed.

### Subsequent Native Qualification

The extended [native Fit scenario](../../tools/native-generation.test.mjs) first failed against the unchanged old development executable: the selected 16:9 slide was **177.650px** high at 1200 x 768. After frontend and offline single-job native builds, the same default measured **365.525px**, with Inspector both shown and hidden. Actual pointer drags resized all three panes; keyboard resizing, preference reload/reset, unrelated preference retention, unchanged authored object geometry and zero native document operations during resizing passed. Existing 16:9/4:3 Fit and wheel assertions remain. Final screenshots were inspected in `.artifacts/native-canvas-fit-bid8pT`.

Two distinct native scenarios passed: the extended Fit/pane case and the existing authoring workflow, including direct editing, graph/icon changes, Undo, protected-input rejection, native save/cancel/reopen and unsaved-close protection. The new test initially compared zoom-adjusted selection handles as document content; its comparison now excludes only those UI handles. A subsequent run completed the assertions but exceeded the unchanged 90-second limit; removing six redundant screenshots allowed the final run to pass in 62.8 seconds including runner overhead. All assertions, time limits and cleanup checks remain; disposable profile/recovery directories were removed. These are combined native results, not a clean two-case final rerun.

The subsequent browser attempt passed two cases, then the keyboard/reset case timed out at its unchanged 30-second limit. Its trace shows keyboard/layout assertions and axe `runPartial` completed, followed by a stalled `typeof window.axe?.runPartial` evaluation on axe's new aggregation page. The case passed unchanged alone, and the remaining 45 cases passed without retries: **48 distinct passing cases**, not a clean single-run 48/48. No browser tests, timeouts or assertions were edited. Build/typecheck and lint passed with the same five existing warnings. Evidence, failed traces, commands and hashes: `.artifacts/workspace-final-TSieyX`. The development executable is SHA-256 `59d1b33fbda7f50d5b49e78cb00cf0bde60a7beef76a17f6cd276ae65240e776`; the CLI remained `d76cb58038f9716d36a6ba805967a74185da65ef5787eb9c7a8f4d4f9dacaa00`. Read-only review of the scoped implementation found no concrete P1/P2 product defect; this was not an independent review. No core/CLI rebuild, full-suite run, installation, Office action, cloud-asset change or Git operation was performed.

## Canvas Fit And Wheel Zoom

On 2026-09-19, the inspector-hidden regression at 1200 x 768 reproduced a slide bottom at 886px and notes bottom at 1062px. Fit now uses both available viewport axes after application chrome, preserving the document's actual aspect ratio. The same test places the slide bottom at 539.984px and notes bottom at 728px. Desktop flex children can shrink; mobile retains a bounded workspace and ordinary page scrolling with the inspector below it. Notes and errors share a fixed-height scrollable region, so an error notification does not move a rejected drag's restored position.

Vertical wheel input over the canvas changes view-only zoom from 10% to 400%, anchored to the cursor where scroll bounds permit. Fit recenters and resumes automatic fitting; manual zoom survives inspector changes. The labeled keyboard-operable selector includes presets and the current wheel percentage. Editable controls, horizontal/Shift-wheel, active pointer gestures and off-canvas scrolling are excluded. Ctrl-wheel is intercepted only on the eligible canvas, not globally. New documents remain 1280 x 720; zoom does not resize documents or create history entries.

Internal zoom retains fractional precision; only its displayed percentage is rounded. A review regression at 10% with five pixel-mode wheel deltas of -10 first failed because each input rounded back to 10%, then passed after using the unrounded state. The focused wheel scenario also retains its cursor, limits, history and saved-byte checks. Evidence: `.artifacts/wheel-fine-red-ixynNJ` and `.artifacts/wheel-fine-green-WrtNJx`.

Final Edge browser verification: **40 passed, 0 failed**, covering the existing authoring QA and drag suite plus page dimensions/Undo. Six new tests cover 1440 x 900, 1200 x 768, 960 x 700 and 390 x 844; 16:9, 4:3, portrait and square pages; inspector toggles; resizing; square PPTX reimport; real wheel input and bounds; unchanged browser scale; selection/editing exclusions; no core requests during view changes; and byte-identical saved PPTX. Aspect fixtures use shapes because scaling the sample report can legitimately violate the core's 8px minimum font size. The original clipping and missing-wheel tests both failed before their respective fixes. A related drag rejection test exposed notification-driven viewport movement and passed after the stable message-region fix.

Initial evidence: `.artifacts/inspector-zoom-red-4WK0Rd`, `.artifacts/inspector-zoom-wheel-red-ZAp34M`, and logs/screenshots/traces in `.artifacts/inspector-zoom-final-IKqkRK`. Reproduce with `node node_modules/@playwright/test/cli.js test tests/e2e/authoring-qa.spec.ts tests/e2e/authoring.spec.ts tests/e2e/editing-workflows.spec.ts:154 --retries=0` using a fresh `AISLIDE_TEST_PORT`, `CI=1` and `AISLIDE_REAL_AI_TEST=1` to reuse the existing CLI without rebuilding it. That earlier CLI SHA-256 was `60d18a93130c8eebb3d47df84771748512ce9e29020ce9d47048f1ce7f295f4c`; its 40-case run predates the fractional-precision correction.

The final publication rerun passed all **40 cases**, including the fractional-input regression, with no failures, skips or retries. Build/typecheck and lint passed; three existing Fast Refresh warnings and two warnings from unrelated working-tree BOM edits remain outside this change. The preceding shared-core run passed 511 tests with six explicitly ignored cases and regenerated the CLI through its integration tests. The final browser and 41-entry histogram-validation gates used SHA-256 `d76cb58038f9716d36a6ba805967a74185da65ef5787eb9c7a8f4d4f9dacaa00`, unchanged throughout those gates. Evidence: `.artifacts/final-histogram-publish-JCRYSx/summary.json` and `.artifacts/histogram-publish-final-PBVDfE/summary.json`. Repeated cases are not counted as additional distinct tests.

The owned native WebView regression first reproduced the old build's slide bottom at 884px and notes bottom at 1060px in a measured 1200 x 768 viewport. After rebuilding, the new scenario and existing native authoring workflow both passed (2/2): 16:9 and 4:3 with Inspector shown/hidden, real wheel input, Fit restoration, unchanged page geometry/browser scale and no core mutation requests. Native measurements and screenshots are in `.artifacts/native-canvas-fit-AseEkf`. That run predates the final fractional-precision correction. After that correction, frontend/native builds and the same focused native Fit/wheel scenario passed again; `.artifacts/canvas-native-final-hZ6XaD/summary.json` records the development executable SHA-256 `c8609767ca9525718ce76eaf737794e779485e15cf4ca64620a372613820a51f` (1,015,062,016 bytes). This repeated scenario is not an additional distinct test. Four unrelated Rust files had duplicate leading BOMs blocking compilation; only those BOMs were normalized, with byte-for-byte body equality recorded in `.artifacts/canvas-build-encoding-RSfhMz/normalization.json`. This is scoped development-WebView qualification, not a full-suite, installed-app update, Office or release qualification. The user's installed application and unsaved work were left untouched.

## Canvas Drag Responsiveness

On 2026-09-16, moving and resizing existing canvas objects was improved without changing the document format, core validation, revision guards or Undo. This follow-up is local and uncommitted. The user journey is to drag an icon or picture, release it without a visible return to its previous position, and retain a single undoable edit. File import/conversion and graph-editor node dragging are not the optimized path.

Previously, pointer-up discarded the preview while the asynchronous core transaction was still running. Held-response browser tests reproduced a maximum **72.225px** return for both movement and resizing. Studio now returns the transaction promise to SlideSurface, which retains the preview until that promise settles. A rejected edit restores committed geometry and retains the existing error message. Commit processing can still take time; this change does not queue edits or bypass the busy guard.

Pointer updates are coalesced into one animation-frame update. Movement uses CSS translation instead of changing layout coordinates on every frame; unchanged image, chart, group and text content is memoized using immutable element/theme references. Resize still recalculates the selected object's size. Escape, pointer cancellation, capture loss, slide/document changes and a different pending edit clear uncommitted gestures. Drag-owned pending commits are retained. Fractional coordinates survive clicks within a 3px screen-space threshold, and pointer-up uses its final coordinates even before a queued frame runs.

### Measurements

The same synthetic slide contains 80 ordinary PNG pictures. Each of three drags uses 120 pointer moves over 72 by 36 screen pixels; images and fonts are decoded before sampling, and each drag is undone. Edge CDP `Performance.getMetrics` measures cumulative browser task work during movement, before the core commit. These are not end-to-end latency, FPS, GPU paint measurements or guarantees for every presentation.

| Variant | Task work, three runs (ms) | Median (ms) | Layouts per drag |
| --- | --- | --- | --- |
| Before rendering optimization | 976.806 / 925.762 / 881.491 | 925.762 | 107 / 107 / 107 |
| Animation frames, translation and memoized content | 720.323 / 670.457 / 661.509 | 670.457 | 0 / 0 / 0 |
| Final lifecycle guards, full-suite remeasurement | 716.691 / 738.833 / 693.246 | 716.691 | 0 / 0 / 0 |

The initial measured reduction was 27.6%; the final remeasurement was **22.6%** relative to the baseline median. Style recalculation was not uniformly faster. Held-transaction preview displacement was **0px** for both move and resize in the browser and in the installed desktop WebView. The image data remained unchanged and Undo restored the original geometry.

### Regression Evidence

The existing [authoring tests](../../tests/e2e/authoring.spec.ts) contain seven added scenarios: dense-picture measurements; pending move; pending resize; cancellation/slide changes; rejection/retry; fractional clicks/final pointer coordinates; and overlapping edits. The last case first reproduced a stale 54px preview when another edit removed the pointer control, then passed after explicit gesture cleanup. Its unrelated edit toggles master-graphic visibility so a newly inserted object cannot obscure the tested picture.

The [installed lifecycle test](../../tools/windows-setup.test.mjs) launches an isolated installation through its actual Start Menu shortcut. It temporarily holds only transaction transport in that test-owned WebView, checks four preview frames and busy state, restores/releases the transport in `finally`, waits for completion, and tests Undo. Tauri's `invoke` property is read-only; the first test interception did not take effect. The corrected test holds the exact IPC fetch endpoint without changing native requests or production code. Both native gestures measured 0.000px displacement. Screenshots were inspected.

Final verification covers **216 distinct passing tests**: Rust workspace 140, Studio browser 71 and setup contracts/installed lifecycle 5. The full Studio run passed 70; an existing graph accessibility/import scenario hit its 30-second limit and passed unchanged in 8.1 seconds on a single-case rerun. Frontend typecheck/build, embedded desktop/NSIS build, lint and scoped editor diagnostics passed. No coverage percentage, complete WCAG audit, new Office parity or independent full-source review is claimed. Review agents were excerpt-limited. Unchanged Node/generation and separate native-unit suites were not rerun for this frontend interaction change.

Commands: `node tools/cargo.mjs test --workspace --locked`, `npm run setup:build -- --debug --no-sign`, `npm run test:setup:installed`, `npm run lint`, and `npm run test:e2e` with `AISLIDE_TEST_PORT=4174`. For the focused measurement, use `npm run test:e2e -- tests/e2e/authoring.spec.ts --grep "dense picture drags"` with the same port environment. See the [updated setup artifact](windows-setup.md#canvas-interaction-refresh). No regular installed app or user document was modified by verification.

## Icon Library Expansion

On 2026-09-16, the user requested the complete current Lucide catalog, superseding an intermediate 96-icon selection. The official npm registry reported **1.43.0** as the latest stable `lucide-react`; the installed package and lockfile were updated from 1.41.0. Its public `icons` export contains **1,818 canonical icons**. The picker is derived from that export instead of a hand-maintained subset, without counting duplicate aliases as additional drawings.

The original 36 names and drawings remain, as do the 96 featured entries' English/Japanese search tags. The rest are searchable by official icon name, with spaces, PascalCase or hyphens. Search applies to the entire catalog and resets to page one. Only 60 buttons render at a time; previous/next buttons, visible range and total count support browsing all results. Seven basic icons whose names overlap existing short labels use a `(plain)` suffix. The same picker serves slides and graph nodes; color, stroke, size, import and Undo behavior still use the unchanged core pipeline. Icons are bundled with the application, not fetched from an external server at runtime. This does not enable future automatic npm updates or include the separate experimental Lucide Lab collection.

Tests first detected the missing pagination against the 96-entry implementation. They then caught the old development server's cached 1.41.0 catalog (1,807 icons instead of 1,818); a fresh loopback test server resolved this without closing user pages. `AISLIDE_TEST_PORT` now optionally selects a validated local port, while the existing 4173 default is unchanged. The complete-catalog test visits every page and compares all official IDs, checks rendered drawings and label bounds, retains every original display name, exercises English/Japanese and empty searches, and inserts Zodiac Virgo through keyboard selection, Undo/Redo and standalone PPTX reopen. The graph test inserts Orbit through the existing history and persistence flow. Straight-line SVG icons legitimately have zero width or height in their geometric bounding box, so the render check permits one zero dimension.

At 1440px, 1200px and 390px, page controls and scoped axe checks pass. The longest official name, SquareCenterlineDashedHorizontal, fits its button and can be selected and inserted; the insert button remains reachable by scrolling the dialog on small screens. Screenshots were inspected. One pre-existing menu axe check timed out during a concurrent build and passed unchanged when rerun on its own. These checks are not a full WCAG audit, comprehensive pixel-diff test, measured coverage percentage or new Office visual-parity claim.

Final checks for this expansion cover **199 distinct passing tests**: Rust workspace 134, Studio browser 60, and setup configuration/installed lifecycle 5. The full browser run passed 59 tests and exposed a test-only Origin header hard-coded to port 4173. Deriving it from the current page URL fixed the remaining test, which then passed alone; the application's same-origin policy was not changed. The installed lifecycle test launched its actual Start Menu link, confirmed the full catalog and 60-item page, searched and inserted Zodiac Virgo, and undid it before verifying the existing install/uninstall guards and cleanup. It did not change the user's installed app or open presentations.

The frontend, embedded desktop and unsigned NSIS setup builds passed, as did lint. The full icon catalog remains inside the lazy-loaded AssetPanel chunk: 785.26 kB minified, 216.32 kB gzip; main JavaScript is 365.87 kB. Vite reports its normal 500 kB chunk warning for the all-icon panel. The threshold was not raised or the warning suppressed; this is a bundled offline-catalog tradeoff, not a measured first-open performance guarantee. React, TypeScript and security reviews were excerpt-only and reported no concrete findings; a full independent source audit was unavailable. See the [refreshed setup artifact](windows-setup.md#lucide-catalog-refresh) for installation details.

## UI Consistency Follow-up

Later on 2026-09-15, toolbar screenshots prompted a UI-only correction. These follow-up changes are local and have not been committed or pushed; the earlier publication authorization/result above describes the preceding workspace checkpoint.

| Surface | Before | After |
| --- | --- | --- |
| Tooltips | Absolutely positioned text was clipped by the scrolling ribbon into a black strip; native `title` could appear as a second tooltip | Shared Radix Tooltip 1.2.16 portals into the body or owning native dialog, with viewport collision handling, hover/focus support and Escape dismissal |
| Tool dimensions | Icon sizes varied between 15-20px and some tools used separate tooltip implementations | Shared tools retain a fixed 44px hitbox, 20px icon and consistent stroke; header, graph, text-format and master tools share tooltip behavior |
| Interaction state | Hover and selected states used the same fill; inspector state was not exposed | Neutral hover, blue selected fill/border, `aria-pressed` on the inspector toggle and a separate selected-layer marker; keyboard focus stays black/yellow |
| Header | Long presentation names wrapped the header or extended the rename control off-screen | A constrained title column ellipsizes text without changing header height; compact controls and responsive rows preserve access to file commands |
| Command recognition | Insert objects/icons and architecture/process commands reused the same icon pairs | Separate Shapes, Sticker, Network and Workflow icons, with labels preserved |

The first tooltip regression failed against the old implementation. Four long-title tests reproduced header growth or an off-screen rename control; a command test detected only two distinct icons for four commands. A review identified stale tooltip state when another edit disabled and re-enabled the hovered tool. A real `edit_slides` request gate reproduced this, then passed after clearing open state when disabled. No layout, document-format, generation or file-permission policy was loosened.

Final checks rerun for this UI-only follow-up: **190 tests passed**: Rust workspace 128, complete Studio browser suite 57, generation browser 4 and native WebView 1. The earlier Node 27 and native-unit 4 tests were not rerun and are not counted again. Production frontend build, Tauri build and lint passed. The added browser checks cover short/long tooltips at 1200px/390px, hoverable content, Escape inside a native dialog, real Tab focus, fixed selection dimensions, busy-state recovery and long titles at 1440px/1200px/960px/390px. Existing dialog checks additionally measure every tool button; all 108 part previews still render.

Screenshots from those four widths, tooltip states, authoring panels and the actual native WebView were inspected. The native scenario also passed direct editing, graph/part operations, OS save/cancel/reopen and close-cancel protection. The previous user window was preserved by renaming its loaded generated executable and loader. Cargo reused a cached build script and did not restore the loader path; copying the identical preserved DLL into the missing path fixed startup, after which the complete native test passed. No user process was terminated.

Automated accessibility checks and screenshots do not constitute complete WCAG conformance or a stored visual-diff baseline. Review agents were excerpt-limited. No new Office visual-parity result is claimed for a change confined to application chrome; the earlier document qualification below remains historical evidence.

## User Operations

- Right-click canvas, slide thumbnails, layers, objects and graph nodes/connections for context-specific commands. Shift+F10, the context-menu key, arrows, Home/End and Escape operate the menus; dismissing restores focus. Coordinates are clamped to the viewport.
- New presentation creates one blank slide. New slide, duplicate, remove, move and rename work on authored and supported reopened PPTX files. The last slide cannot be removed. Graph/part metadata and source bindings participate in the same Undo transaction.
- Save As accepts a filename; Save preserves the opened source by publishing a new file. Browser output starts a download; native output uses the OS save dialog and verifies published bytes. New/Open/desktop close protect unsaved changes with Save / Discard / Cancel. Apply or cancel pending inline/property drafts before saving.
- Shortcuts: Ctrl/Cmd+N new presentation, O open, S save, Shift+S save as, M new slide, D duplicate, Z undo, Shift+Z or Y redo. Native text-input shortcuts remain local to the field.
- Insert icons provides all 1,818 icons in Lucide React 1.43.0, full-catalog search, pagination, color, stroke and size. Import SVG/PNG/JPEG files, drag them onto the canvas or paste supported clipboard data. Figma Copy as SVG can also go into the SVG text field. No account credentials or `.fig` parsing are involved.

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

Distinct automated test count at the preceding workspace-operations checkpoint: **214 passed**, without counting repeated runs twice. The UI follow-up above records its own later reruns.

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

Lucide icons retain their ISC license, including the package's MIT notice for Feather-derived icons. The picker uses the existing `lucide-react` dependency and does not copy Figma libraries or third-party slide engines. Imported design assets retain their own terms; supplying them is not a transfer of rights. DADS attribution and font notices remain in the [design reference](graphs-and-dads.md#design-reference).

Locked Rust dependency metadata reports `resvg`/`usvg` 0.48.1 as Apache-2.0 OR MIT and `tiny-skia`/`tiny-skia-path` 0.12.0 as BSD-3-Clause. Their terms do not select the AISlide project license.