# PoC Completion Tracker

This tracker follows the approved research PoC. An implemented API is not evidence of Office fidelity or factual accuracy. The supported subset and residual boundaries are explicit in the [support matrix](support-matrix.md).

## Proof Goals

- [x] Twelve-slide Japanese/English report using attributed public World Bank data, native text/table/charts/picture/process objects, source bindings and measured layout checks; Office opening, rendering and chart-data editing verified.
- [x] Pinned public 24-file fixture corpus and adversarial tests: all no-op archives byte-identical; all 12 eligible simple-text patches preserve non-target payloads. No third-party binaries are committed or redistributed. Approximate preview is not a fidelity pass.
- [x] GUI-free SDK/MCP intake, source-bound compilation, atomic edits, Undo/Redo, validation, project export/reopen and cancellation. Actual local model generation through MCP also verified with a supplied outline and known synthetic values.

## Implementation Order

- [x] Shared Rust package/report core, first editor, basic CLI/MCP, synthetic benchmark.
- [x] Operator-configured model transport, strict output, remote consent, draft review, cancellation and local fixture tests.
- [x] Native charts and embedded chart workbooks; chart editing in Studio and headless operations.
- [x] Pictures, connectors and groups with bounded media ingestion and native export.
- [x] CSV/JSON/XLSX/text evidence ingestion, explicit field mapping and provenance; bounded PDF extraction and actual Windows image OCR qualification.
- [x] Shared transactions, revision/precondition checks, batch undo, source-bound checkpoints and a reusable SDK.
- [x] Conservative typography/overflow checks, richer report recipes and editable diagram layouts.
- [x] Supported visual import/edit mapping, preservation contracts and public corpus runner.
- [x] Real local model qualification, complete headless scenario, Office open/render/edit verification, and final measured acceptance report.

## External Prerequisites

- The original hosted GitHub Actions run was blocked by account billing/spending limits before any step ran. No billing or runner changes are made to bypass it; current hosted results must be checked for the actual pushed commit.
- A real model is not bundled. Provider configuration and explicit authorization are needed for live remote calls; local test fixtures never count as model-quality evidence.
- A project license remains unselected. The owner authorized public source publication on 2026-09-15; input materials and generated deliverables are excluded. Public test files may only be downloaded from approved sources with recorded reuse terms; no Office fonts or third-party slide engine code may be redistributed.

The original research-PoC gate passed: 106 automated tests, the embedded desktop build, 24 public no-op cases, 12 eligible public patches, actual local model generation through MCP and Office verification of both public-data and model decks. See the [acceptance report](testing/poc.md).

## Authoring Follow-up

- [x] Direct canvas text/shape/table/group editing with IME handling, explicit commit/cancel, resize and undo.
- [x] Native multiple masters/layouts, common logos/text/shapes, placeholders, editable theme colors/fonts and layout inheritance/reset.
- [x] Forty native shape presets, nine editable chart types, table/line insertion and text formatting/hyperlinks.
- [x] Shared core, SDK and MCP APIs with revision checks; unsupported imported design/format changes fail closed.
- [x] Eighteen-slide native Office fixture: all slides rendered, nine workbooks edited, actual master/layout/theme inheritance verified with source hashes unchanged.
- [x] Authoring browser and actual Tauri interaction tests; responsive dialog/axe checks.

See [authoring verification](testing/authoring.md). Production-level arbitrary import editing, exact Office typography, signed distribution and cross-platform qualification are not implied by either checkpoint.

## Single-PPTX And Parts Follow-up

- [x] Standalone current-native-XML PPTX opening/saving with internal source and part metadata; no required external checkpoint.
- [x] Thirty-six categories, three original layouts each, theme-linked native objects and typed data validation.
- [x] Shared insert/update/delete metadata transactions, stale protection, Undo/Redo, SDK and MCP integration.
- [x] Studio Parts library, structured/JSON inputs, draft retention, previews and selected-part editing.
- [x] All 108 defaults generated/reopened and screenshot-checked; desktop/mobile controls and native WebView workflow verified.

The gates at the parts checkpoint totaled 171 passing tests: Rust workspace 110, Node 23, Studio browser 30, generation browser 4, native unit 3, native WebView 1. The final Venn-only geometry change was followed by the full 110 Rust tests and the six parts browser cases, including all 108 previews. All 108 sample slides were rendered in PowerPoint, 24 embedded chart workbooks accepted edits in disposable copies, and an edited/re-saved native part deck reopened in Office. This is test execution evidence, not a measured coverage percentage or universal visual-fidelity result. See [parts verification and limitations](testing/parts-library.md).

## Graph And Workspace Follow-up

- [x] Typed graph editor, shared SDK/MCP operations, attached native connections, boundaries, metadata and Undo/Redo.
- [x] DADS-inspired application controls, local fonts, responsive dialogs and visible keyboard focus.
- [x] Context menus for canvas, objects, layers, slides and graph editing, including keyboard operation.
- [x] Blank files, slide insertion/duplication/deletion/order/rename, Save As and unsaved-draft protection.
- [x] Searchable 36-icon picker and bounded SVG/PNG/JPEG insertion through file, drop and paste handlers.
- [x] Full 214-test local aggregate; native OS save/cancel/reopen and close/cancel verified.
- [x] Synthetic five-slide native-reopened deck passed Open XML and PowerPoint render, chart-workbook and connector-movement checks.

Current counts are Rust 128, Node 27, Studio browser 50, generation browser 4, native unit 4 and native WebView 1. Build, lint, source encoding and publication audit are recorded separately. Public source publication is authorized by the owner; project-license selection, signed distribution and broad Office parity remain outside scope. See [workspace verification](testing/workspace-ux.md).