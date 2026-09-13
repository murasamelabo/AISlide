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

- Hosted GitHub Actions was blocked by account billing/spending limits before any step ran. Keep the repository Private and do not change billing or add runners to bypass it.
- A real model is not bundled. Provider configuration and explicit authorization are needed for live remote calls; local test fixtures never count as model-quality evidence.
- A project license and public release remain unauthorized. Public test files may only be downloaded from approved public sources with recorded reuse terms; no Office fonts or third-party slide engine code may be redistributed.

The final combined verification gate passed: 106 automated tests, the embedded desktop build, 24 public no-op cases, 12 eligible public patches, actual local model generation through MCP and Office verification of both public-data and model decks. See the [acceptance report](testing/poc.md). Production-level arbitrary import editing, exact Office typography, signed distribution and cross-platform qualification are not implied by this research result.