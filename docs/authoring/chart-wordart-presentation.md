# Chart And WordArt Presentation

Phase 3 bounded implementation, 2026-09-19. All eight WordArt selectors and native request isolation are implemented. This is display computation, not a replacement for native chart values, embedded workbooks, editable text, or the PPTX engine. Office validation of these display additions remains pending.

## Read-Only Contracts

- Core `model::chart_format::compute_chart_presentation`, protocol/MCP `compute_chart_presentation`, SDK `client.computeChartPresentation(chart)` accept typed Cartesian chart data. They return axis domains, log/reverse scales, major/minor ticks and labels; per-series XY points, labels, absolute error endpoints, regression samples, normalized coefficients, equation and R-squared. No document, receipt or revision is changed.
- Protocol/MCP `render_element_preview`, SDK `client.renderElementPreview(element, theme?)` accept a typed chart, text or shape only. They return core-generated inert SVG and explicit warnings. Raw SVG/HTML, URLs, paths and arbitrary document inputs are not accepted. Installed fonts are used; this stateless endpoint does not carry document-local font resources. Full-slide static export still uses the existing document-local font path unchanged.
- Studio advanced Cartesian charts and WordArt consume this SVG as an image, with input-content keys, obsolete-result disposal, a serial queue and a 24-entry small-preview cache. No `innerHTML` or per-character text warp is used. Ordinary charts and chartEx retain their existing preview paths. Web preview requests have one separate bounded worker; native foreground, preview and recovery processing are isolated as described below.

## Chart Mathematics

Input limits remain six series, 32 categories, finite raw values in +/-1e15. Non-XY charts use 1-based category coordinates; scatter/bubble use actual parsed numeric X. Secondary combo series have independent value coordinates. Forecasts extend the observed X range, at most 100000 units in each direction; regression paths contain at most 256 points, including every raw X. Sampled regression curves are approximations, not certified curvature/error envelopes.

Regression uses generic `nalgebra` SVD, not normal equations. X is centered/scaled before the Vandermonde fit. Singular values at or below largest singular value times 1e-12 reject rank deficiency; near-unresolvable X ranges also reject. Fixed-intercept fits scale about zero and remove the constant unknown. Returned polynomial coefficients use `z = (x - center) / scale`, or the equivalent transformed logarithmic X, not raw-X polynomial coefficients.

Supported fits:

- Linear, logarithmic, exponential and power: at least two observations and sufficient distinct X.
- Polynomial: degree 2..6, at least degree+1 observations and full column rank.
- Moving average: period 2..point_count-1, original observation order; no forecast, intercept, equation or R-squared.
- Logarithmic/power require positive X, including the requested forecast domain. Exponential/power require positive Y. Exponential fixed intercept means Y at X=0 and must be strictly positive before its logarithm is taken. Logarithmic/power fixed intercept is rejected by the existing native contract.
- R-squared is `1 - SSE/SST` on original Y values, including transformed regressions and fixed-intercept fits. Constant response returns null, with a warning when display was requested. This definition is not an Excel/Office numerical-parity claim.

Errors support fixed value, absolute-value percentage, sample standard deviation, sample standard error, and custom arrays; X/Y and both/plus/minus follow the existing native contract. Sample deviation uses denominator n-1. Standard-deviation bars are centered on the series mean; other errors are centered on each observation. The API returns absolute lower/upper XY endpoints, never negative deltas from an observation to the mean. Every derived endpoint and forecast must remain finite within +/-1e15; nonpositive endpoints on log axes reject rather than disappear.

Axes respect explicit bounds, positive log domains and reversal. Major/minor ticks are capped at 128 each; excessive explicit density rejects. Auto linear ticks use a 1/2/5 step series. Non-XY category axes retain the existing reverse/format-only native contract; numeric min/max/log options apply to XY category axes.

Numeric formatting deliberately supports only General, `0`, fixed decimals with up to 12 zeros, `#,##0` grouping, trailing percent, and `E+00` scientific notation. Unsupported sections, currencies, dates, colors, optional-digit patterns and literals warn and display General, while native format codes remain unchanged. General suppresses binary floating-point tails. This is not a full Excel formatter.

The advanced static/SVG adapter uses bottom legends and above-point data labels, with explicit warnings if native placement differs. Dense equations can require a larger chart. Native label placement is unchanged. Histogram bins and box quartiles were not refactored in this slice. The existing [histogram Office exception](chart-ex.md) remains exactly as documented: `val` bin attributes and two known public CT_Binning schema errors per histogram resource. No automatic bins, aliases, invented namespaces or validator suppression were added. The accepted catalogue remains 24 families; line3d, area3d and surface3d remain unsupported.

## WordArt

Validated public DrawingML names map to eight typed presets:

| Typed Value | Native Name |
| --- | --- |
| arch_up | textArchUp |
| arch_down | textArchDown |
| wave1 | textWave1 |
| wave2 | textWave2 |
| inflate | textInflate |
| deflate | textDeflate |
| slant_up | textSlantUp |
| slant_down | textSlantDown |

Cosmic-text shapes complete runs first, including ligatures and CJK. Its actual font glyph outlines are parsed/flattened with generic `kurbo`, subdivided and transformed in frame coordinates. Native text and `a:prstTxWarp` remain editable; existing outer shape rotation/flips are preserved. Native `a:avLst` is empty for this bounded default-adjustment set. Nonempty imported warp adjustments fail closed rather than being mapped to shape geometry adjustments. Unknown preset names are not claimed as implemented.

The local subdivision target is 0.25 pixels, with conservative curve flattening and horizontal subdivision, not Office geometry equivalence. Limits: frame dimensions at least 8px, aspect ratio 1:32..32:1, at most 8192 points and 128KiB path per glyph, plus the existing whole-scene SVG budget. Extreme subdivision rejects. Underline/highlight and PDF search/selection geometry are not warped; searchable/tagged semantic text remains on the existing `RenderedSlide` path. Tabbed WordArt remains unsupported by static rendering; Studio explicitly warns and shows unwarped rich text rather than losing tabs or fields. Neither PDF/UA nor visual parity is claimed.

The Object Tools selector exposes all eight presets and the TypeScript check passed after the complete list was connected. The bounded native integration test checks the option list, applies `arch_down` through the real dialog, and decodes the core-shaped SVG image. This is not Office visual-parity qualification.

## Dependencies And Integration

- `nalgebra = 0.33.2`, Apache-2.0, default features disabled and std enabled. Its published manifest does not declare an MSRV; repository rustc 1.98.1 compilation passed, a historical minimum compiler version is not certified.
- `kurbo = 0.13.1`, Apache-2.0 OR MIT, declared Rust 1.85. Already present transitively; now a direct geometry dependency. No PPTX engine implementation was copied.
- Root Cargo lock resolved five new packages for nalgebra. The separate Tauri lock is synchronized from the current manifests, without reverting source dependency versions. The debug Tauri product compiles with the current nalgebra, kurbo and tract dependencies. Final bundling, regular installation, publication and Office qualification remain separate gates.
- Native Tauri routes only exact `compute_chart_presentation` and `render_element_preview` operations into one preview slot, independent of its one foreground slot and separate one-operation recovery gate. Caller-supplied preview/lane flags cannot promote other operations; nonpreview operations reject on the preview path. A second preview rejects immediately; no native preview queue is created. One broker owns cancellation registration, with 64 early records and 64 recent-completion records, each expiring after 30 seconds. Early cancellation is retained even while both request lanes are occupied. Active cancellation matches window and operation ID; guards retain their slot until the worker exits and cannot release a replacement owner's slot. Cancellation is cooperative, not hard preemption of font/Rust computation. Existing core size/preflight limits still apply; this is not an RSS guarantee.

## Bounded Native Evidence

2026-09-19: the earlier bounded native unit suite passed 11 tests; after the full-lanes early-cancellation repair, the native unit suite passed **12 tests**. The existing-plus-modern desktop run passed **5 Node test entries**, no failures/skips (one existing scenario, three modern subtests and their parent), **before the latest security repairs**. Its two scenarios use distinct owned WebView profiles and marked private temporary recovery storage without inspecting or terminating user instances. This is not the final post-repair native/install result. `tools/native-generation.test.mjs` accepts `AISLIDE_NATIVE_TEST_EXE` when an independently built executable is needed.

The actual native UI applied `arch_down`, linear chart regression and standard-deviation error bars with explicit multiplier 1, then decoded both core-generated SVG images. Real native IPC also computed statistics while a synthetic loopback provider held the foreground generation operation. Cancellation awaited the provider's actual disconnect event. A foreground transaction completed while a 1,871,688-byte SVG preview remained pending; a native save dialog was cancelled while preview IPC was also admitted. The save test wrote no file. These checks use the compiled native transport, not a web helper or mocked request gate.

Earlier bounded-run logs: `.artifacts/native-bounded-20260919/native-unit-1789770630301.log`, `native-build-final-1789770731880.log`, and `e2e-final-1789770787502.log` in the same directory. The production frontend build log is `build-1789769591823.log`. These filenames do not designate the final post-security-fix product gate. Earlier failures remain recorded and are not counted as passes. See [bounded native recovery evidence](../testing/phase5-recovery-capacity.md#7-bounded-native-integration-evidence) and the [current verification ledger](../planning/editing-completion.md). Native Print/Cancel was subsequently verified separately, with no job submitted; see the [print contract](../api.md#static-export-and-recovery).

Stateless previews still have only the installed-font context described above; no document-local font resources, network font fetches or persistent preview artifacts were added. Regular desktop installation and Office validation remain pending final gates.

## Verification

The initial static regression failed at the old chart-options rejection; the initial WordArt regression failed at the old warp rejection. Both now pass.

Commands use `C:/Program Files/nodejs/node.exe`:

```text
node tools/cargo.mjs test -p aislide-core --test chart_format --test charts --test export_static --test visual --offline
node tools/cargo.mjs build -p aislide-cli --offline
node --test --test-concurrency=1 --test-name-pattern=phase3 tools/client.test.mjs tools/core-client.test.mjs tools/mcp.test.mjs
node node_modules/typescript/bin/tsc -p apps/studio/tsconfig.app.json --noEmit
node node_modules/@playwright/test/cli.js test tests/e2e/object-tools-panels.spec.ts --grep phase3
node node_modules/@playwright/test/cli.js test tests/e2e/document-render.spec.ts
```

Browser checks allocate a fresh `AISLIDE_TEST_PORT` on loopback and launch their own Edge context. They exercise actual settings/Apply, core-generated SVG pixels at 1440/390 widths, native reopen, format copying, exact-byte Undo, Japanese WordArt and deliberately delayed obsolete preview responses. Existing user tabs remain untouched. Core static tests exercise PNG/PDF, all 24 baseline families, searchable/tagged PDF semantics and eight WordArt presets. Native chart values/workbook bytes and source documents are checked unchanged by display calculations. Bounded debug-native concurrency is qualified above; final frozen-product, Office, regular-installation and publication results remain pending in the completion plan.