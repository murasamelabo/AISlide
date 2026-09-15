# Architecture Graphs And DADS-Inspired Studio

Date: 2026-09-15. This report records the graph-authoring checkpoint through shared core, Studio, SDK and MCP. It does not implement draw.io file interchange or change existing slide themes. Current aggregate gates and publication scope are in [workspace verification](workspace-ux.md).

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

Final executed gates: **190 distinct tests passed**. Counts do not add repeated runs twice.

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

Labels are separate native text: graph-editor movement moves labels and recomputes connections, but moving only a shape in Office does not move its label. Office edits may mark metadata stale on reopening. Unknown imported content is preserved, never executed or fetched.

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