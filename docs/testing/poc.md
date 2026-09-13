# Research PoC Acceptance

Date: 2026-09-13. Scope: Windows-first, independently implemented editable PPTX generation, bounded package-preserving import, and GUI-free source/model workflows. This is research acceptance of the [documented subset](../support-matrix.md), not a claim of arbitrary PowerPoint compatibility or production readiness.

## Proof 1: Native Editable Generation

The public-data benchmark uses World Bank World Development Indicators, Japan population, `SP.POP.TOTL`, 2020-2023. The raw API response, derived CSV, upstream/source SHA-256 hashes, attribution and transformation description are retained alongside its checkpoint. Values are not substituted with synthetic data:

| Year | Population |
| --- | ---: |
| 2020 | 126261000 |
| 2021 | 125681593 |
| 2022 | 125124989 |
| 2023 | 124516650 |

Source: <https://api.worldbank.org/v2/country/JPN/indicator/SP.POP.TOTL?date=2020:2023&format=json>. API metadata reported last updated 2026-07-13. Reuse terms: World Bank CC BY 4.0 with additional terms, <https://datacatalog.worldbank.org/public-licenses#cc-by>. The reproducible script sorts the four records by year and copies date/value into CSV without changing values. It records hashes on each run rather than assuming a live API never changes.

Observed: twelve Japanese/English slides; native text, one data table, three charts, one repository-owned PNG, one process group, three attached connectors and twelve populated notes. The Open XML SDK validator passed. Microsoft 365 PowerPoint opened the disposable copy, recognized those native object counts, allowed editing all three embedded chart workbooks, and exported all twelve slide images. The original PPTX hash stayed unchanged. The original measured benchmark contained 78 text frames/table cells with no overflow/missing-glyph errors. Later source binding coverage includes displayed table cells as well as chart values/categories.

Command: `npm run demo:benchmark`, followed by the optional Office scripts with expected counts. The final evidence directory is `.artifacts/benchmark-1789275680861`, with 32 cell/category bindings and zero measured layout errors. Its PPTX SHA-256 is `bacdb3e042fa9c408f9206b1a3ca37e953fddc22df0364559300ed6c259b5e7b`. Final Office verification again opened all twelve slides, recognized three charts/one table/one picture/one group/three connectors, edited the three embedded workbooks and rendered every slide without changing the source file. Screenshots and public data are not committed. The application icon is labeled as an asset, not population evidence.

## Proof 2: Package Preservation

The corpus is pinned in [corpus.json](corpus.json) to Apache POI revision `72f638bd6429f691dc752d54095da28621230717`. Only public test presentations are downloaded, not another PPTX engine implementation. The repository's license and notice are retained with cached files. No third-party presentation binary is redistributed; per-file redistribution rights still require review before any publication.

Observed with `npm run test:corpus`:

- 24 of 24 no-op outputs were byte-identical to their original archive.
- 12 samples had eligible simple top-level text; all 12 supported patches changed only the selected slide part, with every other part's byte count and SHA-256 unchanged.
- The other 12 samples had no eligible simple text, not a successful edit test.
- All 24 returned bounded approximate previews with limitations. A preview can omit all unsupported visuals, so this count is not a visual compatibility score.
- Adversarial tests cover unsafe names, duplicate ZIP entries, decompression bounds, DTD/external slide rejection, signed-edit rejection, invalid crops/images, excessive group depth and dangling connector targets.

Per-file URLs, byte lengths, hashes, supported-edit results and preview warnings are generated under `.artifacts/corpus/<revision>/results.json`. Public corpus Office rendering has not been established. Supported visual import editing remains limited to explicitly listed top-level text/geometry fields. Unknown parts are kept opaque rather than reconstructed.

## Proof 3: Headless Automation And A Real Model

The reusable client and official MCP client tests execute CSV intake, explicit field mapping, a twelve-slide report, revision-checked transactions, stale-edit rejection, Undo/Redo, native diagram insertion, layout/source validation, create-new PPTX/checkpoint publication, and exact project reopening with the GUI closed. Cancellation and an intentionally late successful transport reply are tested without committing state.

Real model qualification was separate from deterministic fixtures:

| Property | Observed |
| --- | --- |
| Model | Official Qwen2.5-1.5B-Instruct GGUF Q4_K_M |
| Model revision | `91cad51170dc346986eccefdc2dd33a9da36ead9` |
| Model SHA-256 | `6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e` |
| Runtime | Official llama.cpp b10809, Windows ARM64 CPU |
| Runtime ZIP SHA-256 | `c1058fe5764a687275c8d20d6bbc1454e787cdbb8ebb8c37a2f959f2b144dc77` |
| Transport | OpenAI-compatible HTTP on an owned ephemeral loopback port |
| Final workflow | MCP generation, edit, Undo and project publication |
| Input | Explicitly synthetic quarterly values 10, 12, 11, 14 and an approved twelve-slide outline |
| Final result | Twelve slides, exact chart categories/values, zero measured text errors |
| Final generation time | 17993 ms, one attempt |
| Artifacts | `.artifacts/real-model-1789272537977` |

Earlier real-model runs exposed invalid JSON, weak unconstrained headings, duplicate chart choices, and a long KPI value overflowing its frame. These were not accepted as successful results or replaced by templates. Typed JSON Schema, layout-specific required fields, approved outline verification, and measured KPI sizing addressed the tested cases. A llama.cpp schema-conversion incompatibility was isolated to `items` taking priority over `prefixItems`; a request-level regression now covers that case.

The final controlled MCP result also passed Open XML validation and PowerPoint verification: twelve slides, one chart, two process groups, two connectors, twelve notes, an editable embedded workbook, and all twelve slide images. The saved MCP PPTX SHA-256 is `6846122bbeeffa1e576b363dcae165879514a828e850ed3ff832e2d16c324ac1`. An earlier unconstrained twelve-slide output also opened successfully but had weaker headings. Neither run proves factual correctness or general writing quality. The tiny model benefits from supplied outlines and is not claimed to be a production-quality author on arbitrary briefs.

The model runtime and weights were downloaded into `.tools` with length/SHA-256 verification and license records. No cloud API key, paid inference, global environment setting or system service was used. Each qualifier restores its environment and stops its owned model process.

## Extraction And Layout

PDF text extraction uses limited streams and page provenance. PNG/JPEG image OCR was exercised against the installed Windows OCR engine using a known bitmap; expected text and word bounding regions were returned locally and labeled unverified. An English OCR pack was absent, so the test used an installed language successfully. Scanned PDF rasterization is not implemented; supply a page image explicitly for OCR.

Cosmic Text measured the initial twelve-slide synthetic report across 130 frames/cells with no text errors; it reported Segoe UI and Yu Gothic on this machine. Aptos fell back during measurement. That fallback is visible and is not proof of Office equivalence. A maximum 32-row data case initially overflowed 70 cells; density-dependent table text sizing and shared cell padding fixed the regression. Long KPI strings now use measured font sizing. New project export blocks measured text errors, while raw structural export remains explicitly low-level.

## Review And Recovery

- Project publication initially used an identity check followed by deleting a public path during rollback. Review identified a check/delete race. Both implementations now preflight existing paths and report racing partial publication without deleting public paths; an injected concurrent writer test covers this.
- A late transport-success-after-abort test confirms the SDK does not commit cancelled state. Studio caches immutable initial snapshots, not a mutable session shared across remounts.
- Integer-valued float normalization fixed Rust/JavaScript differences such as `-2.0` versus `-2` in hashes and source comparisons. Hashes remain unkeyed integrity metadata, not authentication.
- Final screenshot review found a bar-preview axis beginning at the minimum value, hiding the shortest positive bar. Regression tests reproduced it, and both native PPTX and Studio bar axes now include zero for one-sided data. Line-chart ranges remain automatic.
- Import origin and all unknown package payloads are retained. Inverse receipts pass normal patch validation. Their possession confers no extra authority.

Supplementary reviews were limited to supplied excerpts/contracts; no full independent repository security audit or test-coverage percentage is claimed. Native blocking extraction is bounded but not hard-cancellable/process-isolated. Public output pairs are not crash-atomic. Native save core and IPC integration are tested/compiled; automated interaction with every Windows save-dialog state is not established.

## Final Gates

All required local gates passed on the final source. There are **106 distinct automated tests**, excluding the separate public-corpus and real-model/Office qualifications:

| Gate | Result |
| --- | --- |
| Rust workspace | 70 passed |
| Node bridge/publication/SDK/MCP/generation/actual OCR | 18 passed, none skipped |
| Studio browser workflows | 11 passed |
| Model-generation browser workflows | 4 passed |
| Native request management | 2 passed |
| Rebuilt Tauri WebView intake/chart/measurement/generation/cancellation | 1 passed |
| TypeScript/Vite build and Studio lint | Passed |
| Embedded native application build | Passed, x64 GNU on Windows ARM64 |
| Source/config encoding | 113 checked, zero mismatches |
| Pinned public corpus | 24 byte-identical no-ops; 12 eligible patches preserved all non-target parts |
| Real local model through MCP | 12 slides, one attempt, exact chart values, zero measured text errors |
| Public-data and model PPTX Office checks | Native objects, chart data editing and all slides rendered; original hashes unchanged |

The initial frontend bundle is approximately 245 kB before gzip; the chart renderer is a separate on-demand chunk. Desktop/mobile screenshots and axe checks cover the editor, generation dialog and source controls. Native save-dialog IPC compiled successfully and core publication is tested; every interactive dialog branch was not automated.

Historical results before the PoC continuation remain in [generation.md](generation.md) and [first-slice.md](first-slice.md), and must not be confused with the expanded surface. The three research proof workflows pass for the documented subset. Full arbitrary-import editing, production hardening and distribution remain follow-on work, not hidden completed requirements.

Hosted CI remains an external prerequisite: the Private repository's [initial run](https://github.com/murasamelabo/AISlide/actions/runs/34727915785) was denied before any job step because of account billing/spending limits. Billing, repository visibility and runner ownership were not changed. Local verification is the accepted gate until the owner resolves that restriction and dispatches Verify.

## Assessment

| Axis | Score | Evidence and residual gap |
| --- | ---: | --- |
| Accuracy | 4/5 | Measured tests and Office object evidence; arbitrary Office parity remains unverified |
| Completeness | 4/5 | All three bounded research workflows implemented; full master/theme/rich-text import is outside the verified subset |
| Clarity | 4/5 | Explicit API/support matrix and separated synthetic/public/model evidence; some low-level operations still require JSON |
| Actionability | 4/5 | Reproducible SDK, source intake, local model and corpus commands; hosted CI needs owner billing action |
| Conciseness | 4/5 | Primary README and focused references; the full verification surface necessarily spans multiple suites |

Overall: 4.0/5 for the bounded research deliverable. The highest-impact follow-on work is broader native import editing, cloud-model qualification and signed distribution, not re-labeling approximate previews as full fidelity. A reasonable user should distinguish these research proofs from a finished commercial editor.