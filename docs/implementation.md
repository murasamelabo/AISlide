# AISlide Implementation

## First Vertical Slice

Status: first working slice implemented and exercised. The twelve-week PoC is a roadmap, not a completed feature set. See [verification evidence](testing/first-slice.md).

- Independently implement PresentationML with generic ZIP/XML libraries. Do not copy an existing PPTX engine.
- Use a Rust core shared by a headless command line, MCP, and the Tauri editor.
- Generate a deterministic, editable data report from structured input.
- Preserve the original archive exactly on a no-op save. On a supported edit, preserve untouched part payloads; do not claim unchanged ZIP container bytes after repacking.
- Reject unsafe archive paths, duplicate parts, encryption, oversized expansion, DTDs, and unsupported edits.
- Never execute embedded content or automatically fetch external relationships.
- Keep PowerPoint optional and validate only disposable copies.

## Delivered

`crates/aislide-core` owns the bounded package container, strict scene/report models, deterministic layouts, native PPTX generation, simple imported text patches, and JSON operation dispatch. `crates/aislide-cli` and the Tauri shell both call that core. The official-SDK MCP server and the development HTTP adapter invoke the CLI through a bounded fixed-executable bridge.

Studio supports report JSON compilation, model draft generation and review, scene checkpoints, manual text/geometry/style edits, slide ordering/duplication, Undo/Redo, PPTX export, and an imported-text inspector. Tables are native `a:tbl` objects; no charts or images are simulated as equivalent supported objects.

`generation.rs` owns the OpenAI-compatible transport, operator-only provider configuration, remote consent policy, bounded response parsing, report compilation and unverified provenance. It exposes synchronous CLI and asynchronous Tauri entry points. Studio applies a draft only after confirmation; MCP creates a separate in-memory deck. Cancellation drops the Rust HTTP future in the native app, or terminates the fixed CLI process in the development/MCP adapters. Native cancellation is scoped to the requesting window and operation ID.

Generation is not an agent loop: there are no model tools, browsing, external file reads, retries, or provider fallbacks. The model produces strict report JSON, not executable source or OOXML. Source text is untrusted input. Structural validation and a source hash do not prove factual accuracy. Integration tests use synthetic HTTP fixtures; real provider qualification remains open.

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

## Not Yet Implemented

Full import editing, SmartArt editing, exact Office font parity, native charts, XLSX/PDF/OCR ingestion, multi-step AI planning, OS-backed credential storage, full SDK, and a public compatibility corpus remain subsequent work. No installed Office fonts or third-party presentations will be redistributed.

Development uses the user-requested Private repository [murasamelabo/AISlide](https://github.com/murasamelabo/AISlide). The OSS license has not been selected and public release is not authorized. Generated files, local toolchains, environment files and test artifacts are excluded from source control.