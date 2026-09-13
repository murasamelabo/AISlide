# Model Generation Verification

## Scope

The generation path is shared by Rust, the CLI JSON API, the official-SDK MCP server and Studio. Tests use a loopback HTTP fixture with an explicitly synthetic report. They do not measure real model quality, validate any provider credential, or establish Office visual parity.

## Reproduce

```sh
npm ci
npm run core:build
node tools/cargo.mjs test -p aislide-core --test generation --test protocol
npm run test:generation
npm run test:generation:e2e
npm run test:native
```

On a Windows desktop, build the embedded application with `npm run build` and `npm run tauri:build`, then run `npm run test:native:e2e`. The test owns a fresh native process, disposable WebView profile and loopback provider. It never attaches to a pre-existing app or uses a configured real provider.

The browser fixture starts its own Vite process on port 4176 and refuses to reuse another server. Microsoft Edge must be installed. Fixture endpoints and credentials are never added to production configuration.

## Contracts

| Surface | Check |
| --- | --- |
| Provider policy | Loopback HTTP allowed; remote HTTPS requires operator and request consent; document endpoint/key overrides rejected |
| HTTP | Single bounded response; no redirects; error bodies and credentials not echoed |
| Model output | Exact slide count and strict report JSON; malformed, truncated, refused and oversized output rejected |
| Compilation | Shared report compiler; source hash and unverified-content issue/notes retained |
| Cancellation | Pre-cancel avoids networking; in-flight cancellation closes the provider connection; a subsequent request succeeds |
| Tauri | Async generation works within a runtime; busy gate and cancellation scoped by window and operation ID |
| Studio | Review before Apply; Undo restores original deck; failure/cancellation preserves it |
| Accessibility | Labeled controls and small-screen dialog; no serious/critical axe findings in the tested dialog |

## Observed Regression

The initial browser test failed because generation controls did not exist. After implementation, three of four cases passed. Cancellation exposed a browser default-action issue: React reused the cancel button as a submit button during the same click, causing another generation request while the prior CLI was terminating. Separate button keys and cancellation `preventDefault()` fixed it. The same cancellation/retry test then passed, followed by all four generation browser cases.

Screenshots and traces are written under ignored `.artifacts/`. Full real-model qualification, broad compatibility data and coverage percentages have not been established.

The native WebView test also passed on Windows 11 ARM64 using the x64 GNU development build: generation request observed at the local provider, cancellation closed that connection, retry produced a draft, Apply replaced the scene, Undo restored twelve slides, and immediate Rust IPC cancellation was acknowledged. The native screenshot was inspected. This is workflow evidence, not a native ARM64/MSVC qualification or an Office visual-parity result.

## Local Checkpoint: 2026-09-13

| Gate | Observed result |
| --- | --- |
| Rust workspace | 33 tests passed |
| CLI bridge and atomic output | 6 tests passed |
| Existing MCP workflow | 1 test passed |
| CLI/MCP generation integration | 2 tests passed |
| Studio editor browser E2E | 5 tests passed |
| Studio generation browser E2E | 4 tests passed |
| Native request management | 2 tests passed |
| Native generation WebView E2E | 1 test passed |
| Studio build and lint | Passed |
| Embedded Tauri development build | Passed, x64 GNU on Windows ARM64 |
| Source/config encoding | 69 files checked, zero mismatches |

These are 54 local tests across distinct suites. No real provider credential or paid model endpoint was used. Coverage percentage and real-model content quality remain unmeasured.

## Hosted CI Prerequisite

The Private repository's first [Verify run](https://github.com/murasamelabo/AISlide/actions/runs/34727915785) stopped before any job step ran. GitHub's annotation reported an account payment/spending-limit restriction. This is neither a test result nor proof that the hosted runner configuration works. Billing and visibility were left unchanged. The owner must resolve that restriction, then dispatch the existing Verify workflow on `main`; local evidence above is the accepted checkpoint while that external prerequisite remains open.