# AISlide Studio

A local-first slide editor with an independently implemented Rust PresentationML core, a Tauri desktop application, and headless CLI/MCP access.

**Blank startup, icon categories and master presets.** Studio starts with one empty slide; **New report** explicitly opens the synthetic sample. The full Lucide catalog can be filtered by its 42 official categories together with keywords. **Edit masters and layouts > Browse presets** offers seven original styles with native editable layouts, theme colors, fonts, margins, gutters and content/part regions. See the [preset guide and samples](docs/testing/master-presets.md).

**Status: research PoC covering native editable generation, package-preserving import, source-bound transactions and headless automation.** A real local model, a public-data bilingual deck, and a 24-file public PPTX corpus have been exercised. This is not a full PowerPoint replacement or a production release. See the [acceptance report](docs/testing/poc.md) and [support matrix](docs/support-matrix.md).

**Current file workflow: one standard, unencrypted Open XML PPTX.** Use **Open PPTX** and **Save PPTX**. External `.aislide.json` and scene JSON files are no longer required by Studio. The current slide XML is authoritative; optional source/binding and part information is retained in a standard Custom XML part inside the PPTX. See [single-file operation](docs/testing/single-file.md).

**Limited-feature completion, 2026-09-19: implementation complete within the documented bounds; final product verification in progress.** Work began on 2026-09-18. The [completion plan](docs/planning/editing-completion.md) records the implemented features, remaining limits and final verification gates. The earlier 444 Rust / 66 Node / 175 Edge results belong to the [prior expansion checkpoint](docs/planning/editing-expansion.md), not this revision. The catalog has 24 native chart kinds and basic PNG/PDF coverage for all 24. Histogram Office fixtures retain the deliberate `val`-bin encoding exception: **two known SDK 3.5.1 schema errors per histogram chartEx resource**. These are fixture-specific checks, not full Office parity or a production-release qualification. See [chart qualification and history](docs/authoring/chart-ex.md).

**Parts library: 111 editable presets across 36 categories.** Enter labels, axes, values, relationships or schedules to create theme-linked native graphics. Three new frameless list compositions use aligned text and whitespace; the original 108 recipes remain available and unchanged. See [list selection guidance](docs/authoring/README.md#choose-a-list-composition), the [sample generator](tools/parts-demo.mjs) and the [parts guide](docs/testing/parts-library.md). These are deterministic design presets, not AI-generated artwork.

**Filled process diagrams and evidence-led MCP authoring.** Horizontal/vertical flows, trees and cycles now use stage panels, hierarchy blocks and segmented arrows with safe label areas. Four English best-practice profiles support consulting decisions, technical explanations, event talks and reports. MCP can retrieve guides, validate an evidence-linked outline and create a new editable presentation without inventing values or calling a model. The consulting guide retains 48 patterns with honest implementation status; dedicated summary/closing templates supplement the existing parts. See [guided authoring and complete input examples](docs/authoring/README.md).

**Architecture diagrams through Studio, SDK and MCP.** The **Architecture diagram** command opens a six-shape editor with boundaries, movement, resizing, alignment, grid, straight/right-angle routes and arrowheads. Graphs are native PPTX objects with internal metadata and Undo/Redo. See the [five-slide sample generator](tools/graphs-demo.mjs) and [graph/UI verification](docs/testing/graphs-and-dads.md).

**Icons inside architecture nodes.** Select a node and use **Choose icon** to add a library icon or supplied SVG/PNG/JPEG beside its label. Change/remove, movement, Undo and single-PPTX reopen share the graph workflow. Graph-specific image preparation caps the longest side at 256px; SVG becomes PNG. Use `node tools/graphs-demo.mjs --icons` for a reproducible synthetic example. See [node-icon behavior and verification](docs/testing/graphs-and-dads.md#node-icons).

**Studio design is inspired by the Digital Agency Design System (DADS).** Local Noto fonts, blue actions, neutral surfaces, larger controls and keyboard focus are adapted for the editor. Existing slide themes are unchanged. This is not a full DADS compliance or government endorsement claim.

**Workspace commands and asset insertion.** Right-click the canvas, slide thumbnails, layers, objects or graph nodes for relevant actions. Create a blank presentation, insert/duplicate/delete/reorder/rename slides, choose a save filename, and protect unsaved edits when opening or creating a file. **Insert icons** includes all **1,818 icons from Lucide React 1.43.0**, with full-catalog name search, 60-item pages, the existing English/Japanese search tags, and SVG/PNG/JPEG import. Figma **Copy as SVG** can be pasted on the canvas or into the SVG input; the safe inert path subset is retained as SVG with a bounded transparent PNG fallback, not converted into arbitrary editable shapes. Graph icons remain raster-only. See [current workspace operations](docs/support-matrix.md#workspace-operations).

The built-in example remains deterministic and explicitly synthetic. Model generation is a separate operation. Existing PPTX engine code is not used.

Development repository: [murasamelabo/AISlide](https://github.com/murasamelabo/AISlide), **Public**, as changed and confirmed by its owner on 2026-09-15. The project license has not been selected. Local input materials, task-specific examples and generated deliverables are not included in the source publication.

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

### Windows Setup

Windows setup uses Tauri's NSIS installer. It installs for the current user without requesting administrator access and registers **Start > AISlide > AISlide Studio**. The finish page also offers a desktop shortcut. Uninstalling removes the installer's shortcuts and app registration; it does not delete presentations or unrelated files in the installation folder. Save your work and close AISlide before installing, updating or uninstalling: running instances cause setup to stop rather than terminate the app.

```sh
npm run setup:build
```

The default is a release build. Output is under `apps/studio/src-tauri/target/<Rust-host>/release/bundle/nsis/`. For the locally verified, unsigned development installer, run `npm run setup:build -- --debug --no-sign`; its output uses `debug` instead. To regenerate an installer from that same setup build, use `npm run setup:bundle -- --debug --no-sign`. `--dry-run` prints the commands without building or installing.

Setup builds into a separate directory so the currently running development executable is not overwritten. `CARGO_BUILD_TARGET` and `CARGO_TARGET_DIR` overrides are rejected to avoid bundling a different output. A fresh build needs Rust/platform prerequisites; Tauri downloads and hash-checks NSIS tooling when absent. Setup uses the system WebView2 runtime and can download Microsoft's bootstrapper when needed. This does not enable login-time startup, pin to the taskbar, register PPTX associations, sign the app or publish a release.

`npm run test:setup` checks configuration and build routing without installing. The opt-in `npm run test:setup:installed` installs a randomly named test application, launches its Start Menu link and uninstalls it, using temporary files and its own shortcuts. See [Windows setup verification](docs/testing/windows-setup.md) for observed results and remaining release/signing qualifications.

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
- Editable rich text/paragraphs and speaker notes, 40 preset shapes, bounded adaptive-curve Boolean operations including fragment, formatted/merged table cells, 24 native chart kinds with embedded XLSX, PNG/JPEG and inert SVG pictures, groups and connectors. Static rendering includes eight default WordArt warps, six SVD-fitted trendline types, forecasts, error bars and bounded axis options; no full 3D, all Excel formats or arbitrary warp adjustments. Histogram retains the documented schema exception.
- Thirty-six part categories with three original layouts each plus three frameless list alternatives, structured data/JSON input, actual previews, metadata updates and Undo/Redo. Native charts retain raw workbook values; diagrams and map outlines remain ordinary grouped shapes, text and connectors.
- Direct canvas editing, multiple selection/move/resize, copy/paste, group/order/visibility/lock controls, alignment/guides, IME-aware drafts and Undo/Redo. Seven-kind object-style painting and a rendered-slide PNG eyedropper supplement text-format painting and supplied-dictionary proofing. Local-only model proofread/translation requires review before one revision-checked Apply/Undo; observed Japanese quality failures require human review. See [local AI editing](docs/authoring/local-ai.md).
- Native master/layout topology editing, owner-specific themes, POTX/THMX, custom canvas dimensions and master/layout/slide number/date/footer fields, including semantic field inheritance after reopen. Rich notes and notes/handout-master editing are bounded; auxiliary-master deletion, native notes-page resizing and handout imposition remain unsupported. Date/time evaluation covers datetime1..13 in en-US, with an explicit clock for datetime8..13. See [master fields and themes](docs/authoring/master-fields-themes.md).
- Safe SVG text and embedded PNG/JPEG plus a bounded EMF/WMF record subset, without external resource fetching. Optional pinned U2NetP CPU foreground segmentation produces alpha, but does not guarantee fine-hair quality; its 768 MiB admission budget is not an RSS hard cap, and in-flight CPU inference is non-preemptible. Weights are not bundled or published. See [local AI setup and rights](docs/authoring/local-ai.md).
- CSV, scalar-table JSON, bounded XLSX, UTF-8 text/Markdown, text PDF, and PNG/JPEG evidence intake. Explicit column/range mapping; optional Windows-local image OCR with word regions.
- Source hashes, exact cell/page locators, attribution and transformations. Changing a bound value invalidates its citation until corrected or explicitly detached.
- Shared revision/hash-checked atomic JSON Patch transactions, bounded Undo/Redo, and a portable JavaScript/TypeScript client.
- Model-generated reports with optional approved outlines, Rust-type-derived JSON Schema, at most one explicitly enabled validation repair, cancellation and review before application.
- Single-file PPTX opening and saving, with native text/style, theme, master/layout contents, table, picture, chart, group and connector reading. Sources and bindings travel inside the PPTX without a scene snapshot or external JSON. Legacy checkpoint APIs remain for compatibility only.
- Part-preserving edits of reopened PPTX files, with explicit rejection of unsupported complex formatting. Supported presentations allow slide and master/layout structural edits within capacity/preservation guards. Native sections/custom slide shows block slide structural editing, and deleting referenced content fails rather than breaking links.
- Static PNG/JPEG and outlined PDF with searchable/selectable English/Japanese Unicode text, real structure tags, alt text and declared table headers. Visible PDF text remains outlines, not editable text; no font programs are redistributed. Direct print uses a PNG-only same-document portal and the user-confirmed system dialog; native Print/Cancel was verified with Cancel only, without submitting a job. Opt-in v2 recovery remains off by default, with both Undo/Redo stacks capped at 30 receipts/4 MiB each and five entries/48 MiB each/100 MiB total. Seven-day expiry runs when the enabled store opens; originals and legacy copies remain untouched. See [static output and recovery](docs/support-matrix.md#static-output-and-recovery).
- Separate local modern GUID-author threads with three states, rich bodies/replies and guarded native edits, alongside legacy comments. Masked PII candidates are manual-review hints, not selectable redaction categories. Explicit table headers, effective-theme/alt checks and selective clean copies do not establish PDF/UA, WCAG, authenticated mentions or a blanket PII-removal guarantee. See [Phase 6 review](docs/authoring/review-phase6.md).
- Exact original bytes on a no-op round trip. A supported patch changes its slide part while preserving untouched entry payloads; edited ZIP containers are not claimed byte-identical.
- Installed-font measurement, overflow/missing-glyph reports and opt-in document-local static TTF/OTF full EOT v1 embedding with fsType checks (eight faces, 12 MiB/face, 24 MiB total, also constrained by the complete document). A real 5.77 MB static Japanese font is qualified in SDK/MCP/Studio; no OS installation, subsetting or Office typography certification. See [font limits](docs/authoring/fonts.md).
- Strict schemas, bounded archives/rasters/PDF streams, unsafe-name/duplicate-entry rejection, and no fetching or execution of imported content.

Use **Sources** for data mapping and source review, **Open PPTX** to open one presentation, and **Validate layout** for font diagnostics. **Save PPTX** creates a new PPTX with optional internal source metadata and does not overwrite existing files. **Inspect PPTX** remains the independent simple-text inspector. **Report data** accepts structured report JSON, never arbitrary HTML/CSS or executable model output.

Double-click text, a table or a group to edit on the canvas. Ctrl/Cmd+Enter or the check icon commits; Escape cancels outside IME composition. **Insert objects**, **Edit theme**, and **Edit masters and layouts** expose the authoring controls. Reopened PPTX files support bounded native edits while preserving the original package; unsupported changes fail before committing. See [single-file operation](docs/testing/single-file.md) and [authoring verification](docs/testing/authoring.md).

Open **Parts library**, choose a category/layout, enter data and select **Insert part**. Select the resulting group and use **Edit part data** to update it. **Save PPTX** retains the metadata internally. Manual or external edits that no longer match it disable semantic replacement without discarding the native objects. Insertion does not rearrange existing slide content; choose space on the slide before inserting.

Open **Architecture diagram**, choose an example or add shapes, and select **Insert graph**. Select the group and use **Edit graph** to reopen it. Canvas, Preview and JSON share Rust validation; invalid JSON retains its draft, and closing without insertion/update leaves the slide unchanged. Property forms provide alternatives to dragging and connecting. Graph-local Undo/Redo is separate from the single document revision created on application. Saving needs only the PPTX. draw.io XML interchange, self-loops, nested boundaries and automatic obstacle avoidance are not supported.

## Headless CLI

The executable is `target/debug/aislide.exe` on Windows, or `target/debug/aislide` on other systems.

```sh
aislide sample
aislide generate input.json new-report.pptx
aislide inspect existing.pptx
aislide request
```

`request` reads one JSON document from stdin and writes one JSON result to stdout. Errors go to stderr with a nonzero exit status. `generate` uses atomic, create-new publication and never overwrites the input or an existing destination.

The shared JSON API includes `sample`, `compile`, `provider_status`, `generate`, `ingest`, `data_report`, `create_picture`, `create_diagram`, `object_catalog`, `create_object`, `part_catalog`, `create_part`, `insert_part`, `update_part`, `design_defaults`, `update_design`, `apply_theme`, `assign_layout`, `new_document`, `transaction`, `undo_transaction`, `open_presentation`, `export_presentation`, `export_project`, `open_project`, `import_document`, `import_pptx`, `save_import`, `measure_layout`, `ocr_status`, `export`, `validate`, `inspect`, `roundtrip`, `package_manifest` and `patch_text`. The default `large` profile permits 256 slides, 8192 elements, 32 MiB complete document including immutable origin, 96 MiB JSON wire and 16 MiB archive. Explicit `standard` and `legacy` preserve their prior numeric budgets. Image, source, static-output and recovery budgets remain independent. See [API contracts](docs/api.md) and [Phase 5 practical capacity](docs/testing/phase5-recovery-capacity.md).

The CLI `generate` subcommand and low-level JSON `export` perform deterministic structural export. Use document/project operations for source-integrity and measured-layout gates. Model inference uses the JSON `generate` operation, not the similarly named CLI subcommand.

Workspace JSON operations also include `create_presentation`, `edit_slides`, `edit_elements` and `create_asset`. Their mutations use the same document revisions and transaction history.

```sh
npm run demo
```

This produces a uniquely named PPTX in `.artifacts/` without overwriting previous results.

## MCP

```sh
node tools/mcp.mjs --output-dir ./output
```

The official MCP SDK exposes stdio tools for model generation, source ingestion/mapping, complete document reads, atomic edits, Undo/Redo, object/part insertion, part metadata updates, master/theme editing, layout assignment, layout measurement and single-PPTX export/reopen. The previous tools such as `compile_report`, `update_text` and legacy project APIs remain available. The GUI need not be running. `aislide://report/example` supplies the structured example. VS Code configuration is in [.vscode/mcp.json](.vscode/mcp.json).

Generation creates a new in-memory document and never automatically exports it. Up to eight documents and eight source handles are retained; each document's Undo/Redo history has count and memory caps. Export is disabled without `--output-dir`, accepts a plain filename only, and refuses existing files. A project pair is published per file, not as an atomic filesystem transaction. Concurrent collisions or process interruption can leave a partial pair, which must be checked before use; recovery never deletes published paths. No generic filesystem, shell, or network tool is exposed.

Graph MCP tools are `graph_catalog`, `create_graph`, `transform_graph`, `get_graph`, `add_graph`, `update_graph` and `apply_graph`. Mutations require the current `expected_revision`. `node tools/graphs-demo.mjs` exercises creation, export, reopen, movement and byte-identical Undo through the official MCP SDK in a new artifact directory. See [the graph API](docs/api.md#architecture-graphs).

Workspace MCP tools are `create_presentation`, `edit_slides`, `edit_elements`, `create_asset` and `add_asset`. They share revision checks, input limits, history and the existing output-directory restrictions. SVG is parsed/rasterized locally with `resvg`; bounded safe text and embedded PNG/JPEG are supported. Scripts, external resources and unsupported effects are rejected, never fetched or executed.

`node tools/workspace-demo.mjs` creates a synthetic five-slide qualification in a new `.artifacts/` directory, including SVG icons, independent copied chart workbooks, native slide insertion/deletion/reordering and standalone reopen checks.

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

**2026-09-19 final gates are pending.** Focused feature checks are recorded in the linked contracts; they do not replace the final frozen-product core, Node, UI, native, Office and Windows-install checks. Final counts, artifact hashes, the regular desktop update and the normal commit/push to the existing Public repository will be recorded in the [completion plan](docs/planning/editing-completion.md) after verification. No current all-green result or published revision is asserted here.

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

After `npm run tauri:build`, `npm run test:native:e2e` runs direct editing, IME, object insertion, master/theme/layout editing, Undo and the generation workflow in a separately owned Windows desktop WebView using a disposable profile and loopback fixture. It closes the test application and removes its debugging profile afterward. This interactive desktop check is separate from hosted CI.

Optional network/desktop qualifications are deliberately separate: `npm run test:corpus` downloads the pinned public fixture set, `npm run demo:benchmark` fetches attributed World Bank data, `npm run model:qualify` uses the prepared local model, and `tools/verify-powerpoint.ps1` checks Office on disposable copies. None is presented as a credential-free hosted CI result.

The Open XML validator downloads pinned official NuGet packages from Microsoft's public .NET mirror into `.tools/openxml`. It performs schema validation only. The PowerPoint check confirms the generated sample opens, contains native objects and notes, and exports two preview images. It does not establish universal Office compatibility, interactive edit parity, or freedom from every repair/dialog condition.

See [docs/testing/first-slice.md](docs/testing/first-slice.md) for observed results and [docs/implementation.md](docs/implementation.md) for boundaries.

### GitHub Development

The owner authorized publishing the verified source to the existing Public repository on 2026-09-15. This checkpoint includes the dependent authoring, single-PPTX, parts, graph and workspace improvements. Source publication excludes local `deliverables/`, task-specific `examples/`, inputs, credentials, downloaded toolchains and build outputs. Reproducible synthetic examples live in `tools/*-demo.mjs`.

The original PoC baseline was pushed to `main` at `33243bf87381226285b99c8897ee67af9e406df1` while the repository was Private. The [initial Actions run](https://github.com/murasamelabo/AISlide/actions/runs/34727915785) was blocked before any step ran: GitHub reported failed recent account payments or a spending limit requiring attention. That historical failure is not a result for the current revision. No billing settings, spending limits or repository visibility are changed by this publication. See the latest [workspace verification](docs/testing/workspace-ux.md) and the actual Actions run for the pushed commit.

After the repository owner resolves the restriction in [GitHub billing settings](https://github.com/settings/billing), open **Actions > Verify > Run workflow** on `main`, or run:

```sh
gh workflow run verify.yml --repo murasamelabo/AISlide --ref main
```

Review that run before treating a clean hosted Windows build as established. Do not make the repository public or add a self-hosted runner merely to work around the restriction.

## Beyond This PoC

Full arbitrary-PPTX editing, unrestricted imported master/theme rewriting, exact Office typography, presentation/animation/video playback, encryption and cloud coauthoring remain outside this expansion. The 24-kind chart catalog does not cover every Office chart family or formatting combination; histogram retains the explicit schema exception. Automatic scanned-PDF rendering/OCR, in-place source AutoSave, credential-vault UI, broad cloud-model qualification, signed installers and cross-platform distribution remain unqualified or unsupported. Recovery persist/restart/restore with both histories passed in two distinct native WebView profiles before the latest storage-security repair; the final post-repair native/install gate remains pending. Imported SmartArt/OLE/media and unknown extensions are preserved, not executed or fully rendered. ZIP64, encrypted presentations, legacy `.ppt`, non-UTF-8 edited XML and signed-package edits are rejected; protected originals remain untouched. Parts have bounded data sizes; maps have generalized coastlines rather than country borders/geocoding, and nominal Venn layouts do not calculate area-accurate intersections. See the [current remaining limits and verification scope](docs/planning/editing-completion.md).

Synthetic data must not be treated as factual, template compilation must not be described as AI generation, and structural validation must not be described as Office visual parity.

## Design Reference

Source: [Digital Agency Design System website](https://design.digital.go.jp/dads/), adapted for AISlide. デジタル庁デザインシステムウェブサイトを参考に、AISlide向けに編集・加工。Noto Sans JP and Noto Sans Mono retain SIL Open Font License 1.1. Details and deviations are in the [graph/UI report](docs/testing/graphs-and-dads.md#design-reference).

## License

The project license has **not** been selected. Public source visibility does not establish a general redistribution license for the project. Third-party dependencies retain their own licenses. No Office fonts, external slide engine source, or third-party presentations are bundled. The geographic parts include public-domain Natural Earth land outlines; origin, terms and the exact asset hash are recorded in the [parts guide](docs/testing/parts-library.md#asset-provenance).