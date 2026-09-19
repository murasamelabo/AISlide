# G38 Bounded Practical Capacity

The current default is `large`: 256 slides, 32 MiB complete document, 96 MiB JSON wire and 16 MiB archive. `standard` and `legacy` retain their numeric limits. See [Phase 5's current contract and verification](phase5-recovery-capacity.md) for request-local propagation, strict UTF-8 history, v2 recovery and full CJK fonts. All limits apply together: a raw PPTX below the file limit can still exceed the document budget after its immutable archive origin, decoded scene, repeated image strings, sources and metadata are combined.

The sections below record the **historical 2026-09-17 G38 standard-profile slice**, including its former default, history and recovery contracts. They are not the current defaults; Phase 5 supersedes those specific limits and behaviors without rewriting the historical test results.

## Profiles

Core owns the definitions in `crates/aislide-core/src/limits.rs`. SDK/Studio/Node/MCP use the fixed SDK mirror, compared field-for-field against the live core by `tools/capacity.test.mjs`. Read them with `{"op":"capacity_profiles"}`, `client.capacityProfiles()`, or MCP `capacity_profiles`.

Existing request envelopes continue to work and select `standard`. The low-level envelope also accepts `"capacity_profile":"legacy"` or `"capacity_profile":"standard"`. Unknown names, null and custom limit objects reject. The profile is request-local, not a Deck field, persisted preference, trusted-input switch or global override. Legacy checks both input and output; an operation cannot return an oversized legacy scene just because its input was small. Library document/model APIs use the fixed standard bounds.

| Budget | Legacy | Standard |
| --- | ---: | ---: |
| Editable slides / deterministic report sections | 32 | 128 |
| Elements per slide, including group descendants | 256 | 256 |
| Total elements, including master/layout objects | 2048 | 8192 |
| Group nesting | 8 | 8 |
| Revisioned document content, UTF-8 JSON | 2 MiB | 8 MiB |
| Core request / response, UTF-8 JSON | 4 MiB | 16 MiB |
| Binary archive protocol budget | 3 MiB minus 1024 bytes | 8 MiB |
| Scene image payload, repeated base64 plus SVG included | 3 MiB | 3 MiB |
| Unique raster/SVG-fallback resource keys | 128 | 128 |
| Unique raster work, sum of width x height x 4 | 64 MiB | 64 MiB |
| JSON value nodes / nesting | 250,000 / 64 | 250,000 / 64 |

The new allocation guards also apply to legacy; this is compatibility with its conservative size envelope, not permission to bypass new safeguards. Template objects count toward the scene totals. Per-object style, table, chart, source, SVG, font and native preservation constraints remain independently authoritative. In particular, model generation and guided authoring retain their separate 32-slide input limits.

## Safeguards And Allocation

- CLI input is byte-bounded, then visited using serde's streaming parser before building a JSON Value. Object/array amplification and deep nesting fail before typed deserialization. The shared protocol also preflights an already-parsed Value for native callers, including scene counts before typed conversion. Tauri still constructs its framework-level JSON Value before the command handler; this change does not claim a zero-allocation IPC parser.
- Document size checks use a bounded counting writer and borrowed content instead of cloning and materializing a full byte buffer just to reject it. Intermediate JSON Patch results remain bounded. Canonical hashing, native-origin verification and copy-through protections remain mandatory.
- Count and encoded resource budgets are checked across the complete scene before raster decoding. Only then are bounded raster headers inspected for the combined RGBA work estimate. This is a deterministic admission estimate, not measured process peak memory; it excludes renderer scratch space, SVG rasterization, codecs, JSON objects and repeated operation passes.
- Generated chart/media part counts are conservatively checked before workbook generation. The existing ZIP package limits remain 64 MiB compressed, 16 MiB per part, 128 MiB total actual expansion and 4096 entries. Remaining expansion is checked before reading another entry. ZIP64, duplicate entries, encryption, symlinks and unsafe paths remain rejected.
- Cancellation is checked before dispatch, during the Value walk every 1024 nodes, before a blocking worker starts, and before accepting its result. The Node child deadline stays 20 seconds (generation 310 seconds); cancellation terminates only its own CLI process. Native blocking work is not hard-preemptible: cancellation discards the result when the bounded worker completes, retaining the busy guard until then.

## Reduced Duplication

Fresh PPTX export shares immutable media across slides, groups, masters and layouts using MIME plus decoded SHA-256 raster and optional SVG fingerprints. A matching key must also match the stored bytes. Shape identity, position, crop, alt text and relationship IDs stay independent. Mutable chart XML and embedded workbooks are never deduplicated. Native resource edits create new parts; changing one reference cannot alter the other picture. Existing imported packages are not rewritten simply to deduplicate them, and no-op export remains byte-preserving.

Single-PPTX export no longer constructs and then discards an entire checkpoint or decodes its own base64 before attaching provenance. Large inverse-patch fallback replaces only changed top-level content members, not unchanged origin/source data. Normal small text/title edit receipts remain small. Full snapshot document/history architecture is retained, not replaced by handles or external resource paths.

MCP JSON results above 64 KiB are returned once in `content[0].text`; `structuredContent` is omitted for these large results. Small results retain both representations. Consumers must parse text when structured content is absent. The outer serialized MCP result is still capped at 16 MiB; JSON escaping and wrapper metadata consume that budget.

## Separate Limits Retained

- Source intake: 2 MiB; raster bytes: 1 MiB; dimensions: 4096 per axis; decoder allocation: 64 MiB. SVG source: 256 KiB and its existing graph/expansion limits.
- Sources: 8 per document; bindings: 4096; transaction operations: 128. Revision and hash preconditions are unchanged.
- Provenance XML retains 2 MiB and depth 32. Identity records permit at most 168 (128 slides + 8 masters + 32 layouts), 256 objects per record and 8192 objects total, checked on both writing and reading. This removes the old 72-record bottleneck without increasing the XML byte budget.
- SDK Undo/Redo retention remains 30 receipts and its existing 4 MiB string-length retention threshold, retaining at least one receipt. It is not a new strict UTF-8 heap budget.
- Static export retains at most 32 selected pages per call, 8192px edges, 32 MiB core output and a separate legacy-sized binary bundle envelope (less than 3 MiB). Pages may be selected from anywhere in the 128-slide deck. Core raster working limits still apply.
- Studio local recovery and MCP recovery JSON retain 2 MiB per snapshot; Studio retains five documents / 10 MiB total. Larger editable documents do not automatically become recoverable snapshots. Save and explicit capacity errors remain available.
- File pickers admit at most 8 MiB PPTX before reading/base64 allocation. No arbitrary file paths, persistent server handles, imported-file access, network fetches or protection changes were introduced.

## Verification

`node tools/cargo.mjs test -p aislide-core --test capacity --locked --offline -- --nocapture` covers profile compatibility, 64/128 slide admission, 129 rejection, aggregate elements, nesting, image payload/count/work, chart part preflight, document/JSON budgets, cancellation, real edit/Undo/PPTX reopen, actual media deduplication and independent native image edits. `node --test tools/capacity.test.mjs` covers the real CLI/SDK, MCP and built Studio; build the CLI and Studio first.

All fixtures are synthetic. The Studio test intercepts every request and serves only its own build files or routes JSON to the CLI over standard input/output; it starts no HTTP server. Source presentations, Office, external services and user browser sessions are not used.

The original RED rejected 64 slides with `expected 1-32 slides`. Subsequent RED tests reproduced unknown profile handling, typed deserialization before capacity checks, and four identical media parts instead of one. The resource slice passed 11 tests; its representative 64-slide fixture was 78,500 document bytes, 349,676 bytes after native reopen, with 223,548 base64 export characters. The reported 4,615 ms was one debug-build test observation, not a performance guarantee or a controlled before/after comparison. Final integration measurements are printed as `G38_MEASURE`; no native heap/RSS peak has been measured.

### Measured Integration Run

Real debug CLI processes through the SDK, Windows, 2026-09-17. Each slide has a short headline and body plus the normal report decoration; all content is synthetic. Times include process startup. These are single observations, not percentile benchmarks or regression thresholds.

| Measurement | 64 slides | 128 slides |
| --- | ---: | ---: |
| Document JSON bytes | 73,357 | 146,626 |
| Reopened document JSON bytes | 346,439 | 685,596 |
| Native PPTX bytes | 160,489 | 316,081 |
| Compile | 119 ms | 111 ms |
| Create document | 154 ms | 227 ms |
| Edit | 235 ms | 398 ms |
| Undo | 294 ms | 425 ms |
| Export | 1,506 ms | 2,526 ms |
| Reopen | 1,297 ms | 2,255 ms |
| Native text edit | 1,647 ms | 3,211 ms |

The final four integration tests passed in 44.88 seconds, including the actual MCP 64-to-128 replacement/129 rejection, moving to position 128, Undo and the built Studio's 64th-slide edit/Undo/byte-identical save. Desktop and mobile screenshots are in `.artifacts/g38/`. The Studio workflow passed in 12.87 seconds. No Office instance, real presentation, external service, installed-app replacement or protection settings were used. The unit fixture exceeding 2 MiB additionally proves the document budget increased; small 64/128-slide documents alone would not prove that byte-limit change.

The final focused run passed 15 capacity and 22 existing selection tests. The 128-slide native large-inverse fixture produced a 251,306-byte receipt versus its 691,263-byte original document; the unchanged origin contained 439,852 base64 characters and was not copied into the receipt. Undo restored the original content hash. This measures serialized receipt reduction, not total heap savings.

### Historical Slice Gate Accounting

The failures below describe the earlier G38 integration checkpoint, not the current combined implementation. The later clipboard schema fix and visual expectation update are covered by the [current editing verification](../planning/editing-expansion.md). Preserve these entries as the actual earlier run history rather than relabeling a failed run as green.

- `node tools/cargo.mjs test --workspace --locked --offline` was run, then repeated once after the obsolete 2048-element selection expectation was updated to assert acceptance there and rejection at the new total. The repeated run recorded 383 passed, one failed and two explicitly ignored, reaching the final visual suite and failing its obsolete two-identical-SVG-parts expectation. After updating that expectation and retaining active/external reference rejection, the affected visual suite passed 16 tests with one explicit fixture-output test ignored. The full workspace was not run a third time; do not label its recorded exit 101 as a single green full-suite run. Final capacity plus strict-protocol tests passed 20/20 after the last profile-admission change.
- Native Tauri unit tests passed 5/5, including the real guard's capacity and cancellation paths. Its pre-existing manifest lock mismatch required offline lock synchronization; no dependencies were added to manifests by G38. Installed WebView UI and packaging were not tested or replaced.
- Final existing Node regression: 34/35 passed. The remaining failure is `tools/mcp-poc.test.mjs` expanded-authoring clipboard input: `source_document` is returned by the current selection code but rejected by MCP's clipboard schema. That unrelated contract was not changed by G38. Dedicated G38 MCP tests passed.
- Final CLI build and `npm run build` passed. Lint passed with existing panel warnings; the existing large AssetPanel chunk warning remains. Final encoding check reported 352 files / zero mismatches; the separate G38 source check confirmed exactly one UTF-8 BOM, and JSON configuration remains BOM-free. Final `git diff --check` passed. No commit, push, installed-app update or protection change was performed.

Evidence logs: `.artifacts/g38-capacity-final.log`, `g38-workspace-final.log`, `g38-visual-final.log`, `g38-native-final.log`, `g38-node-final.log`, `g38-build-final.log`, and the RED/focused logs with the same prefix. Counts across repeated runs overlap and must not be added together as independent test cases.