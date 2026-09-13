# First-Slice Verification

Date: 2026-09-12. Scope: the first functional vertical slice, not the twelve-week PoC.

## TDD Evidence

| Behavior | RED evidence | GREEN evidence |
| --- | --- | --- |
| Package no-op and part patch | Two failures: OPC reader not implemented | Package tests passed |
| Native report and text patch | Missing report/PPTX API | Report tests passed |
| Shared JSON protocol | Missing execute_request API | Three protocol tests passed |
| CLI request and exclusive save | Two CLI not-implemented failures | Two CLI tests passed |
| Node bridge | Four bridge-not-implemented failures | Four bridge tests passed |
| Studio edit, undo, export | Missing slides, then inaccessible textbox query | Main browser journey passed after label fix |
| Scene reopen | Missing file input | Scene checkpoint journey passed |
| MCP headless workflow | Official client received Method not found | Generate/edit/read/export/collision/traversal workflow passed |
| Duplicate archive member | Crafted ZIP with two identical member names was accepted | Preflight rejects count mismatch after ZIP library deduplication |
| Failed publication cleanup | Cleanup helper absent | Two failure-injection tests passed |

Tests are under `crates/*/tests`, `tools/*.test.mjs`, and `tests/e2e`. No checkpoint commits were made because commits were not authorized.

## Native And Office Evidence

- `cargo check` and `cargo build` succeeded for the Tauri shell using the isolated x64 GNU toolchain on Windows ARM64.
- `npm run build` passed TypeScript checking and Vite compilation.
- Browser E2E verified editing, Undo, actual PPTX download, invalid report handling, responsive width, and automated serious/critical accessibility checks on editor controls. The slide content itself is excluded from the axe pass.
- `tools/native-smoke.mjs` connected to the actual Tauri WebView2 instance, saw twelve slides, edited text, undid the edit, and exported a ZIP through the real Rust IPC command.
- Open XML SDK 3.5.1 schema validation passed for the generated sample.
- Microsoft 365 PowerPoint opened the corrected sample: **12 slides, 136 text-frame shapes, 3 native tables, 12 populated note pages**, and two PNG exports. Text-frame count includes shapes capable of holding text, not necessarily 136 nonempty text boxes.
- Cover and table PNGs were visually inspected. They were not pixel-compared against every Studio slide.
- Source SHA-256 was checked before and after Office verification and remained unchanged.

## Compatibility Defect Found And Fixed

A sample with a theme part shared by the slide master and notes master passed SDK schema validation but PowerPoint refused to open it with `0x80070570`. A native PowerPoint control document opened successfully. Copy-only isolation showed the AISlide content opened when notes were removed; emitting a separate notes theme part resolved the failure. A regression test checks distinct theme targets.

Temporary diagnostic copies live under ignored `.artifacts/` and local temp directories. No native Office XML was copied into the product implementation.

## Remaining Gaps

- No broad real-world or public presentation compatibility corpus yet.
- No measured line/branch coverage percentage; 80% coverage is not claimed.
- No guaranteed PowerPoint/browser text wrapping, font substitution, or pixel parity.
- Native save-dialog interaction was not automated; native IPC export was verified separately from browser download.
- No native ARM64/MSVC or signed-installer verification; the tested native executable is x64 GNU.
- Reviewer agents could not read repository files in this environment. A bounded independent review of supplied safety excerpts found no critical/high issues; this is not a full repository review.
- No AI/provider call was made. The example and layout compiler are deterministic and explicitly synthetic.

## Self-Assessment

| Axis | Score | Evidence / remaining gap |
| --- | --- | --- |
| Accuracy | 4/5 | Real Office and native IPC checks; no complete parity claim |
| Completeness | 4/5 | First generation/edit/headless path works; larger PoC remains unimplemented |
| Clarity | 4/5 | README separates available features from planned scope; setup differs by toolchain |
| Actionability | 4/5 | Runnable editor, CLI, MCP and reproducible tests; installer not provided |
| Conciseness | 3/5 | Environment setup and Office troubleshooting required many iterations |

Average: 3.8/5. Highest-value follow-ups: measured coverage, a public compatibility corpus, and a model adapter that operates through the verified core. The assessment deliberately does not imply completion of the overall PoC.