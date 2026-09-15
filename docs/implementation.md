# AISlide Implementation

## Research PoC

The initial slice has been extended across the three research proofs. See the [PoC tracker](poc-status.md), [support matrix](support-matrix.md), [API contracts](api.md) and [acceptance evidence](testing/poc.md). Arbitrary PowerPoint feature parity and production readiness are not established.

- Independently implement PresentationML with generic ZIP/XML libraries. Do not copy an existing PPTX engine.
- Use a Rust core shared by a headless command line, MCP, and the Tauri editor.
- Generate a deterministic, editable data report from structured input.
- Preserve the original archive exactly on a no-op save. On a supported edit, preserve untouched part payloads; do not claim unchanged ZIP container bytes after repacking.
- Reject unsafe archive paths, duplicate parts, encryption, oversized expansion, DTDs, and unsupported edits.
- Never execute embedded content or automatically fetch external relationships.
- Keep PowerPoint optional and validate only disposable copies.

## Delivered

`crates/aislide-core` owns the bounded package container, strict scene/report models, deterministic layouts, native PPTX generation, simple imported text patches, and JSON operation dispatch. `crates/aislide-cli` and the Tauri shell both call that core. The official-SDK MCP server and the development HTTP adapter invoke the CLI through a bounded fixed-executable bridge.

Studio supports structured reports, model drafts, source mapping, source-bound project checkpoints, text/geometry/style edits, native chart data, pictures/crop, process groups, Undo/Redo, measured layout checks and bounded native import. Charts are real DrawingML chart parts with editable embedded XLSX; pictures and connectors are native objects, not slide screenshots.

`generation.rs` owns the OpenAI-compatible transport, operator-only provider configuration, remote consent policy, bounded response parsing, report compilation and unverified provenance. It exposes synchronous CLI and asynchronous Tauri entry points. Studio applies a draft only after confirmation; MCP creates a separate in-memory deck. Cancellation drops the Rust HTTP future in the native app, or terminates the fixed CLI process in the development/MCP adapters. Native cancellation is scoped to the requesting window and operation ID.

Generation is a bounded structured-output workflow, not an autonomous agent loop. It has no model tools, browsing or external file access. HTTP errors are not retried. A user may explicitly allow one report-validation repair inside the same deadline. Schema mode derives JSON Schema from Rust types and can enforce an approved outline. A pinned real local Qwen model has been exercised through MCP; fixture tests remain separate from that evidence. Content and citations are never automatically declared factual.

`sources.rs` and `extraction.rs` parse bounded input without execution. `data_report.rs` binds chart/table values to exact source cells. `document.rs` seals canonical state, validates revisions and sources, applies atomic JSON Patch and produces checked inverse receipts. `packages/client` is the shared transport/session adapter, not a second document engine. Original-preserving import retains immutable input bytes in the document. Project publication is exclusive per file; racing collisions can leave a reported partial pair, and recovery does not delete public paths.

Native input is intentionally restricted: classic non-ZIP64 packages, Transitional PresentationML, up to 256 inspected slides, UTF-8 XML, direct top-level text shapes only for patches. Signed-package edits and ambiguous/empty/mixed/CDATA text runs are rejected. Opaque parts are preserved, not rendered or activated.

Native style CSP currently permits inline styles because geometry is rendered as React style properties; scripts remain restricted to bundled application code. Raw document HTML, SVG, script and remote relationships are not executed.

## Initial Acceptance Checks

| Contract | Evidence |
| --- | --- |
| No-op save is byte-identical | `cargo test -p aislide-core --test package` |
| Editing one part preserves opaque part payloads | `cargo test -p aislide-core --test package` |
| Path traversal and unbounded expansion are rejected | `cargo test -p aislide-core --test package` |
| Generated objects remain PowerPoint-native | Package structure tests and optional M365 check |
| GUI and headless clients use one layout result | Shared core integration tests |

## Remaining Boundaries

Full arbitrary import/master/theme editing, SmartArt editing, exact Office text/rendering parity, broad cloud-model quality, OS-backed credential storage, persistent history/autosave, automatic scanned-PDF rendering, signed installers and platform expansion remain outside this research checkpoint. Native blocking extractors are bounded but not process-isolated. No installed Office fonts, model binaries or third-party presentations are redistributed with source.

Development uses [murasamelabo/AISlide](https://github.com/murasamelabo/AISlide). On 2026-09-15 its owner confirmed changing it to Public and authorized the source push. The project license has not been selected. Generated deliverables, task-specific examples and input materials, local toolchains, environment files and test artifacts remain excluded from source publication. See [current verification and publication scope](testing/workspace-ux.md).