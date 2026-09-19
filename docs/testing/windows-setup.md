# Windows Setup And Start Menu

Date: 2026-09-19. Windows setup uses the existing Tauri CLI 2.11.4 and its NSIS template. The authorized regular desktop update is complete. Ordinary commit/push to the existing public `murasamelabo/AISlide` repository's `main` branch is authorized but still pending; the publication owner must record the actual SHA after pushing. Signing, choosing a project license, force push, visibility changes and release/binary publication are not authorized by this update. See the [current completion ledger](../planning/editing-completion.md).

## Current Verified Update

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