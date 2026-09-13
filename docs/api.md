# Shared API Contracts

Every operation is strict JSON with a discriminator `op`. Unknown fields fail. The CLI reads one request from stdin and prints one response; errors use stderr and a nonzero exit code. Tauri and the development adapter share the same dispatch in `aislide-core`.

## Document Operations

| Operation | Request fields besides op | Result |
| --- | --- | --- |
| `new_document` | `id`, `deck`, optional `sources`, `bindings`, `report` | Canonical document with revision 0 and content hash |
| `transaction` | `document`, `transaction: {expected_revision, expected_hash, operations}` | Updated document, inverse receipt, changed JSON paths |
| `undo_transaction` | `document`, `expected_revision`, `receipt` | Updated document and a receipt usable for redo |
| `export_project` | `document` | `base64`, `filename`, `checkpoint`, `checkpoint_filename`, layout evidence |
| `open_project` | `base64`, `checkpoint` | Verified document, only if the exact PPTX matches |
| `import_document` | `id`, `base64` | Origin-bound document, warnings, per-object editable fields |

Content JSON Patch paths start with `/deck`, `/sources`, `/bindings`, `/report` or the immutable import origin. A batch has 1-128 operations; a stale revision/hash, failed test/path, oversized intermediate result or invalid final state aborts the batch. Revisions increase on real changes and Undo/Redo; no-op transactions do not create history. Core receipts are not signed capabilities and undergo the same validation as edits.

## Source And Generation Operations

`ingest` takes `input: {name, format, base64, ocr?, ocr_language?, attribution?}`. Formats are `csv`, `json`, `xlsx`, `markdown`, `text`, `pdf`, `png`, `jpeg`. Attribution fields are `citation`, `url`, `license`, optional `derived_from_sha256` and `transformation`. URLs are metadata only. The result preserves raw-byte identity and a separate extracted-content hash, source tables with exact locators, plain text, page/word evidence, and limitations.

`data_report` takes a source document and `mapping: {title, period, table_index, category_column, value_columns, row_start, row_count, chart_kind}`. Indices are zero-based. It creates exactly twelve slides, native charts/tables/process graphics, and field bindings; it is deterministic source compilation, not AI generation. Missing/formula/nonnumeric selected cells fail rather than becoming zero.

`generate` takes `input: {prompt, source_text?, slide_count, allow_remote?, max_repairs?, outline?}`. Each outline entry is `{title, layout}`. Model configuration comes only from the host environment. `max_repairs` accepts 0 or 1, default 0; repair is a separately authorized model call for report validation, not HTTP retry. Provenance includes model, local/remote mode, source hash, duration, attempts and `verified:false`. Model output never grants filesystem/network authority.

The core's report schema is generated from Rust types in schema mode and further constrained by the requested count/outline. Not every OpenAI-compatible server supports every JSON Schema keyword; runtime validation remains authoritative. The pinned local llama.cpp qualification includes a regression for `items` masking `prefixItems`.

## Graphics And Inspection

- `create_picture`: `id`, `base64`, `mime_type`, `alt`; returns a decoded, fitted picture element.
- `create_diagram`: `id`, `steps`; returns native rectangles, text and connected arrows in a group.
- `compile`: `report`; returns a scene and issues. `sample` returns the explicitly synthetic twelve-slide input.
- `validate`: `deck`; returns structural status and a canonical deck with serde defaults applied.
- `measure_layout`: `deck`; reports text/table metrics, fonts, overflow and missing glyphs, never Office parity.
- `inspect`: `base64`; lists simple top-level text runs.
- `patch_text`: `base64`, `part`, `shape_id`, `run_index`, `expected`, `text`; returns a new archive or an optimistic conflict.
- `import_pptx`: `base64`; approximate scene, warnings and writable fields.
- `save_import`: `base64`, `deck`; original bytes for no-op or narrowly patched original package.
- `roundtrip`: `base64`; inspected no-op archive. `package_manifest`: `base64`; all part byte lengths and SHA-256 values.
- `export`: `deck`; structural-only PPTX. `provider_status` and `ocr_status` report configuration/capability, not content quality.

## MCP Mapping

MCP retains up to eight document and source handles. `ingest_source` returns a source handle; `compile_data_report` consumes it. `get_document` includes revision and sources, while `get_deck` retains the legacy scene-only shape. `apply_transaction` requires a revision. `update_text`, `undo`, `redo`, `add_picture`, `add_diagram`, `measure_layout`, `import_pptx`, `open_project`, `export_project` and legacy `export_pptx` use the shared document/session behavior.

Only `--output-dir` enables MCP file output. Filenames are restricted to plain `.pptx` names; no generic path or shell operation exists. Model tools declare external-world behavior because operator-approved remote inference is possible. Imported relationships remain inaccessible regardless of model permissions.