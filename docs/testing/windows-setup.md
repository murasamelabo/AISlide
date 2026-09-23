# Windows Setup And Start Menu

Date: 2026-09-23. Windows setup uses Tauri CLI 2.11.4 and its NSIS template. The regular desktop update now includes transparent relationship labels, the shared core for [MCP visual authoring and delivery bundles](../authoring/README.md), and keyboard access to notes and scrolling toolbars. Source commit [3c20560a38b72d384287dc4a078c458a3fc70e7c](https://github.com/murasamelabo/AISlide/commit/3c20560a38b72d384287dc4a078c458a3fc70e7c) was pushed before installation. No signing, project-license choice, force push, visibility change or release/binary publication was performed.

## Current Verified Update

The regular current-user update and normal Start Menu launch were verified at
**2026-09-23T04:21:59Z**. AISlide was already closed before the installer ran
once and exited 0. No user process was terminated, no elevation or uninstall
was used, and the existing HKCU registration and argument-free shortcut were
preserved. The visible, responding app was left open with its normal WebView
profile and no test or remote-debugging flags.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Unsigned debug x64 GNU NSIS installer | 123,521,080 | `6dfbad81b81e05f22a314234a4cfc8ced49de1a06baffb5a7efe21ee42e9cac9` |
| Source bundle executable | 959,813,120 | `d6187b000823657a8b8455abd966020f651e7116184768aee8f298652b60a53d` |
| Installed executable | 959,813,120 | `4924337bffadec60bdcb13c9c4e3d62bf538c72b93ff2fda808a38316a1181c5` |

Every installed byte matches the qualified source except the expected three
Tauri marker bytes at offset **174306440** (`UNK` to `NSS`). The candidate and
actual installed executable each passed the same **3/3 native scenarios**:
Fit/wheel and resizable panes; editing, Undo and native Save-Cancel-Save/PPTX
reopening; and local cloud images, nested graph edits and metadata reopening.
There were no failures, skips or cancellations. Test-owned processes and
temporary WebView/recovery directories were cleaned before normal launch.
Eight candidate screenshots decoded as nonblank; representative pane and
nested-graph views were inspected. A separate restart-recovery scenario and
installer/uninstaller lifecycle were not rerun.

The 1,861-file preservation snapshot stayed unchanged through packaging,
installation, isolated tests and immediately before normal launch. It covers
settings/recovery content, selected WebView storage, WebView inventory,
model metadata and the local icon packs. Model contents were not rehashed.
Normal startup may subsequently modify its profile. All 366 source snapshots,
including 11 unrelated local encoding changes, were preserved during the
desktop update. No user presentation was opened or overwritten.

The feature baseline passed Rust **559** with six existing ignored cases and
Node **182/182**. After the keyboard correction, the four affected browser
suites passed **79/79** without retries or skips; Studio build/typecheck and
lint passed with nine pre-existing warnings in the local dirty worktree.
The old toolbar tests prohibited the horizontal scrolling already documented
in [workspace behavior](workspace-ux.md#resizable-workspace-panels). Their
replacement checks every enabled control through real Tab navigation and
full viewport visibility at 1440px and 390px, while retaining page, header
and dialog overflow assertions. Notes retain the full-editor axe check and
now have a named, keyboard-focusable region.

Private evidence: `.artifacts/mcp-publish-20260923/desktop-final.json`,
`build.json`, `candidate-native.log`, `installed-native.log`, and
`affected-browser.stdout.log`. The earlier feature baseline is recorded in
`.artifacts/p1-delivery-verify-xIXDIE/verification.json`. At the post-install
check, [feature CI](https://github.com/murasamelabo/AISlide/actions/runs/35816171711)
had passed its core, MCP, SDK, build and lint stages and was still running
browser tests; this is not a completed remote CI approval.

This remains an unsigned debug x64 GNU build on Windows 11 ARM64, not a signed
release/native ARM64/MSVC or Office visual-parity qualification. Vendor artwork
and model weights are not bundled. The standalone MCP server uses the updated
repository tools and CLI; desktop installation does not restart an existing
MCP connection or preserve its in-memory handles across a restart. Save any
active MCP document before reconnecting to expose the new tools.

## Earlier Cloud Update

**Historical: the 2026-09-20 update below is superseded by the current update.**

The regular per-user update and normal Start Menu launch were verified at
**2026-09-20T00:25:36Z**. A fresh process check confirmed AISlide was already
closed. The validated NSIS installer ran once and exited 0 without elevation,
uninstallation or terminating a user process. HKCU registration and the
existing argument-free Start Menu shortcut still target the normal installation.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Unsigned debug x64 GNU NSIS installer | 123,020,663 | `462b08b51a0fff6159661f0afa447f1535b7ac716f1364116eee1e8199a04843` |
| Source bundle executable | 954,322,944 | `ac9a91d6f4223330f8bd62cf091dcc9e4c0e7015e26fda7ed67de5e297811954` |
| Installed executable | 954,322,944 | `8f44b72aa7cfb1109bde1b2c3ce393e044346c999cd1813da50702bccf2d14e6` |

The installed payload differs only at the three-byte Tauri marker starting at
offset **173431992** (`UNK` to `NSS`); every other byte matches the qualified
source bundle. The unchanged build passed its three native scenarios again
against the **actual installed executable: 3/3 passed**, with zero failures,
skips or cancellations. These cover Fit/wheel and all three resizable panes,
existing authoring/Undo/native Save-Cancel-Save/PPTX reopening, and original
cloud PNG bytes, nested boundaries, descendant movement, connections, graph
history and editable metadata reopening. Test-only WebView/recovery directories
and their owned processes were cleaned before normal launch. A separate
restart-recovery test and installer/uninstaller lifecycle were not rerun.

Selected settings/recovery content hashes, WebView inventory, model metadata,
and all **1,472 icon-pack files** remained unchanged after installation, after
isolated tests, and immediately before normal launch. The pack comprises 1,471
deduplicated PNGs plus its acknowledgment. Model contents were not rehashed.
Normal startup may legitimately modify its profile afterward. The real Start
Menu shortcut opened a visible, responding app with the normal profile and no
test or debugging flags; it was left open. No user presentation was opened or
overwritten. All 363 source-file snapshots, including the 40 published feature
files and eight then-uncommitted files, stayed unchanged during installation.

The preceding local gates passed: Rust **535** with six existing explicit
ignored cases; browser **80/80**, zero skips/retries; Node **62** with one
intentional isolated-installed-lifecycle skip; frontend build/typecheck and
lint, with five pre-existing warnings. The native candidate and installed
three-case results are separate executions of the same cases, not six distinct
tests. Earlier interrupted/failed attempts remain recorded and are not counted.
Evidence: `.artifacts/cloud-desktop-final-20260920-a91f7c/final-proof.json` and
`.artifacts/cloud-regular-update-20260920-c4e917/final-run/final-proof.json`.

The first remote Verify run for `ec88ecb` failed the existing Lucide license
comparison because Windows checkout converted its line endings to CRLF. The
follow-up Git attribute fixes only that license file to LF, without changing
license text, assertions, runtime code or this installed binary. Checkout
filters with `autocrlf=true`, `false` and `input` all retained the exact upstream
text, and both original icon tests passed locally. This is not a claim that
the entire remote workflow passed; its status is reported separately.

The installer contains code and catalog metadata, **not vendor artwork**.
This PC's explicitly provisioned Azure/Entra, AWS and Google Cloud pack remains
available offline; other PCs require the separate opt-in setup and applicable
vendor permissions. This is an unsigned debug x64 GNU build on Windows 11
ARM64, not signed release/native ARM64/MSVC or general Office visual-parity
qualification. Logical nested boundaries and deterministic connectors do not
provide automatic obstacle avoidance.

## Earlier Canvas Update

**Historical: the Fit/wheel update below is superseded by the current cloud and
resizable-workspace update.**

The regular per-user update and normal Start Menu launch completed at
**2026-09-19T14:25:09Z**. The app was closed before installation; no user process
was terminated. The existing installation, shortcut target and argument-free
launch were preserved. The updated window was visible and responding with its
normal profile and no test/debugging flags, and was left open.

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| Unsigned debug x64 GNU NSIS installer | 122,824,298 | `7214528193d69ccf94c33b1b27147669fd2eb9b524579434c57349e8ce513d8f` |
| Source bundle executable | 951,846,400 | `f5d84d88bbb082bd5c6c6b2475c20bd76c5ef8bdc6804f01b0679ae1aef5eaf2` |
| Installed executable | 951,846,400 | `0206975886cacfd6105c7292a1504e3173ea0c77096b733752e16cac54b3e514` |

NSIS exited 0. The installed payload differs only at the three-byte Tauri
bundle marker starting at offset 173109704 (`UNK` to `NSS`); every other byte
matches. `npm run setup:build -- --debug --no-sign` built the current source,
whose code bodies match the published commit. Existing unrelated encoding
edits were retained. Only the BOM removed by the bundler from the Cargo
manifest was restored after checking that its body was unchanged.

The installed binary passed **5/5 Node test entries**, with no failures or
skips: the canvas Fit/wheel scenario and the existing bounded-preview/recovery
group. This includes measured 1200 x 768 Fit, Inspector shown/hidden, 16:9 and
4:3 geometry, wheel/restore behavior, and Undo/Redo across an isolated restart.
Tests used fresh marked recovery directories and WebView profiles and cleaned
their owned resources. The earlier separate installer/uninstaller lifecycle
was not rerun; this update verified the actual regular installation instead.

Before normal launch, selected settings and recovery content hashes and the
WebView inventory remained unchanged across installation and isolated tests.
Model preservation used size/timestamp inventory, not content hashes. Normal
startup may subsequently update its profile. No presentation was modified.
Evidence: `.artifacts/desktop-canvas-update-LWyD84/final-proof.json`, with
`setup-build.json` and `installed-smoke.log` in that private directory.
This remains an unsigned debug x64 build on Windows 11 ARM64, not a signed
release or native ARM64/MSVC qualification. The histogram compatibility
classifier is a separate repository validation tool, not a new desktop toggle.

## Earlier Editing Update

**Historical: superseded by the canvas update above.**

The regular per-user desktop update completed at **2026-09-19T07:02:09Z**. NSIS exited 0, reused the existing HKCU installation at `%LOCALAPPDATA%/AISlide Studio`, and preserved the Start Menu shortcut targeting the installed application with no arguments. The actual shortcut then opened a visible, responding application using its normal profile, without remote debugging or test environment flags; that normal window was left open.

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| Unsigned debug x64 GNU NSIS installer | 122,860,638 | `14ae220acfc81b5cdb10c1f353bd7a873e2ab61535d009c13b168f79f2311127` |
| Source bundle executable | 951,846,400 | `4aefdcdcade347c2685f2b7966974526891c80603050dcce1c5ceb1e824527b5` |
| Installed executable | 951,846,400 | `5f2790e7c399e8f55368796fadb4b0ddfa35d874810387ec7769f7fd8d8fcc84` |

The installed executable differs from the bundle by exactly three bytes at offsets **173108712, 173108713, 173108714**: Tauri's `__TAURI_BUNDLE_TYPE_VAR_UNK` becomes `__TAURI_BUNDLE_TYPE_VAR_NSS`. All other bytes are identical; normalizing only that marker reproduces the source bundle SHA. This is the expected NSIS bundle marker, not an unexplained payload mismatch. The installed application is an x64 Windows GUI executable. The separate development-native executable used for the six-test native gate is not this bundle payload.

`npm run setup:build -- --debug --no-sign` succeeded on Windows 11 ARM64 with x64 GNU. Native unit tests passed **14/14**; the current native group passed **6/6**, with no skips. The final isolated installed lifecycle passed **5/5**, no skips, in **169.6 seconds**, including the actual test-owned Start Menu launch, running-app guards, shortcut/registration removal and unrelated-file retention. After the regular update, the installed binary passed a separate **4/4** bounded native group using isolated fixtures, including recovery across distinct processes/fresh WebViews and restart Undo/Redo. These counts describe separate gates, not an additive product-coverage total.

An earlier lifecycle reached the 180-second deadline while test-only packaging took about 112 seconds for the roughly 1GB debug payload. Only the **installed-test lifecycle** deadline was raised to **300 seconds**; no product timeout was changed. A later teardown exposed incomplete exit synchronization. The test now pins the exact owned process by PID and executable path, awaits `WaitForExit`, and awaits the actual NSIS worker using `/S` and `_?=<destination>`. Both synchronization changes together passed the existing five tests without weakening assertions or adding retries/skips. The old failure did not record worker exit status, so the individual contribution of application-exit timing versus the self-copy worker is not proven. The final run left no new owned temporary files, aliases, shortcuts or related processes; pre-existing test artifacts were left untouched.

Selected settings and recovery-store content hashes remained unchanged across installation and isolated tests, **before normal launch**. The WebView inventory was also unchanged. Model weights are not bundled or modified; existing model preservation was checked by size, timestamp and first/last 64KiB samples, not full-file hashes. Normal startup can legitimately update its own profile, so the pre-launch preservation proof is not a claim that all user-data bytes remain frozen afterward. No automatic downgrade or executable-only rollback was performed.

Local evidence names below are not public download links:

- `.artifacts/bounded-native-20260919-8Cwlpf/setup-build/execution.json`: successful build and installer/bundle hashes.
- `.artifacts/completion-native-20260919-6d914e/unit/command.log` and `.artifacts/bounded-native-20260919-8Cwlpf/native-full/execution.json`: 14 unit and six current native passes.
- `.artifacts/installer-teardown-tkidbE/final-gates.json` and its `command.log`: final isolated 5/5, duration, frozen artifact hashes and owned cleanup.
- `.artifacts/regular-desktop-update-20260919-1556-9e283c/final-proof.json`: regular update, three-byte payload comparison, preservation, installed 4/4 and normal Start Menu launch.

This remains an unsigned **debug x64** build tested on Windows 11 ARM64. Release/signing, native ARM64/MSVC, broad upgrade compatibility and missing-WebView2 setup remain unqualified. Model weights and generated verification artifacts are not publication payloads. The completion ledger separately records the limited Office qualification; installation tests do not establish Office visual parity.

The checkpoint sections from Editing Expansion through Lucide Catalog Refresh below are **HISTORICAL**. Their package sizes, test counts, authorization and statements about not updating the regular installation apply only to those earlier runs, not to the completed update above.

## Editing Expansion

**HISTORICAL: 2026-09-18 checkpoint, superseded by Current Verified Update.** The unsigned debug x64 installer was **95,174,769 bytes**, SHA-256 `7b17ed14074ae9b8c4c4a026f169a5c768d8247200ac66a89d63f658d73d1a94`.

It used the standard local setup output path described below, which now holds the current package rather than these historical bytes.

`npm run setup:build -- --debug --no-sign` completed with the [28-item editing expansion](../planning/editing-expansion.md). Native unit tests passed 5/5, the embedded WebView workflow passed, and the isolated installed lifecycle passed all 5 cases in 158 seconds. The test used a random product name, temporary directory and WebView profile; it did not install or update the regular AISlide application. The actual Start Menu launch, icon/part/preset editing, Undo, 0.000px pending move/resize preview displacement, running-app install/uninstall guards, shortcut/registration removal and unrelated-file retention passed.

The isolated installed test used the preceding 95,149,633-byte package (`e6ed887168026ddd61a7e3b803dc5a2725c0b1d9ba2984faa44b6ae51937afab`). The final package above was rebuilt after the Sunburst-only native boundary/legacy-encoding fix, with no packaging or UI code changes. The final core passed 444 tests, Node passed 66, and the rebuilt ordinary desktop passed the complete WebView workflow again in 50 seconds. The installed lifecycle was not repeated for this final XML-only change; its fresh generated/edited Sunburst files passed Office and schema checks separately.

An initial installed run reached its unchanged 180-second deadline. After adding stage timings, the same lifecycle passed: test-only rebundling took about 94 seconds, installation 26 seconds, and the remaining checks/cleanup completed within the deadline. No timeout, assertion or app guard was relaxed. This is a single successful current lifecycle, not a sustained installation-performance guarantee.

Save and close AISlide before manually running setup. The build remains unsigned and debug-only on Windows 11 ARM64 using x64 GNU. Release/signing, native ARM64/MSVC, broad upgrade compatibility and missing-WebView2 setup remain unqualified. The earlier checkpoint evidence below is historical.

## Parts And Guided Authoring

The preceding installer included the revised horizontal/vertical flows, trees and cycles, relationship-label and Japanese Venn fixes, and the shared core's four authoring profiles. The existing standalone MCP server exposes guide retrieval and validated creation; the desktop continues to use Parts library for direct insertion. These remain included in the editing expansion. See [parts verification](parts-library.md#2026-09-16-refresh) and [guided authoring](../authoring/README.md).

That installer was **83,341,220 bytes**, SHA-256 `c5280adc1d4a4a874bf7df5b7b2bf25654605cec2aed2a3c6f3fa9859f6262b7`, superseded above. `npm run setup:build -- --debug --no-sign` succeeded. All five `npm run test:setup:installed` cases passed against that core: the test-owned actual Start Menu launch retrieved all four profiles, inserted a segmented cycle, checked four filled polygon segments, and undid it. Existing icon/master operations, pending move/resize stability, GUI-subsystem checks and install/uninstall safety gates also passed. Native unit tests passed all four cases.

This remains an unsigned debug x64 build tested on Windows 11 ARM64. Save and exit the normal AISlide application, then run setup to update it. No regular installation, user window or presentation was modified by the isolated tests. Console-free startup and earlier drag improvements remain included. Release, signing, native ARM64/MSVC and missing-WebView2 qualification remain separate work.

## Console-Free Startup

The preceding refresh opens only the Studio GUI instead of also opening a blank terminal. The prior entry-point attribute selected the Windows GUI subsystem only when debug assertions were disabled, so the distributed debug build was a console executable. Studio now selects the GUI subsystem for Windows application builds in either profile. Rust test executables retain their console output through `not(test)`; the separate CLI and MCP are unchanged.

That installer was **83,062,695 bytes**, SHA-256 `5c681b29b2e35f0c31bd0c5f568a3303fb34187b693ca32d437a4b3fa4519c06`, now superseded by the parts/guided-authoring refresh above. It includes the canvas interaction improvements below. Save work and exit AISlide before running the updated setup. Do not close only the old black terminal while working: a console close can also terminate the attached app. Existing open windows are not changed automatically by a new build.

The added installed-test assertion reads the built and installed PE headers: Windows GUI subsystem `2` is required for both PE32 and PE32+. The original debug payload reproduced the failure with subsystem `3`; the rebuilt payload passed before and after installation. The actual test-owned Start Menu shortcut launched the app successfully, and the existing edit, Undo, running-app install/uninstall guards and cleanup checks passed.

Final checks for this correction: **149 distinct tests passed** (Rust workspace 140, native unit 4, setup/installed lifecycle 5). `npm run setup:build -- --debug --no-sign` passed, including the frontend typecheck/build and native build. Native unit tests used `node tools/cargo.mjs test --manifest-path apps/studio/src-tauri/Cargo.toml --bin aislide-studio --locked --target-dir apps/studio/src-tauri/target/x86_64-pc-windows-gnu`; the final installed check used `npm run test:setup:installed`. The first GUI-only attribute also hid test-harness output; excluding test builds restored the visible four-test result without restoring the app console. Reviews were excerpt-limited. Unchanged browser suites were not rerun for this native entry-point change.

This remains an unsigned debug x64 build tested on Windows 11 ARM64, not a signed release or native ARM64 qualification. No terminal preferences, shortcuts, file associations or user installation were modified by verification. Startup failures still exit with a nonzero code; a GUI launch has no visible stderr console, as in the previous release configuration.

## Canvas Interaction Refresh

The preceding installer added smoother canvas movement and resizing: animation-frame batching, translated movement, memoized unchanged content and a preview retained until the core edit completes. Existing Undo, error handling, blank startup, icon categories and master presets remain. See [the measurements and regression evidence](workspace-ux.md#canvas-drag-responsiveness).

That installer was **83,060,275 bytes**, SHA-256 `8133153c9cad8727364efd445fdbb1d0fd4e759f77dc1d147009a96f3864499c`, now superseded by the console-free startup refresh above. `npm run setup:build -- --debug --no-sign` succeeded; all five `npm run test:setup:installed` cases passed. The actual installed WebView held move and resize transactions, measured 0.000px preview displacement for each, and verified Undo before the existing install/uninstall lifecycle checks.

This remains an **unsigned debug x64 build** tested on Windows 11 ARM64. Save and close AISlide before running setup to update the regular installation. Verification used only a random test product, shortcut, directory and WebView profile; the user's running app and presentations were preserved. Signing, release/MSVC/native-ARM64 builds and WebView2-absent bootstrap remain unqualified.

## Blank And Master Refresh

The preceding 2026-09-16 build starts with one empty slide, filters the complete Lucide catalog by 42 official categories, and includes seven original native master presets with fonts, palettes, spacing and layout regions. It includes the fitted-canvas correction for parts placed into the Text and visual layout. See [preset definitions and verification](master-presets.md).

That installer was **83,054,559 bytes**, SHA-256 `d5824afc9d2d25fcdf005890b994ad8bae2f8e7d08c9acffc991379de53055ce`, now superseded by the canvas interaction refresh above. `npm run setup:build -- --debug --no-sign` and all five `npm run test:setup:installed` cases passed against this checkpoint. The isolated test launched the actual Start Menu shortcut, checked blank startup, category-filtered icon insertion, a master preset and layout, Undo, running-app guards, shortcut/registry removal and unrelated-file preservation.

This is still an unsigned debug x64 build qualified on Windows 11 ARM64, not a signed production release. Installation is not automatic: save and close AISlide, then run the updated setup. No regular installed app or user document was changed by the isolated verification.

## Lucide Catalog Refresh

Earlier on 2026-09-16, `npm run setup:build -- --debug --no-sign` rebuilt the desktop and setup with every canonical icon in Lucide React 1.43.0: **1,818 icons**. That installer was **82,552,327 bytes**, SHA-256 `e59c23a7c254b08927eab54db602a59f81485b36a5dd9bfceed262b484d7e331`. It superseded the 2026-09-15 artifact, and is now superseded by the blank/master refresh above.

The updated `npm run test:setup:installed` passed all 5 tests with no skips. In the test-owned installation, the actual Start Menu link launched the app; the picker reported all 1,818 icons with 60 choices per page; Zodiac Virgo was searched, rendered, inserted and undone. Running-app install/uninstall guards, owned shortcut/registration removal and unrelated-file preservation also passed. Test identities and temporary data were removed; the regular AISlide installation and user windows were not updated or closed. Save and close AISlide before running the regular installer to update it.

The package remains an **unsigned debug x64 build**, tested on Windows 11 ARM64. Release/MSVC/native-ARM64 packaging, signing and the WebView2-absent bootstrap path remain unqualified. The [icon verification report](workspace-ux.md#icon-library-expansion) records browser, core, bundle-size and review scope.

## Installed Behavior

- `tauri.windows.conf.json` enables NSIS for Windows only, with `currentUser` installation, an `AISlide` Start Menu folder, the existing ICO, and English/Japanese installer languages.
- The Start Menu shortcut launches the installed desktop executable, not Vite, npm, PowerShell or a path in the checkout. The finish page offers an additional desktop shortcut.
- The installer registers its own uninstaller. Uninstall removes its shortcuts and application entry; unrelated files in the installation folder are retained.
- Running AISlide instances block installation and uninstallation. Interactive setup asks the user to save and close the app; silent mode exits with code 2. It never intentionally terminates a running app.
- No autorun, taskbar pinning, file association, service, scheduled task or machine-wide installation is added. Runtime and source presentations are unchanged.

The hook overrides Tauri's `CheckIfAppIsRunning` macro through its supported hook include so the later standard checks cannot silently terminate the process. The two pre-install/uninstall hooks use the same guard. This relies on the NSIS utility names and include order in the locked Tauri CLI; rerun the real installed test when upgrading it. A removed macro causes packaging to fail rather than silently remove the guard.

## Build And Verify

```sh
npm run setup:build
npm run setup:build -- --debug --no-sign
npm run setup:bundle -- --debug --no-sign
npm run test:setup
npm run test:setup:installed
```

The first command selects release mode. The development command was executed on Windows 11 ARM64 using the repository's isolated x64 GNU Rust/LLVM-MinGW toolchain. The output is `apps/studio/src-tauri/target/x86_64-pc-windows-gnu/debug/bundle/nsis/AISlide Studio_0.1.0_x64-setup.exe`. The payload is x64; the NSIS bootstrap itself is an x86 executable. This is an unsigned debug installer, not a signed production release. Release/ARM64/MSVC packaging has not been executed in this checkpoint; those combinations require their own validation. The installed test accepts `--release` when a release setup build is available.

`setup:build` runs the frontend build, the ordinary Cargo host build into a separate target directory, then the standard Tauri bundler with an explicit matching target. `setup:bundle` uses the same host and directory convention but skips compilation. `--dry-run` prints the plan only. Builds reject `CARGO_BUILD_TARGET` and `CARGO_TARGET_DIR` overrides; custom cross-compilation/output configurations are not supported by this convenience command.

The separate Cargo output is intentional: the first attempt tried to package a running development executable and encountered a sharing violation; the ARM64 Node CLI also inferred the wrong payload architecture. Passing an explicit target to the whole GNU build then omitted target-specific linker flags from host build scripts, causing missing `libgcc` errors. A targetless host build with a dedicated `--target-dir`, followed by an explicit-target bundle, succeeded without changing the compiler or system settings.

## Original Setup Evidence

The configuration, installed-CLI dispatch, debug/release command planning and target-override checks passed. The opt-in installed test also passed against a uniquely named NSIS package with the same installer hooks and settings; only product/binary/shortcut identifiers and test compression were changed for isolation.

Final verification: `npm run test:setup:installed` passed all 5 tests with no skips; `node tools/cargo.mjs test --workspace --locked` passed 134 tests, and `npm run build` succeeded. The normal setup command has 4 passing tests and skips the opt-in installed test. The last command-routing changes received an excerpt-only review with no findings, not a full independent source audit.

The 2026-09-15 regular installer was 82,418,038 bytes, SHA-256 `8880773876859af218f3450798e6e7b577156a83e63c7c958c3a5396d77f191a`. PE headers identified the NSIS bootstrap as x86 (`0x014c`) and the 553,351,680-byte debug application as x64 (`0x8664`). These historical sizes include debug information; release size is not measured.

The installed test confirmed:

1. A Start Menu `.lnk` points to the installed executable with no added arguments.
2. Launching that actual shortcut opens the desktop WebView and loads the twelve-slide sample with no application alert.
3. Reinstall and uninstall both exit 2 while the test app is open; its window and installed executable remain intact.
4. After stopping only the test-owned process, uninstall removes the test shortcuts and HKCU application entry.
5. A synthetic unrelated file placed in the installation directory survives uninstall.

The test owns a random product name, temporary installation/profile, loopback debugging port and process identity. It removes its own test installer, alias binary, shortcuts and temporary data afterward. It does not stop existing AISlide windows, alter their profiles, overwrite existing shortcuts or inspect presentation contents. The actual `AISlide` Start Menu registration occurs when the user runs the regular installer; verification-only entries are not left behind.

NSIS and its Tauri helper were downloaded by the official CLI with hash verification. WebView2 was already installed, so bootstrap download/install was not exercised. SmartScreen/signing, enterprise deployment, broad upgrade compatibility and a real Start Menu pin are not claimed. No visual Office qualification is inferred from installation tests.

The CLI's manifest rewrite removes the UTF-8 BOM from the Tauri Cargo manifest. After the final bundling/test command, `npm run encoding:fix` restored it and `npm run encoding:check` passed for all 233 source/config files. No dependency or package metadata value was changed by that normalization.

References: [Tauri Windows installer](https://v2.tauri.app/distribute/windows-installer/), [NSIS configuration](https://v2.tauri.app/reference/config/#nsisconfig).