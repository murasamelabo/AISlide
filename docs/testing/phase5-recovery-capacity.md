# Phase 5: Capacity, Session Recovery And CJK Fonts

Date: 2026-09-19. Scope: G37/G38 and the G28 capacity extension. Existing PDF, notes, visual tools, generation and local-AI inference logic are outside this change. No Office automation, user installation, Git commit/push or user-profile deletion is performed here.

## 1. Fixed Capacity Profiles

| Profile | Slides | Complete Document | JSON Request/Response | Raw Archive |
| --- | ---: | ---: | ---: | ---: |
| `legacy` | 32 | 2 MiB | 4 MiB | 3 MiB minus 1024 bytes |
| `standard` | 128 | 8 MiB | 16 MiB | 8 MiB |
| `large` (default) | 256 | 32 MiB | 96 MiB | 16 MiB |

Complete document means UTF-8 JSON including identity, revision, hash, scene, sources, fonts and the immutable original archive's base64. All limits apply together. A raw file below 16 MiB can still fail after decoding its scene and retaining its original. No truncation or implicit profile upgrade occurs.

The request-local profile is carried into document sealing, verification, every intermediate transaction state, Undo and export. It is not part of the presentation scene or content hash, and no global mutable validation context exists. SDK sessions retain their profile across operations; `setCapacityProfile()` verifies the current document and both retained histories before changing it, and late cancellation preserves the old selection. Studio exposes the fixed selector under Document setup and propagates it through new/open/template/recovery/native-save paths. Lowering can reject because an earlier retained history state is too large, even if the current scene is small.

Scene ceilings remain 256 elements per slide, 8192 total (2048 legacy), group depth 8, 128 unique images, 3 MiB repeated image payload, 64 MiB estimated unique raster work, 250,000 JSON nodes and depth 64. Provenance accepts 296 identities (256 slides + 8 masters + 32 layouts); its 2 MiB XML, 256 objects/identity and 8192 total object bounds remain unchanged. Package parser ceilings remain 64 MiB compressed, 16 MiB/part, 128 MiB expanded and 4096 parts; protocol admission is the stricter 16 MiB archive ceiling. ZIP64, unsafe paths and active/external content remain rejected or inert as before.

Static export still accepts at most 32 selected pages and its separate existing output budget. Guided/model-generation input limits are unchanged. Large requests over 4 MiB and recovery verification have a finite 120-second Node deadline; ordinary requests remain 20 seconds and existing AI deadlines are unchanged. Vite uses the same classification. Native workers check cancellation before publication but are not hard-preemptible while Rust/font/ZIP computation is running. These budgets are not process-RSS guarantees.

## 2. Verified Session History

`SessionRecovery` has `format: "aislide.session"`, `version: 1`, `capacity_profile`, the complete current `document`, `past`, `future`, and nullable `history_boundary`. The envelope format version is independent of storage policy v2.

Core `verify_session_recovery` verifies the document and traverses each receipt stack independently, newest first, against disposable states using the real Undo/transaction implementation. It checks document IDs, content hashes, legal receipt paths and immutable origin. Receipt paths cannot address origin, identity or a whole-document replacement. Verification does not consume real revisions or modify the current session. Hashes are unkeyed consistency checks, not authenticity or authorization.

Each direction is strictly bounded to 30 receipts and 4 MiB UTF-8 JSON. If the newest receipt alone exceeds that limit, its entire direction becomes unavailable and `history_boundary: "history_limit"` is shown in Studio and available to SDK/MCP. No unreachable older receipts are retained behind an oversized missing link. Subsequent small edits remain undoable up to the boundary.

SDK: `session.recoveryEnvelope`, `client.verifySessionRecovery(envelope)`, `client.recoverSession(envelope)`, `DocumentSession.recover(transport, envelope)`, `session.capacityProfile`, `session.setCapacityProfile(profile)` and `session.historyBoundary`. Public constructors do not accept private history. Legacy `verifyRecovery`/`recoverPresentation` retain their document-only behavior and empty history.

MCP: `capacity_profiles`, `set_capacity_profile`, `get_session_recovery`, `recover_session`, plus optional `capacity_profile` on relevant create/open/legacy-recovery tools. These exchange caller-supplied JSON only. There is no generic recovery-filesystem tool. Font schemas are independent of unchanged 1 MiB image schemas.

## 3. Opt-In Storage Policy V2

Recovery is OFF until explicit v2 consent. The UI warns that plaintext original bytes, sensitive sources and editing history persist. It stores committed states, not unapplied drafts, and never writes into the source PPTX.

- At most 5 entries, 48 MiB each and 100 MiB aggregate committed payload. Oldest v2 entries are evicted only under the approved v2 policy. Document/history sub-limits still apply.
- Seven days from the last successful checkpoint. Expiry occurs only when the enabled store is opened; nothing runs while the app is closed. Disabling retains existing copies and prevents further writes. Explicit deletion remains available while disabled.
- Web: separate `aislide.recovery.v2` IndexedDB with atomic state/slot transactions. Legacy `aislide.recovery.v1` is not migrated, expired or deleted automatically. Its separate panel allows deliberate restore/discard; legacy consent never enables v2.
- Desktop: fixed Tauri application-local-data `recovery-v2` directory, independent of temporary WebView profiles. A dedicated worker accepts only state/transition/load requests; the UI supplies no directory/path. Ordinary files, no reparse points, process file lock, generation CAS, content-addressed slots, exclusive temporary files and atomic private-index replacement guard publication. Internal stale slots may be cleaned after committed policy transitions; no original presentation or v1 store is addressed.
- Generation advances on successful changes, including deletion; consent changes advance the consent epoch. Stale cross-window saves/deletes fail without replacing a newer checkpoint or resurrecting a deleted one. Store actions serialize locally; separate stores still require CAS.
- Cancel/dispose abort pending IndexedDB transactions and native requests, suppress late callbacks, and check cancellation before native slot/index publication. A commit linearized before cancellation is already completed, not rolled back. Disabling waits for cancellation settlement before changing consent.
- Failures are visible and retain the last good committed checkpoint. AppData/browser storage is not encrypted by AISlide; device permissions, backup and eviction apply. Native staging can temporarily require up to one additional entry's disk space; the 100 MiB bound is committed payload, not instantaneous disk/RSS usage.

## 4. Actual CJK Qualification

Full static TTF/OTF: 12 MiB per face, 24 MiB combined, 8 faces, still constrained by the selected complete-document budget. An EOT header plus a maximum face remains below the unchanged 16 MiB part ceiling. The parser's table/name limits and permission checks are unchanged. Variable fonts, collections, CFF2-only and color fonts remain unsupported. Oversized/unsupported native font resources remain opaque with inventory status; no labels or original bytes are silently dropped.

The official Google Fonts Noto Sans JP source is pinned at revision `66a36c8c94b1a5d992ee4e7f392fccfe4945767c`. An isolated FontTools 4.59.2 instancer creates weight 400 with all 17,936 glyphs retained, no subsetting, unchanged `fsType=0`, and retained OFL notices. The product does not instantiate or download fonts automatically. `0x0100` remains respected because AISlide performs no subsetting. Font embedding still requires the user's actual rights acknowledgement; metadata alone is not legal proof.

Measured fixture: 5,766,884 bytes, SHA-256 `c2893b7e910d63f34972e80a794a9ca336b320b838597adf5a77b4c240de8db2`. Native PPTX: 3,345,321 bytes; reopened document: 12,153,815 bytes. Japanese text measurement, document-local loading, embedding, native reopen, text edit and exact-byte Undo passed. Standard-profile lowering correctly rejects the reopened document. Actual native text editing took about 21 seconds in this debug build; not a benchmark guarantee. Office recognition, pixel parity, arbitrary CJK fonts and representative CFF OTF are not qualified here.

Optional fixture preparation (fonts and tooling remain ignored/local, never published):

```powershell
python -m pip install --target .tools/phase5-fonttools fonttools==4.59.2
& 'C:/Program Files/nodejs/node.exe' tools/cjk-font-fixture.mjs --accept-ofl
```

The helper downloads about 9.6 MB from the official pinned font repository plus its OFL file, below the approved 25 MB font download budget. It does not search private font directories or redistribute Microsoft fonts. See [the font guide](../authoring/fonts.md).

## 5. Verification And Remaining Gates

Focused tests cover 256 slides with all 296 custom identities, native export/reopen/edit/Undo, legacy/standard rejection and request isolation, UTF-8 document/history limits, both receipt chains, stale hashes/origin paths, generation/consent/expiry, entry/aggregate budgets, file-lock exclusion, native store restart and corruption, browser reload restore/Undo/Redo, cross-window deletion races, cancellation/disposal, v1 retention and visible storage failures. Real CJK bytes are tested through SDK, MCP and desktop/mobile Studio. Fixture-dependent CJK tests skip explicitly when the opt-in file is absent; such a skip is not qualification.

Repro commands use the full Node executable on this host:

```powershell
& 'C:/Program Files/nodejs/node.exe' tools/cargo.mjs test -p aislide-core --test capacity --test fonts --test transactions --test protocol --test single_file --test static_api --locked --offline
& 'C:/Program Files/nodejs/node.exe' tools/cargo.mjs test -p aislide-core --lib recovery --locked --offline
& 'C:/Program Files/nodejs/node.exe' tools/cargo.mjs build -p aislide-cli --locked --offline
& 'C:/Program Files/nodejs/node.exe' --test --test-concurrency=1 tools/core-client.test.mjs tools/client.test.mjs tools/mcp.test.mjs tools/mcp-poc.test.mjs tools/capacity.test.mjs tools/fonts.test.mjs
& 'C:/Program Files/nodejs/node.exe' tools/cargo.mjs test --manifest-path apps/studio/src-tauri/Cargo.toml --bin aislide-studio --locked --offline
& 'C:/Program Files/nodejs/node.exe' 'C:/Program Files/nodejs/node_modules/npm/bin/npm-cli.js' run build
```

Browser scope: `phase5-recovery.spec.ts`, `fonts.spec.ts`, `recovery.spec.ts`, and `output-recovery-workflow.spec.ts`, on a fresh loopback port. The built-Studio capacity test in `tools/capacity.test.mjs` serves the production assets in an isolated owned browser and edits the 256th slide. Screenshots are under `.artifacts/phase5-*` and `.artifacts/g38/studio-256-*`.

Final slice results: focused Rust integration 78 passed, private file-store restart 1 passed, native command unit tests 5 passed, final serial Node SDK/MCP/CJK/capacity 53 passed (no skips), final four browser suites 29 passed on fresh port 58374. The final two native profile-sealing/revision-size changes were followed by all 19 capacity + 6 transaction tests. Production build, TypeScript and lint passed; three pre-existing Fast Refresh warnings and Vite's existing chunk-size warnings remain. Logs: `.artifacts/phase5-final-20260919/`.

Earlier failures are retained in those logs, not relabeled as passes: Windows build-script sharing violation (same storage test retry passed without killing a process); stale standard-profile test expectations; missing await for the 256-slide Undo response; a 180-second aggregate CJK-test timeout followed by a busy next test (finite 300-second test budget and propagated cancellation fixed it); and recovery panel readiness/early-close races fixed by per-effect disposal and modal task ownership. Final Node 53/53 and browser 29/29 are complete current runs, not sums of selected retries. Normal editing and background recovery now have separate one-operation worker slots; a second concurrent recovery still rejects.

Native worker compilation/unit tests and disposable file-store reopening are not an installed-app restart test. The parent owns final source-freeze gates, actual desktop persist/restart/Undo/Redo, whole-product regression, Office qualification, packaging/install and Git publication. No claim of those gates is made by this slice.

## 6. Changed Files

1. Shared core: `src/{limits,preflight,model,editing,import,report,document,protocol,lib,fonts,authoring_ops,recovery}.rs`, `src/recovery/storage.rs`, `tests/{capacity,fonts}.rs` under `crates/aislide-core`. New recovery modules own validation/policy/private-slot behavior; existing names remain compatible.
2. SDK/bridge/MCP: `packages/client/{index.mjs,index.d.mts,README.md}`; `tools/{core-client.mjs,core-client.d.mts,core-client.test.mjs,mcp.mjs,mcp-poc.test.mjs,client.test.mjs,capacity.test.mjs,fonts.test.mjs,cjk-font-fixture.mjs}`. No manifest/runtime dependency addition; FontTools is isolated test tooling only.
3. Studio/native: `apps/studio/src/{Studio.tsx,api.ts,DocumentSetupPanel.tsx,FontPanel.tsx,RecoveryPanel.tsx,SessionRecoveryPanel.tsx,recovery-v2.ts}`, `apps/studio/vite.config.ts`, `apps/studio/src-tauri/src/{main,requests}.rs`. Legacy `recovery.ts` remains unchanged.
4. Browser tests: `tests/e2e/{phase5-recovery,fonts,output-recovery-workflow}.spec.ts`. The unchanged v1 `recovery.spec.ts` also passed.
5. Documentation: this guide, `docs/testing/capacity.md`, `docs/authoring/{fonts.md,README.md}`, `docs/api.md`, `docs/support-matrix.md`, phase 5 row only in `docs/planning/editing-completion.md`, root README and SDK README. Existing dirty changes outside this list were preserved.

The generated handoff manifest records exact source hashes and UTF-8/BOM checks after the final gates. No source freeze implies permission to commit, push, update user applications or run Office; those actions remain with the parent.

## 7. Bounded Native Integration Evidence

Additional owned debug-desktop verification, 2026-09-19. This supplements the earlier phase-5 results; it is not a regular installed-app, Office or full-product release claim.

- Native Rust **11/11** passed, including separate foreground/preview admission, second-preview rejection, exact operation allowlisting, mutation-payload substitution rejection, window/ID cancellation, bounded early/late cancellation records, cancellation retaining a live worker's slot, stale-guard ownership and private recovery-root validation.
- Final native E2E **5/5 Node test entries** passed, no failures/skips: the existing authoring/generation/native-save scenario, three modern subtests and their parent. The modern UI selected `arch_down` from all eight WordArt presets and applied linear regression plus standard-deviation bars with multiplier 1. Both SVG images decoded successfully. A native transaction completed while a 1,871,688-byte preview was pending. Native statistics also completed with a foreground generation request held by the synthetic loopback fixture; cancellation awaited the actual network disconnect. A separate owned native save dialog was cancelled with preview IPC admitted.
- Actual process restart: first owned PID **51756**, second **49452**, different freshly created WebView profile directories, same explicitly isolated private recovery directory. UI edits created one text insertion and one text replacement; Undo left exactly **one past and one future receipt**. The explicit v2 consent transition completed, the UI reported the snapshot saved, and native `state`/`load` confirmed committed generation **2** before the first owned process was stopped. The second process restored the checkpoint, redid the replacement, undid it, and undid the initial insertion to an empty slide. This verifies application-local file persistence independently of WebView storage.

Test-only host configuration:

```text
AISLIDE_TEST_RECOVERY_ROOT=<direct child of OS temp named aislide-native-owned-*>
AISLIDE_TEST_RECOVERY_OWNER=<36-character owned fixture identifier>
```

The helper creates `.aislide-test-owner` with that exact identifier before launch. The host resolves and validates this override once at startup; it requires a canonical direct temporary-directory child and an ordinary, matching 36-byte marker. Relative, nested, missing-marker and wrong-owner paths reject. Release builds reject the override instead of silently falling back to real AppData. No IPC request accepts a recovery root. Normal launches still use Tauri `app_local_data_dir()/recovery-v2`; tests never read, configure or delete that real-user directory. All owned processes are stopped before their private storage is removed. Autosave does not publish or overwrite a source PPTX. `AISLIDE_NATIVE_TEST_EXE` selects an alternate debug executable when the normal build path is occupied; WebView-profile isolation alone is never used as recovery isolation.

Reproduction uses the existing commands `node tools/cargo.mjs test --manifest-path apps/studio/src-tauri/Cargo.toml --locked --offline`, `npm run build`, `node tools/cargo.mjs build --manifest-path apps/studio/src-tauri/Cargo.toml --features custom-protocol --locked --offline`, and `node --test --test-concurrency=1 tools/native-generation.test.mjs`. Full Node path on this host: `C:/Program Files/nodejs/node.exe`.

Exact logs under `.artifacts/native-bounded-20260919/`:

- `build-1789769591823.log`: current production frontend build, exit 0; existing chunk-size warning remains.
- `native-unit-1789770630301.log`: final 11 native unit tests, exit 0.
- `native-build-final-1789770731880.log`: final custom-protocol debug native build, exit 0.
- `e2e-final-1789770787502.log`: complete final native E2E, exit 0, including network lifecycle and restart proof records. Earlier failures are retained, not combined into this passing result.

Built product: `apps/studio/src-tauri/target/debug/aislide-studio.exe`, **1,014,415,872 bytes**, SHA-256 **68e2ea07a9968e5c4fa902064d4b033cb28dfbf8c088dfa1f17a32973e1f0048**. This is the tested unoptimized debug executable, not an installer. The separate Tauri Cargo lock was synchronized without changing dependency source versions. No running regular application was renamed, killed or replaced, and no installation or Git operation was performed.

**Print remains unverified and blocked in this snapshot.** The parent-owned `ExportPanel.tsx` still prepares an iframe, while the native CSP has `frame-src 'none'`; the requested owned `window.open` preview flow is not present. No print command, print dialog, spool job, physical print or global-window capture was invoked. A native save dialog cancellation is not print-dialog evidence. The parent must complete the owned preview-window integration and establish exact owned-window/child-process cancellation before running the actual WebView2 print-dialog gate. No capability or CSP relaxation was made here.

## 8. G35 Direct Print Follow-Up (2026-09-19)

This follow-up supersedes the print blocker in the preceding historical snapshot, not its recovery or RequestGate evidence. The shared browser/native `ExportPanel` now uses a React portal directly under `document.body`. Its private, cryptographically generated class identifies a container containing only core-generated PNG images with local Blob URLs. There is no iframe, `srcDoc`, imported HTML, script interpolation or `dangerouslySetInnerHTML`. Native CSP, including `frame-src 'none'`, and native permissions are unchanged.

Prepare export does not print. Prepare print renders the selected PDF page indices again through the existing core at scale 1 (96 dpi), with opaque PNG backgrounds, regardless of the PDF scale. Admission checks require 1..32 selected pages, exact response count/order, PNG MIME/signature/byte length, equal integer dimensions in 1..8192, decoded image dimensions matching the response, and summed base64 data no larger than 3 MiB minus 1024 bytes. Core and static-response budgets remain in force independently. Print-only CSS requests the checked width/96 and height/96 in inches with zero margins, puts each image on a separate page, and hides every other body child including Studio and its modal backdrop. Outside print media, Studio stays visible and the owned print area stays hidden. Printer support for custom paper sizes, margins and scaling remains system-dependent.

Only the explicit Open print dialog gesture calls the main `window.print()`. There is no save-to-file, silent print, arbitrary-path command or automatic fallback. The installed Wry 0.55.1 WebView2 implementation of `print()` itself evaluates `window.print()`, so switching APIs would not provide a different backend. A missing, throwing or ignored print function reports an explicit error with PDF-download fallback. `beforeprint`/`afterprint` share one lifecycle in web and native. After closing the dialog, the status explicitly says AISlide cannot distinguish printing from cancellation. Prepared image URLs, CSS and DOM are removed on completion, new export/options, close or unmount. Session identity, revision and generation checks reject late/stale preparation results; print requests recheck revision. Export controls and panel close are unavailable while a requested dialog is active.

Focused reproduction (use `C:/Program Files/nodejs/node.exe` as `node` on this host):

```text
node node_modules/@playwright/test/cli.js test tests/e2e/output-recovery-workflow.spec.ts --grep "print (content|rejects|drops)|actual Studio exports"
npm run build
npm run lint
node tools/cargo.mjs build --manifest-path apps/studio/src-tauri/Cargo.toml --features custom-protocol --locked --offline
node --test --test-name-pattern="native owned direct print" tools/native-generation.test.mjs
```

The browser tests use a reserved loopback `AISLIDE_TEST_PORT`; they verify real core PNG preparation, multiple pages, paper CSS, source-preserving PDF/PNG/JPEG downloads, explicit gestures, URL revocation, seven invalid-response variants, ignored print, and StrictMode late-result rejection after session replacement, revision change and unmount. Mocked browser print functions only test lifecycle, not an OS dialog.

Actual native evidence is separate: `.artifacts/g35-directprint-20260919/native-print-proof.json` records the preflight-owned executable/PID/HWND, prepared PNG dimensions, visible native Print and enabled Cancel controls, UIA Cancel invocation, WebView2 version, `beforeprint` then `afterprint`, and final cleanup status. On this host WebView2 **153.0.4234.48** embeds the print UI inside the owned Tauri main HWND, rather than a separate top-level dialog. The test helper accepts only its exact executable and unique launch marker, then the preflight HWND or owned WebView2 descendants. It never invokes the Print control. The fixture uses the existing temporary recovery-root override and a fresh WebView profile, then cleans up only its owned process and files.

No physical print, spool job, PDF/file print, normal installation, Office automation, user-window operation, Git operation, model call or download was performed. This qualifies dialog display and cancellation on the tested debug Windows/WebView2 build, not printer output, Office visual parity, other platforms, or a release installer. Final command logs, versions and source/product hashes are indexed by `.artifacts/g35-directprint-20260919/handoff.json`; earlier failed detection attempts remain as evidence, not passing results.