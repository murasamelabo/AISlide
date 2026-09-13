# AISlide Studio

A local-first slide editor with an independently implemented Rust PresentationML core, a Tauri desktop application, and headless CLI/MCP access.

**Status: research PoC covering native editable generation, package-preserving import, source-bound transactions and headless automation.** A real local model, a public-data bilingual deck, and a 24-file public PPTX corpus have been exercised. This is not a full PowerPoint replacement or a production release. See the [acceptance report](docs/testing/poc.md) and [support matrix](docs/support-matrix.md).

The built-in example remains deterministic and explicitly synthetic. Model generation is a separate operation. Existing PPTX engine code is not used.

Development repository: [murasamelabo/AISlide](https://github.com/murasamelabo/AISlide), **Private**. No public release or project license has been authorized.

## Run

Prerequisites: Node.js 24+, Rust stable, and the platform dependencies described in the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). Windows 11 with Microsoft 365 PowerPoint is the initial compatibility target. PowerPoint is optional at runtime.

```sh
npm ci
npm run core:build
npm run dev
```

Open **http://127.0.0.1:4173**. The development-only HTTP adapter launches the same Rust CLI used by MCP. It accepts only same-origin requests on loopback; it is not a deployable web backend. Use another port when occupied, for example `npm run dev --workspace studio -- --host 127.0.0.1 --port 4174`.

### Desktop

```sh
npm run build
npm run tauri:build
```

On Windows, run `apps/studio/src-tauri/target/debug/aislide-studio.exe`. This embeds the frontend and requires no Vite server. It is an unsigned development executable, not an installer. `npm run tauri:dev` runs the native development shell while Vite is running separately.

This session also verified an isolated x64 GNU Rust/LLVM-MinGW toolchain under ignored `.tools/` on Windows ARM64. `tools/cargo.mjs` uses that local installation when present and otherwise calls Cargo from PATH. Native ARM64/MSVC builds have not been verified. The local toolchain is not part of the application distribution and is not downloaded automatically on a new checkout.

### Optional Local Model

The optional qualification setup downloads about 1.1 GB of pinned, hash-verified Qwen2.5-1.5B Q4_K_M weights and an official llama.cpp Windows CPU runtime into ignored `.tools/local-model`. It does not install a system service, change global AI settings, or require cloud credentials. Model quality varies; approved outlines and `schema` mode give this small model a more reliable structure.

```powershell
npm run model:setup
$runtime = Get-ChildItem .tools/local-model/llama-*-bin-win-cpu-*.zip | Select-Object -First 1
Expand-Archive -LiteralPath $runtime.FullName -DestinationPath .tools/local-model/runtime
npm run dev:local-model
```

The final command prints a free loopback Studio URL and owns both servers. Ctrl+C closes them. Do not extract over an existing runtime without checking its version. To run a bounded, GUI-free real-model qualification instead, use `npm run model:qualify`; it generates twelve slides through MCP, checks known synthetic chart values, edits, undoes, exports, and stops its model server.

## Available Now

- Twelve-slide synthetic example plus a source-bound report compiler, with cover, metrics, table, columns, statement, chart and process recipes.
- Editable PowerPoint text, rectangles, tables, column/bar/line charts with embedded XLSX, PNG/JPEG pictures with crop, nested groups, connected arrows, minimal master/layout/theme parts and notes.
- Slide navigation, duplication/reordering, object selection, dragging/arrow-key movement, position/size/text/color edits, and Undo/Redo.
- CSV, scalar-table JSON, bounded XLSX, UTF-8 text/Markdown, text PDF, and PNG/JPEG evidence intake. Explicit column/range mapping; optional Windows-local image OCR with word regions.
- Source hashes, exact cell/page locators, attribution and transformations. Changing a bound value invalidates its citation until corrected or explicitly detached.
- Shared revision/hash-checked atomic JSON Patch transactions, bounded Undo/Redo, and a portable JavaScript/TypeScript client.
- Model-generated reports with optional approved outlines, Rust-type-derived JSON Schema, at most one explicitly enabled validation repair, cancellation and review before application.
- PPTX-bound project checkpoints with sources and scene state. **Save project** writes a new PPTX and matching `.aislide.json`; **Open project** validates both files. Native save uses one dialog. No autosave or persisted undo stack yet.
- Approximate native PPTX import with explicit warnings and limited non-destructive text/geometry editing; the original package is retained in imported projects.
- Exact original bytes on a no-op round trip. A supported patch changes its slide part while preserving untouched entry payloads; edited ZIP containers are not claimed byte-identical.
- Installed-font measurement of text/table cells, overflow and missing-glyph detection, and observable font fallback. New project export blocks measured text errors; measurement is not Office parity.
- Strict schemas, bounded archives/rasters/PDF streams, unsafe-name/duplicate-entry rejection, and no fetching or execution of imported content.

Use **Sources** for data mapping and source review, **Open presentation** for bounded native import, and **Validate layout** for font diagnostics. **Inspect PPTX** remains the independent simple-text inspector. **Report data** accepts structured report JSON, never arbitrary HTML/CSS or executable model output. Legacy scene JSON saves only the scene, not sources or the original imported package; use project checkpoints to retain those.

## Headless CLI

The executable is `target/debug/aislide.exe` on Windows, or `target/debug/aislide` on other systems.

```sh
aislide sample
aislide generate input.json new-report.pptx
aislide inspect existing.pptx
aislide request
```

`request` reads one JSON document from stdin and writes one JSON result to stdout. Errors go to stderr with a nonzero exit status. `generate` uses atomic, create-new publication and never overwrites the input or an existing destination.

The shared JSON API includes `sample`, `compile`, `provider_status`, `generate`, `ingest`, `data_report`, `create_picture`, `create_diagram`, `new_document`, `transaction`, `undo_transaction`, `export_project`, `open_project`, `import_document`, `import_pptx`, `save_import`, `measure_layout`, `ocr_status`, `export`, `validate`, `inspect`, `roundtrip`, `package_manifest` and `patch_text`. Its request and response limit is 4 MiB including base64; a revisioned document is limited to 2 MiB and can therefore accept less than the raw file limit. See [API contracts](docs/api.md).

The CLI `generate` subcommand and low-level JSON `export` perform deterministic structural export. Use document/project operations for source-integrity and measured-layout gates. Model inference uses the JSON `generate` operation, not the similarly named CLI subcommand.

```sh
npm run demo
```

This produces a uniquely named PPTX in `.artifacts/` without overwriting previous results.

## MCP

```sh
node tools/mcp.mjs --output-dir ./output
```

The official MCP SDK exposes stdio tools for model generation, source ingestion/mapping, complete document reads, atomic edits, Undo/Redo, native picture/diagram insertion, layout measurement, original-preserving import and project export/reopen. The previous tools such as `compile_report`, `update_text` and `export_pptx` remain available. The GUI need not be running. `aislide://report/example` supplies the structured example. VS Code configuration is in [.vscode/mcp.json](.vscode/mcp.json).

Generation creates a new in-memory document and never automatically exports it. Up to eight documents and eight source handles are retained; each document's Undo/Redo history has count and memory caps. Export is disabled without `--output-dir`, accepts a plain filename only, and refuses existing files. A project pair is published per file, not as an atomic filesystem transaction. Concurrent collisions or process interruption can leave a partial pair, which must be checked before use; recovery never deletes published paths. No generic filesystem, shell, or network tool is exposed.

## Reusable Client

[packages/client/index.mjs](packages/client/index.mjs) accepts a transport function, so Node, Studio and Tauri use the same session behavior. The client supplies immutable snapshots, revision-checked edits, inverse receipts, Undo/Redo, intake and project APIs. Hashes and receipts are integrity/concurrency metadata, not authentication credentials. See [the client guide](packages/client/README.md) for a complete example.

## Model Configuration

Set process environment variables before starting Studio, Vite, or the MCP server. AISlide does not automatically load `.env` files. For a running local OpenAI-compatible server:

```powershell
$env:AISLIDE_AI_BASE_URL = 'http://127.0.0.1:1234/v1'
$env:AISLIDE_AI_MODEL = 'your-installed-model-id'
npm run dev
```

The server must support non-streaming `POST /v1/chat/completions` and return a single JSON report. The path is derived from the configured base URL; the sample port and model name are placeholders, not a bundled server. No model is installed automatically.

| Variable | Behavior |
| --- | --- |
| `AISLIDE_AI_BASE_URL` | Operator-controlled OpenAI-compatible base URL |
| `AISLIDE_AI_MODEL` | Model identifier, required |
| `AISLIDE_AI_API_KEY` | Optional bearer credential, supplied through the local process environment, never chat or document input |
| `AISLIDE_AI_ALLOW_REMOTE` | `1` permits non-loopback HTTPS; otherwise remote endpoints are rejected |
| `AISLIDE_AI_JSON_MODE` | `1` for JSON object mode; `0` for providers without `response_format`; `schema` for compatible JSON Schema servers |
| `AISLIDE_AI_TIMEOUT_SECONDS` | 1-300 seconds; default 120 |

Remote transmission additionally requires per-request `allow_remote: true`, or the Studio consent checkbox. Brief and source text are sent to the displayed endpoint. Endpoint and credential overrides in UI, model output, or MCP arguments are rejected. Redirects, HTTP retries, system proxies, and external relationship fetching are disabled.

Studio leaves the existing deck unchanged until **Apply draft**. Cancellation, malformed output, or provider failure does not replace it. Model output passes the same report and geometry validation as deterministic input, but facts and citations remain **unverified**. There is no silent sample/template fallback.

`max_repairs` is `0` by default. Setting it to `1` authorizes one extra model call for a report schema/layout validation error, within the same overall time budget. HTTP errors, refusals and connection failures are not automatically retried. Optional `outline` entries fix the exact titles and layout order. Returned provenance includes the attempt count. Generated content and supplied source text are retained locally in the document/checkpoint when applied or attached; remote consent only controls transmission to the provider.

The `provider_status` operation reports configuration only, not connectivity or credential validity. For MCP generation, configure the client request timeout longer than the provider budget; the CLI bridge permits up to 310 seconds. A real local Qwen2.5-1.5B run was qualified through MCP; arbitrary cloud providers and broad model quality have not been qualified. Historical fixture evidence is in [generation verification](docs/testing/generation.md); current evidence is in the [PoC report](docs/testing/poc.md).

## Verification

```sh
node tools/cargo.mjs test --workspace
npm run test:bridge
npm run test:mcp
npm run test:sdk
npm run test:extraction
npm run test:generation
npm run test:e2e
npm run test:generation:e2e
npm run build
npm run lint
npm run test:native
npm run encoding:check
```

Browser E2E uses installed Microsoft Edge through Playwright. Generation tests use an explicitly synthetic loopback provider fixture, not a real model or paid API. The Windows GitHub Actions workflow defines the core, bridge, MCP, browser and native gates without model credentials. Its first hosted run could not start because of an account billing/spending-limit restriction; hosted CI is not yet verified. The optional Windows PowerPoint checks operate on disposable copies outside OneDrive:

```powershell
./tools/verify-demo.ps1
```

After `npm run tauri:build`, `npm run test:native:e2e` runs the generation workflow in a separately owned Windows desktop WebView using a disposable profile and loopback fixture. It closes the test application and removes its debugging profile afterward. This interactive desktop check is separate from hosted CI.

Optional network/desktop qualifications are deliberately separate: `npm run test:corpus` downloads the pinned public fixture set, `npm run demo:benchmark` fetches attributed World Bank data, `npm run model:qualify` uses the prepared local model, and `tools/verify-powerpoint.ps1` checks Office on disposable copies. None is presented as a credential-free hosted CI result.

The Open XML validator downloads pinned official NuGet packages from Microsoft's public .NET mirror into `.tools/openxml`. It performs schema validation only. The PowerPoint check confirms the generated sample opens, contains native objects and notes, and exports two preview images. It does not establish universal Office compatibility, interactive edit parity, or freedom from every repair/dialog condition.

See [docs/testing/first-slice.md](docs/testing/first-slice.md) for observed results and [docs/implementation.md](docs/implementation.md) for boundaries.

### Private GitHub Development

The source and tests are pushed to `main` in the Private repository. The [initial Actions run](https://github.com/murasamelabo/AISlide/actions/runs/34727915785) was blocked before any step ran: GitHub reported failed recent account payments or a spending limit requiring attention. No billing settings, spending limits, or repository visibility were changed. Local checks are the verified baseline for this checkpoint, not a substitute claim that hosted CI passed.

After the repository owner resolves the restriction in [GitHub billing settings](https://github.com/settings/billing), open **Actions > Verify > Run workflow** on `main`, or run:

```sh
gh workflow run verify.yml --repo murasamelabo/AISlide --ref main
```

Review that run before treating a clean hosted Windows build as established. Do not make the repository public or add a self-hosted runner merely to work around the restriction.

## Beyond This PoC

Full arbitrary-PPTX editing, slide-master/theme editing, mixed-run rich text, animations, chart types beyond bar/column/line, exact Office typography, automatic scanned-PDF rendering/OCR, persistent project history/autosave, credential-vault UI, broad cloud-model qualification, signed installers and cross-platform distribution remain outside this verified checkpoint. Imported SmartArt/OLE/media and unknown extensions are preserved, not executed or fully rendered. ZIP64, encrypted presentations, legacy `.ppt`, non-UTF-8 edited XML and signed-package edits are rejected.

Synthetic data must not be treated as factual, template compilation must not be described as AI generation, and structural validation must not be described as Office visual parity.

## License

The project license has **not** been selected. There is no public-release authorization or redistribution license for the project yet. Third-party dependencies retain their own licenses. No Office fonts, external slide engine source, or third-party presentations are bundled.