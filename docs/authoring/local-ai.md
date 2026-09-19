# Local AI Editing (G04 / G12)

AISlide offers optional local-model proofreading, full-text translation and U2NetP saliency-based background removal. These are model results, not dictionary replacements or color-key templates. Results remain candidates until explicit Apply; Cancel, rejected output and stale revisions do not create history. Apply creates one ordinary transaction and one Undo. No weights are included in source or installers. The project license is unchanged.

## Text Setup

Reuse an existing OpenAI-compatible loopback model server. Set `AISLIDE_AI_BASE_URL` (for example `http://127.0.0.1:8080/v1`) and `AISLIDE_AI_MODEL` in the environment that launches the desktop application, CLI or MCP. Optional `AISLIDE_AI_API_KEY` is read only by the host; never enter credentials in slide content. `AISLIDE_AI_TIMEOUT_SECONDS` accepts 1-300 seconds. The desktop reads the same host environment as the CLI; it does not automatically start or download a language model.

For this checkout, `node tools/local-studio.mjs` starts the already-prepared `.tools/local-model/runtime/llama-server.exe` and `.tools/local-model/qwen2.5-1.5b-instruct-q4_k_m.gguf` with a new loopback Studio. It does not download another LLM. An installed desktop can instead be launched from the environment containing the two variables above while an operator-chosen local server is running. A desktop already running before environment changes must be saved and relaunched by its owner; this task does not stop it.

Use Text tools > Local AI. Choose Proofread or Translate, an object, source language and translation target (`en`, `ja`, `en-US`, `ja-JP`, or another valid language tag). Generate, inspect the deleted/inserted text, then Apply or Cancel. Missing configuration, refused/incomplete responses and schema errors are explicit. Check setup reports configuration only, not connectivity or language quality.

`text_assist` receives `{input:{task,text,language,target_language?}}`. Input and output are at most 8,000 Unicode scalars, nonempty and LF-separated; paragraph count must remain unchanged. The model must return exactly `{"text":"..."}`. Unknown/duplicate JSON keys, `kind`, tools, refusals, truncation and fenced/invalid JSON reject. Markup-like strings are rendered as inert text, never executed. No dictionary or template fallback is used.

`apply_text_assist` requires document/session, expected revision, slide ID, top-level text/shape ID, exact expected old text and `{text,source_sha256}` candidate. It validates the hash, old text, locked/hidden state and native preservation before committing. All field-bearing targets, including unknown field kinds/caches, reject. Each original paragraph is replaced through the existing rich-text helper, preserving paragraph settings and run styles on unchanged spans; newly inserted text inherits the existing helper's adjacent style. It does not infer semantic formatting for newly translated words. Existing text-object limits (4,000 scalars), native unknown-content guards and layout/export checks still apply; this feature does not raise capacity limits or automatically shrink overflowing text.

The completion transport is shared with report generation: literal loopback/localhost pinned to loopback, no proxy, redirect or retry, bounded 1 MiB HTTP response, host authentication, timeout and cancellation. Unlike generation, new text editing rejects remote configurations even when generation has remote consent. Strict JSON schema is always requested for text assistance.

## Image Setup And Rights

Review the [author's model license statement](https://github.com/xuebinqin/U-2-Net/issues/208) and applicable DUTS dataset terms. The author states that released code/models follow Apache-2.0 and explicitly directs users to check dataset licenses; U2Net/U2NetP were trained on DUTS. This is not a blanket legal clearance of dataset or input-image rights.

Explicit optional setup:

```text
node tools/local-image-model-setup.mjs --accept-model-and-dataset-terms
node tools/local-image-model-setup.mjs --accept-model-and-dataset-terms --file C:/existing/u2netp.onnx
```

Only one of these commands is needed. They install the verified model under `%LOCALAPPDATA%/AISlide/models/u2netp.onnx` and a small consent record. Existing files are not overwritten. No executable runtime or additional LLM is downloaded. A host-provided alternative local file uses `AISLIDE_U2NETP_MODEL` and `AISLIDE_SEGMENTATION_LICENSE_ACCEPTED=1`; only the same known bytes are accepted. Request JSON never accepts a path, URL, runtime plugin or arbitrary model. Network/device paths reject. Installation is separate from image processing; inference never fetches resources.

| Item | Pin / Provenance |
| --- | --- |
| Model download | [rembg v0.0.0 / u2netp.onnx](https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2netp.onnx), asset 85857953 |
| Size | 4,574,861 bytes (4.36 MiB) |
| SHA-256 | `309c8469258dda742793dce0ebea8e6dd393174f89934733ecc8b14c76f4ddd8` |
| Published loader MD5 | `8e83ca70e441ab06c318d82300c84806` |
| Release provenance | tag commit `7fb6683169d588f653281d53c3c258838194c950`; not a byte-integrity pin |
| Initial verification | 2026-09-18: GitHub release asset digest was null and release immutable=false. Published MD5 matched, SHA-256 independently computed and then pinned in setup/core. Future installs require the strong pin, not release immutability. |
| CPU runtime | `tract-onnx = 0.23.7`, MIT OR Apache-2.0, linked Rust dependency; actual x64 Windows GNU on ARM64 host tested. No downloaded ONNX Runtime DLL or custom operators. |

Use Object tools > Local AI background removal > Generate cutout. Review original and candidate, then Apply AI cutout or Cancel. General saliency is not interactive region segmentation, guaranteed object recognition, hair matting or production quality certification. The existing RGB background-key operation remains independent and explicitly non-AI.

Input validation retains 1 MiB PNG/JPEG, at most 4096 pixels per side, and the existing 64 MiB decoder/RGBA admission limits. RGB is resized with Lanczos3 to 320x320, divided by the global RGB maximum (epsilon guard), normalized by ImageNet means `[0.485,0.456,0.406]` and deviations `[0.229,0.224,0.225]`, and arranged as float32 NCHW 1x3x320x320. The first model output is already sigmoid saliency; no second sigmoid is applied. Nonfinite/wrong-shape/constant output rejects. Min/max normalization creates an 8-bit mask, Lanczos3 restores original dimensions, and mask alpha multiplies existing source alpha. Output is PNG capped at 1 MiB, without source metadata. It is never a color-key fallback.

Inference is serialized in each native host. Fixed tensor shapes and actual runtime execution order determine a peak live tensor admission estimate, capped independently at 768 MiB. This is not a hard process-RAM cap: parsing, optimization, operator scratch buffers, allocator overhead and image buffers are additional. The pinned model prevents untrusted graphs/external data/custom operators; unsupported runtime boundaries fail explicitly. Status verifies bytes/consent only and exposes no host path.

Native CPU inference is non-preemptible. Cancellation is checked before/after decoding/model load/inference/result construction; a running native call completes and its result is discarded. The UI stays busy until acknowledgement. CLI-based hosts additionally own their request child process and can terminate only that child. No user processes are stopped. `segment_image` is stateless and returns `{image,provenance}`; existing `apply_image_edit` performs the separate revision-checked native document transaction, retaining frame/crop/alt text and one Undo.

## Qualification, Not Quality Certification

2026-09-18, existing local Qwen2.5-1.5B-Instruct Q4_K_M, 1,117,320,736 bytes, revision `91cad51170dc346986eccefdc2dd33a9da36ead9`, SHA-256 `6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e`, official llama.cpp b10809 ARM64 CPU runtime already present. No extra LLM download. Four synthetic EN/JA proofreading/translation requests returned valid strict JSON in 333-677 ms after startup. English grammar corrected correctly. Japanese proofreading changed a past-tense sentence to present tense; EN-to-JA omitted an explicit meeting subject. These observed quality defects are why review remains mandatory; structural success does not certify meaning preservation or translation quality.

Actual U2NetP on an original nontrivial synthetic shaded vase/background: foreground alpha 255, background 0 and nonconstant intermediate edge alpha. Rust debug end-to-end 24,692 ms; actual browser/CLI run about 29,721 ms. Planned live tensor peak 107,166,732 bytes (102.2 MiB); process RSS/peak RAM was not measured. CPU speed and quality are fixture/hardware dependent. This is not a public-image benchmark.

Focused verification commands (run from repository root):

```text
node tools/cargo.mjs test -p aislide-core --test generation --test local_ai --test image_edit --locked --offline
node tools/cargo.mjs test -p aislide-core --lib segmentation --locked --offline
node tools/cargo.mjs test -p aislide-core --test local_ai real_u2netp --locked --offline -- --ignored --nocapture
node tools/cargo.mjs build -p aislide-cli --locked --offline
node --test tools/local-ai.test.mjs
```

Set `AISLIDE_REAL_AI_TEST=1` only to opt in to real existing Qwen tests and the G12 browser fixture. Three browser workflows in existing text-tools/object-tools-panels tests cover proofreading/translation review, cancellation and Undo using labelled bounded test responses, plus actual U2NetP review/apply/native reopen/exact-byte Undo. Desktop/mobile 1440/390px captures were inspected. No Office, user browser, installed desktop update, whole 175-test browser run or global model-quality claim is included. Parent workflow owns final installation, Git and broader qualification.