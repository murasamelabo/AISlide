# PoC Support Matrix

The independently implemented exporter follows public OPC/OOXML specifications using generic XML/ZIP libraries. This matrix describes actual support, not universal compatibility.

| Surface | Create/export | Import preview | Non-destructive import edit |
| --- | --- | --- | --- |
| Text | Native shapes; multiline text, color, bold, size | Direct geometry and text; mixed formatting approximated | Top-level single plain run per paragraph; same paragraph count; geometry |
| Rectangles | Native shapes and fill | Direct rectangles | Geometry only |
| Tables | Native editable cells; bounded rectangular data | Text and first-run size; theme/style approximation | Geometry only; original cells/styles preserved |
| Charts | Native bar/column/line plus embedded editable XLSX | Supported cached categories/values | Geometry only; original chart/workbook preserved |
| Pictures | PNG/JPEG, alt text, crop and geometry | Bounded embedded PNG/JPEG | Geometry only; original image preserved |
| Groups | Native nested groups, maximum depth 8 | Zero-origin supported transforms | Read-only; preserved |
| Connectors | Native lines/arrows with sibling shape references | Supported line geometry and references | Read-only; preserved |
| Masters/themes | Minimal authoring master/layout/theme | Inherited master content may be absent | Preserved, not editable |
| SmartArt, OLE, audio/video, effects, unknown extensions | Not authored | Preserved but not executed; may be absent from preview | No editing or activation |
| Slide ordering | New scenes support reorder/duplicate | Original order resolved through root slide list | Original package slide structure is read-only |
| Notes | Native speaker notes and source attribution | Best-effort plain text | Original notes preserved |

All import warnings must remain visible. Preview success can mean zero supported visual objects; it is not a visual-fidelity score. Source packages are retained and no-op saves return their exact bytes. Supported patches preserve every non-target part payload. Unsupported edits fail rather than silently regenerating a lossy copy.

## Evidence Intake

| Format | Supported behavior | Limitations |
| --- | --- | --- |
| CSV | Header/record parsing, original strings and record/column locators | UTF-8; no delimiter guessing or numeric imputation |
| JSON | Scalar record arrays or header/row matrices, JSON pointers | Nested values rejected; missing fields remain null |
| XLSX | Bounded sheets, cached values, sheet/cell locators | No formula evaluation, external relationships or macros; dates require review |
| Text/Markdown | Plain text evidence | Never rendered as executable HTML |
| PDF | Bounded text extraction with page locators | No page rendering or table inference; scanned pages need separate image input |
| PNG/JPEG | Decode validation and raster metadata; optional Windows OCR | Installed OCR language required; words/regions remain unverified |

Intake is capped at 2 MiB input; rasters at 1 MiB, 4096 pixels per dimension and a decoder allocation budget. Source tables allow at most 1000 rows, 32 columns, 8 sheets and 16000 total data cells. Data-report mapping selects 1-32 rows and 1-6 numeric series explicitly. XLSX cells/dimensions are preflighted before allocating ranges. PDF loading and extraction use per-stream limits; this is not an OS-level process sandbox.

## Safety And Portability

- JSON request/response: 4 MiB; revisioned content: 2 MiB. Import document capacity is lower than the raw PPTX envelope because the original archive and preview are retained together.
- Scenes: 1-32 slides at 1280x720; 256 nodes per slide, 2048 total. Simple inspection/roundtrip supports up to 256 slides. Group coordinate transforms preserve child units.
- New project export checks geometry, source bindings, measured text overflow and missing glyphs. Font fallback is reported. Neither chart-label fitting nor Office-native text parity is proven by these checks.
- Raw low-level `export` and deterministic CLI `generate` are structural tools; they do not imply the project-level measured-layout gate.
- Publication uses exclusive file creation; no original/source file is overwritten. Two-file publication is not crash-atomic. Existing destinations are rejected before publication; racing collisions are reported as partial publication without deleting public paths.
- Hashes, source locators and inverse receipts provide integrity and concurrency checks, not authentication. Checkpoints retain local source content and must be protected accordingly.
- Model HTTP and in-flight CLI requests support cancellation. Native blocking OCR/PDF work has bounded input but is not a hard-cancellable, isolated worker process.
- Current native qualification is Windows 11 ARM64 running an x64 GNU Tauri build, with a native ARM64 CPU model server. Native ARM64/MSVC application builds, macOS/Linux distribution and signed installers remain unqualified.

ZIP64, encrypted/legacy presentations, non-UTF-8 edited XML and signed-package edits are rejected. Source OPC relationships are never fetched and imported content is never executed. PowerPoint checks use disposable copies and are optional, never a runtime dependency.