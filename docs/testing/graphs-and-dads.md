# Architecture Graphs And DADS-Inspired Studio

## 2026-09-20 Cloud And Workspace Qualification

**Qualified candidate; publication and regular installation recorded separately.** The current NSIS candidate is built, not yet normally installed or publicly committed. Cloud icons, icon-first nodes, nested boundaries and resizable panes share the existing core/SDK/MCP contracts. See [API limits](../api.md#architecture-graphs), [cloud setup and workflow](../authoring/cloud-icons.md) and [pane behavior](workspace-ux.md#resizable-workspace-panels).

| Final local gate | Result |
| --- | --- |
| Rust workspace | 535 passed, 6 existing ignored; interrupted earlier attempt excluded |
| Edge, one five-file run | 80 passed, 0 failed/skipped/retries: authoring QA 34, authoring 13, page-size editing 1, graphs 15, workspace UX 17 |
| Node | 62 passed, 1 intentional installed-lifecycle skip: client 28, MCP 6, bridge 6, cloud setup 16, Lucide 2, setup 4 |
| Build/typecheck and lint | Passed; 0 lint errors, 5 pre-existing warnings (3 Fast Refresh, 2 unrelated BOM); bundle-size warning retained |
| Development native, one final run | 3 passed, 0 failed/cancelled/skipped, 51,225 ms |

Native checks cover Fit/wheel/pane resize and preference reload, existing authoring with a local synthetic model fixture, and cloud PNG preservation/nested editing in owned processes. One test-only save-dialog readiness repair retained original timeouts and safety checks. User settings stayed unchanged. Process-restart recovery was not in this three-test pattern. Local evidence: `.artifacts/cloud-core-final-attributable-XJ73zj/summary.json`, `.artifacts/cloud-final-browser-20260920-8b615d/handoff.md`, and `.artifacts/cloud-desktop-final-20260920-a91f7c/handoff.md`.

The palette retains 1,818 Lucide entries/42 categories plus 1,498 cloud source identities (Azure 638 + Entra 7, AWS 808, GCP 45). Vendor art is optional local data, not public-source or installer payload. The original three-slide `--cloud-icons` demo has 23 nodes, 18 boundaries, 40 images, 14 connectors and 27 vendor identities; MCP verified pinned PNGs, native attachments, metadata reopen and byte-identical Undo. These are synthetic examples, not deployed-system facts or copies of user diagrams.

Six earlier sample views at 1440x960/390x844 loaded 40 images and passed 82 visual checks (`.artifacts/cloud-visual-20260920-7eac91/summary.json`); that earlier CLI rendering is not pixel-equivalence proof for the final binary or Office. Deterministic routes can share the Azure corridor and overlap; there is no automatic obstacle avoidance. Logical nesting remains flat editable native objects within one graph root. Moving only a shape in Office does not move its separate label/picture. No new Office/schema-parity qualification is claimed.

The final 80-case browser run and native cloud run reported no collected window/page errors, with non-suppressing collection. Earlier intermittent ResizeObserver failures remain recorded; these bounded successes do not prove universal elimination. Current filesystem-backed code/security review and regressions cover local-drive rejection and exact catalog bytes, but are not a full independent security audit or race-free filesystem sandbox.

## Historical Checkpoints

Everything below records earlier dated implementations, limits, hashes and reviews. Superseded limits, combined-run counts, publication/CI status and excerpt-only review notes are historical, not the current contract; original evidence is retained unchanged.

Date: 2026-09-15. This report records graph authoring through shared core, Studio, SDK and MCP. The node-icon follow-up below is local and uncommitted, like the preceding [UI consistency follow-up](workspace-ux.md#ui-consistency-follow-up). Earlier checkpoint results remain separately recorded. No new commit, push or project-license selection is implied.

## Node Icons

Select a node in **Architecture diagram** and choose **Choose icon**. The shared library now includes all 1,818 icons in Lucide React 1.43.0 following the [2026-09-16 expansion](workspace-ux.md#icon-library-expansion); color/stroke controls and SVG/PNG/JPEG file import are available in an editor-owned selection view. **Change icon**, **Remove node icon** and context-menu equivalents use the same graph transaction path. Cancel or Escape returns to the current node form without losing typed fields. Accepting an icon applies the current node form and icon together as one graph-local edit; Undo restores the previous node. Applying the graph creates one document transaction. Local graph history is bounded to 30 entries and 4 MiB per direction.

An optional `GraphNode.icon` stores PNG/JPEG data and alt text. `create_graph_icon` / `client.createGraphIcon` prepare supplied SVG/PNG/JPEG bytes at a maximum 256px longest side. SVG uses the existing inert subset and becomes transparent PNG; larger PNG/JPEG images are downsampled in their original format, while already-small rasters retain exact bytes. The icon is fitted beside the label at up to 48 graph pixels. The original native shape remains the connector target; picture, text and shape are separately editable objects within the graph root. Graph-editor movement updates all three. Moving just the shape in Office does not move its separate picture or label.

Colors are fixed raster colors, not live theme references. There is no Figma account integration, external URL retrieval, raw SVG rendering in the DOM, editable SVG-path output or independent icon-shaped node mode. Existing image, graph, document, metadata and protocol limits remain enforced. Direct raw `GraphIcon` payloads can still reach those limits; the preparation helper is the recommended route. An icon-free graph retains its previous output.

### Verification Evidence

Final local aggregate for the node-icon follow-up: **229 distinct tests passed**.

| Gate | Passed |
| --- | --- |
| Rust workspace | 134 |
| Node bridge, SDK, MCP, extraction and generation | 27 |
| Studio browser | 59 |
| Generation browser | 4 |
| Native unit | 4 |
| Native WebView | 1 |

The full Rust run preceded the final icon-width adjustment; all 14 graph tests passed again after it. The current CLI then passed the complete Node/browser suites and the rebuilt desktop passed its complete native workflow. Frontend/Tauri builds and lint passed. Native icon insertion, graph-local Undo/Redo, replacement, document Undo, save/cancel/reopen and close protection were verified in a dedicated WebView2 profile. The existing user window remained open throughout rebuilding.

- Core regression tests cover PNG/JPEG/SVG preparation, original small raster bytes, non-overlap/aspect ratio, all six shape connection sites, movement, metadata reopen, replacement/removal, exact Undo and stale protection after external image changes.
- SDK and official MCP tests prepare an icon, insert/move/save/reopen it, remove it and undo, while retaining revision checks and source bytes.
- Browser tests cover node-form retention on cancel, icon addition/replacement/removal, graph and document Undo, keyboard movement, preview, standalone PPTX reopen, invalid image rejection and no fetch of an external SVG resource. The picker was checked at 1440px and 390px, including scoped axe, focus and actual rendered PNGs.
- `node tools/graphs-demo.mjs --icons` generates five synthetic slides containing 18 PNG/JPEG icons across all six shapes, 5 root groups and 13 attached connectors. It replaces an icon after native reopen, moves the node, reopens the edited result and restores exact original PPTX bytes with Undo. No input or existing output is overwritten.

Final generated original SHA-256: `fb2040fc6498364eb556edfc7ce02de5741f63e5f12a68a48280996198b696a7` (166,347 bytes). Edited SHA-256: `0380dc5f5abe4089bb7812686a4518550fc26c869281408364537760b88abad6` (189,314 bytes).

The final edited deck passed official Open XML schema validation. PowerPoint opened it in a disposable copy, rendered all five slides, counted all 18 pictures, and verified both attachments plus begin-shape movement for all 13 connectors. All five captures were inspected; a default-width label initially split "Application" in Office, so icon width allocation was reduced and the final deck was regenerated/rechecked. Source SHA-256 remained unchanged. This is sample qualification, not an end-target movement test or a general Office visual-parity guarantee.

### Resource Validation

An initial 18-icon deck exposed repeated full decoding of identical 1024px assets: native `apply_graph` took 34.552 seconds and exceeded the unchanged 20-second CLI bridge budget. Raster validation now retains at most 64 successful `RasterInfo` records per thread, keyed by SHA-256 of the current bytes and MIME. Base64/byte limits, MIME allowance and magic-byte agreement are checked before every lookup. No image pixels, raw bytes or failed validations are cached. Changed content, wrong MIME, truncated/oversized rasters and cache eviction are tested.

After that correction, retaining 1024px icons still caused a document-size rejection on reopen. The graph-specific 256px preparation fixed this without raising the 2 MiB document or 4 MiB protocol limits; ordinary `create_asset` still uses its original resolution. Final production MCP icon replacement completed in 1.860 seconds under the existing limits. This is a measured bounded sample, not a worst-case latency guarantee for arbitrary raw image payloads.

Review-agent feedback was excerpt-limited. The successful-insert test explicitly checks applying the node draft together with its icon; the cancel test checks retaining it without applying. No full independent security audit, measured coverage percentage or complete accessibility conformance is claimed.

## Authoring

Use **Architecture diagram** to create a graph and **Edit graph** on its selected root group to reopen it. Six native shapes, boundary groups, labels, ports, straight/right-angle routes, arrows, dashed lines and theme colors are supported. Dragging, resizing, keyboard movement, property forms, alignment, grid, deletion and local Undo/Redo operate on a typed graph. Preview and JSON use Rust validation; invalid JSON retains its tab and draft. Apply creates one document transaction; cancel leaves the slide unchanged.

Metadata is optional Custom XML inside the PPTX using existing part provenance/stale guards. Native XML remains authoritative. External shape/text/connector edits block stale semantic replacement, not trigger regeneration. Root-only movement/resizing in AISlide preserves semantic editing. Native editing remains possible without metadata.

Definitions: [API](../api.md#architecture-graphs), [support matrix](../support-matrix.md#architecture-graphs), [SDK example](../../packages/client/README.md#architecture-graphs).

## Native Qualification

The five-slide sample was generated with [the synthetic graph demo](../../tools/graphs-demo.mjs) through the official MCP SDK with the GUI closed. It has 5 groups, 29 scene text elements, 25 shapes and 13 attached connectors. Reopening restored all five current metadata records. Node movement after reopen, export and another reopen succeeded. Undo exported exactly the original bytes; the original was not overwritten. Generated presentations and captures remain local; the script is included for reproduction.

Original SHA-256: `101e460905961d76d23013107cebf106eb4c0b9db7f0694bcdf9a64a115d5471`.
Edited SHA-256: `5e7f5c36bcec405f13b804fc84a21bfadff18325e3aa348dbd31a5e9b33d8670`.

Official Open XML schema validation passed. Microsoft 365 PowerPoint opened a disposable copy, rendered all five slides, counted 54 native text-capable shapes, and verified both attachments for all 13 connectors. For each connector, moving its begin-connected shape changed connector geometry while retaining both endpoint identities; the shape was then restored. This is not an end-target movement test. PNGs are post-restoration captures, not screenshots of the transient movement. Source SHA-256 was unchanged. This qualifies these examples, not arbitrary Office parity.

Initially schema-valid custom-path connectors were rejected by Office. A same-graph preset-line control opened; removing zero dimensions did not fix custom paths. Final output uses standard `straightConnector1` and `bentConnector2/3/4`, bounded adjustments and flips. Tests cover both routes across all 16 source/target port combinations, reverse arrows/dashes and unsupported-path rejection.

Native movement then exposed shape-specific indexing: ellipse has eight sites, cylinder five, cloud a different order/inset. The six mappings were measured on self-created Office shapes, independently mapped in core, and regression-tested in four logical directions. Final captures show correct horizontal attachment and vertical two-bend routing after move/restore.

## Verification

Executed gates at the original icon-free graph checkpoint: **190 distinct tests passed**. Counts do not add repeated runs twice.

| Gate | Passed | Scope |
| --- | --- | --- |
| Rust workspace | 118 | Includes 8 graph integration tests; final shape-site mapping |
| Node bridge/SDK/MCP/extraction/generation | 25 | Final CLI; graph reopen/update/Undo and existing safety tests |
| Studio browser | 39 | Full prior 38 plus initialization regression; graph-focused 7 passed on final CLI/UI |
| Generation browser | 4 | Review/apply, failure, cancellation and small-screen dialog |
| Native unit | 3 | Tauri request scoping and cancellation |
| Native WebView | 1 | Final Tauri build; graph create/edit/update/Undo and existing authoring/generation |

Production frontend build, Tauri build, lint, encoding (211 files), diff whitespace checks and scoped editor diagnostics passed. Graph tests cover limits, references, geometry/sites, unsupported-path rejection, metadata, revisions, Undo and operations. The full 38-test Studio run preceded final localized corrections; the affected graph suite and final desktop integration test were rerun after them.

Final development-server startup exposed Windows `EBUSY` while Vite watched locked Rust objects below `src-tauri/target`. The server watch configuration now excludes that generated directory only. A real watcher-ready/HTTP check confirmed exclusion and HTTP 200; a subsequent browser opened the final PPTX and its three-node service graph without page errors. This development-only change does not alter the bundled desktop output or loopback request restrictions.

Screenshot inspection then caught an application-level `Core is busy` error despite no JavaScript exception. StrictMode's initial preview abort/restart could release the frontend request before the backend gate. A new repeated-mount test first failed with zero fitted labels, then passed after adopting the existing PartsPanel completion queue: obsolete results are discarded without aborting the core job, and changes/save await the queue. All seven graph browser tests and the rebuilt native WebView test passed afterward. The previously opened desktop window was preserved under a renamed running executable rather than closed.

- DADS checks at 1440px/390px: local fonts, 16px controls, 44px toolbar targets, black/yellow focus, no page overflow and unchanged slide-table color. Graph/parts dialogs passed scoped axe checks.

Reproduce with `node tools/graphs-demo.mjs` into a new artifact directory and `tools/verify-powerpoint.ps1 -VerifyConnections` on a disposable copy. Required gates include workspace Rust tests, Studio/generation E2E, native unit/WebView tests, build, lint and encoding checks.

Coverage percentage is not measured. Hosted GitHub CI remains externally billing-blocked. Review-agent calls lacked direct filesystem access; subsequent code/React/TypeScript/Rust/security/PowerShell reviews used supplied excerpts and cannot be presented as independent full-source audits. The React save-state concern was resolved by checking that Studio unmounts GraphEditor on successful application. PowerShell review prompted the precise begin-target movement and post-restoration-capture qualifications above. Local source review and executable checks supply the primary evidence.

## Design Reference

Source: [Digital Agency Design System website](https://design.digital.go.jp/dads/), v2.18.0 consulted on 2026-09-15, including typography, color, spacing, buttons, input accessibility, modal guidance and [usage notices](https://design.digital.go.jp/dads/introduction/notices/).

デジタル庁デザインシステムウェブサイトを参考に、AISlide向けに編集・加工。

AISlide adapts locally served Noto Sans JP/Noto Sans Mono, regular 400/bold 700, 16px principal text/14px dense labels, 8/16/24/32 spacing, neutral surfaces, `#0017c1` actions and black/yellow focus. Main inputs/buttons are generally 48px high and toolbar targets 44px. Document fonts/chart colors/themes remain independent. Noto fonts retain [SIL Open Font License 1.1](https://openfontlicense.org/); Fontsource supplies local assets. No Digital Agency logo, endorsement or project-license selection is implied.

This is an adaptation, not full DADS conformity. Application-specific modal forms, safety-disabled busy/invalid controls, maxlength behavior and some existing alert semantics remain. Diagram gestures have property-form/keyboard alternatives. Axe is not a complete WCAG 2.2 audit; full assistive-technology/touch-device qualification remains outside this checkpoint.

## Boundaries

No draw.io XML interchange, arbitrary HTML/executable input, self-loops, nested boundaries, arbitrary waypoints, curves or automatic obstacle/label collision avoidance. Grid preserves sizes and rejects layouts that cannot fit; with boundaries all nodes need membership. Insertion does not reflow other objects.

Labels are separate native text and node icons are separate pictures: graph-editor movement moves both and recomputes connections, but moving only a shape in Office does not move its label or icon. Office edits may mark metadata stale on reopening. Unknown imported content is preserved, never executed or fetched.

## Self-Evaluation

Delivered scope: working graph authoring and DADS-inspired application chrome, with native output and bounded qualification.

| Axis | Score | Evidence and improvement |
| --- | --- | --- |
| Accuracy | 4/5 | Native Office tests caught two issues schema checks missed; arbitrary input/Office parity remains unqualified |
| Completeness | 4/5 | Agreed core/MCP/editor/theme scope implemented; broader assistive-technology testing remains unmeasured |
| Clarity | 4/5 | API limits and DADS adaptations documented; keep implementation diagnostics out of the user handoff |
| Actionability | 4/5 | Native sample, reproducible MCP script and editor provided; unsigned local development build only |
| Conciseness | 3/5 | Troubleshooting produced many updates; final handoff is reduced to entry point, capabilities, evidence and exclusions |

Overall: 3.8/5. Improvements by impact: retain Office movement checks when changing connection mappings; expand assistive-technology qualification before production; keep handoff concise. The assessment is evidence-based, not an assumption of user acceptance.