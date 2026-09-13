# AISlide Studio

A local-first slide editor with an independently implemented Rust PresentationML core, a Tauri desktop application, and headless CLI/MCP access.

**Status: working editor and model-generation integration, not the completed PoC.** The included twelve-slide example is deterministic and explicitly synthetic. Optional OpenAI-compatible model generation uses a separately configured provider. Existing PPTX engine code is not used.

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

## Available Now

- Twelve-slide synthetic data-report example, with five layout recipes and Japanese/English text.
- True editable PowerPoint text, rectangles, native tables, minimal master/layout/theme parts, and speaker notes.
- Slide navigation, duplication/reordering, object selection, dragging/arrow-key movement, position/size/text/color edits, and Undo/Redo.
- Structured report JSON import and compilation. Recompilation replaces the scene; Undo restores the previous scene.
- Optional model-generated reports through Studio, the shared JSON API, and MCP, with draft review, explicit application, cancellation, and unverified-content labels.
- Scene JSON checkpoints for saving and reopening manual edits. No autosave or complete project-sidecar model yet.
- PPTX inspector for simple top-level text runs, with expected-text conflict checking and patch export to a different file.
- Exact original bytes on a no-op round trip. A supported patch changes its slide part while preserving untouched entry payloads; edited ZIP containers are not claimed byte-identical.
- Strict schema/geometry validation, bounded archives, unsafe-name/duplicate-entry rejection, and no fetching or execution of imported content.

The editor's **Report data** dialog accepts the format returned by the CLI's `sample` command. It does not accept arbitrary HTML/CSS or model-generated source code. **Save scene JSON / Open scene JSON** preserves manual layout edits. **Inspect PPTX** is a text inspector, not a full visual import editor.

## Headless CLI

The executable is `target/debug/aislide.exe` on Windows, or `target/debug/aislide` on other systems.

```sh
aislide sample
aislide generate input.json new-report.pptx
aislide inspect existing.pptx
aislide request
```

`request` reads one JSON document from stdin and writes one JSON result to stdout. Errors go to stderr with a nonzero exit status. `generate` uses atomic, create-new publication and never overwrites the input or an existing destination.

Operations: `sample`, `compile` (`report`), `provider_status`, `generate` (`input`), `export` (`deck`), `validate` (`deck`), `inspect` (`base64`), `roundtrip` (`base64`), and `patch_text` (`base64`, `part`, `shape_id`, `run_index`, `expected`, `text`). The JSON request limit is 4 MiB, including base64. The GUI limits imported PPTX files to 2.8 MiB to leave room for the envelope. The CLI subcommand `aislide generate input.json output.pptx` remains deterministic compilation; model inference uses the JSON `generate` operation.

```sh
npm run demo
```

This produces a uniquely named PPTX in `.artifacts/` without overwriting previous results.

## MCP

```sh
node tools/mcp.mjs --output-dir ./output
```

The official TypeScript SDK exposes stdio tools: `sample_report`, `compile_report`, `provider_status`, `generate_report`, `get_deck`, `update_text`, `undo`, `validate_deck`, `export_pptx`, and `close_deck`. The GUI need not be running. `aislide://report/example` supplies the structured example. VS Code configuration is in [.vscode/mcp.json](.vscode/mcp.json).

The MCP client can supply structured content to `compile_report`, or request inference from the operator-configured provider through `generate_report`. Generation creates a new in-memory deck and never automatically exports it. Up to eight decks and ten text-edit undo snapshots per deck are retained in memory. Export is disabled without `--output-dir`, accepts a plain filename only, and refuses existing files. No generic filesystem, shell, or network tool is exposed.

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
| `AISLIDE_AI_JSON_MODE` | `1` by default; set `0` only for providers without `response_format` support |
| `AISLIDE_AI_TIMEOUT_SECONDS` | 1-300 seconds; default 120 |

Remote transmission additionally requires per-request `allow_remote: true`, or the Studio consent checkbox. Brief and source text are sent to the displayed endpoint. Endpoint and credential overrides in UI, model output, or MCP arguments are rejected. Redirects, HTTP retries, system proxies, and external relationship fetching are disabled.

Studio leaves the existing deck unchanged until **Apply draft**. Cancellation, malformed output, or provider failure does not replace it. Model output passes the same report and geometry validation as deterministic input, but facts and citations remain **unverified**. There is no silent sample/template fallback.

The `provider_status` operation reports configuration only, not connectivity or credential validity. For MCP generation, configure the client request timeout longer than the provider budget; the CLI bridge permits up to 310 seconds. See [generation verification](docs/testing/generation.md) for fixture-based evidence and remaining model qualification work.

## Verification

```sh
node tools/cargo.mjs test --workspace
npm run test:bridge
npm run test:mcp
npm run test:generation
npm run test:e2e
npm run test:generation:e2e
npm run build
npm run lint
npm run test:native
npm run encoding:check
```

Browser E2E uses installed Microsoft Edge through Playwright. Generation tests use an explicitly synthetic loopback provider fixture, not a real model or paid API. The Windows GitHub Actions workflow runs the core, bridge, MCP, browser and native gates without model credentials. The optional Windows PowerPoint checks operate on disposable copies outside OneDrive:

```powershell
./tools/verify-demo.ps1
```

After `npm run tauri:build`, `npm run test:native:e2e` runs the generation workflow in a separately owned Windows desktop WebView using a disposable profile and loopback fixture. It closes the test application and removes its debugging profile afterward. This interactive desktop check is separate from hosted CI.

The Open XML validator downloads pinned official NuGet packages from Microsoft's public .NET mirror into `.tools/openxml`. It performs schema validation only. The PowerPoint check confirms the generated sample opens, contains native objects and notes, and exports two preview images. It does not establish universal Office compatibility, interactive edit parity, or freedom from every repair/dialog condition.

See [docs/testing/first-slice.md](docs/testing/first-slice.md) for observed results and [docs/implementation.md](docs/implementation.md) for boundaries.

## Remaining Work

Qualification against real local/cloud models; evidence extraction from CSV/XLSX/PDF/images; native charts, pictures, connectors/groups; robust font measurement and text-fit validation; full slide/master/theme editing; public compatibility corpus; richer SDK and transaction APIs; source-bound sidecar history; signed installers. Imported SmartArt/OLE/media and unknown extensions remain opaque and are never executed. ZIP64, encrypted presentations, legacy `.ppt`, non-UTF-8 edited XML, and signed-package edits are currently rejected.

Synthetic data must not be treated as factual, template compilation must not be described as AI generation, and structural validation must not be described as Office visual parity.

## License

The project license has **not** been selected. There is no public-release authorization or redistribution license for the project yet. Third-party dependencies retain their own licenses. No Office fonts, external slide engine source, or third-party presentations are bundled.