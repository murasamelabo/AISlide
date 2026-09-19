# Authoring Verification

The 2026-09-13 authoring expansion addresses the missing direct canvas editor and adds native designs and more object types. It builds on, but does not replace, the earlier [research PoC evidence](poc.md).

## User Operations

- Double-click a text box, preset shape, table or group. Text and immediate group labels are plain-text editors; tables have cell editors plus row/column add/remove controls. Ctrl/Cmd+Enter or the check icon applies; Escape cancels outside IME composition. Failed commits retain the draft.
- Select an object to drag/move it or use its resize handle. Arrow keys move; the focused resize handle changes width/height. Shift uses ten-pixel steps. Geometry remains inside the 1280x720 scene.
- **Insert objects** exposes 40 preset shapes, text, lines/arrows, tables and nine chart kinds. Existing picture/process tools remain available. Generated chart examples are labeled synthetic.
- **Edit theme** changes twelve OOXML color slots, heading/body fonts and East Asian/complex-script fallback families. Matching old palette values become live theme references; custom colors remain custom. Fonts are neither downloaded nor embedded.
- **Edit masters and layouts** opens a canvas with common text, rectangles and PNG/JPEG logos, editable geometry and text formatting, master/layout creation and duplication, and layout placeholders. Deleting a design used by slides is disabled. Saving uses a shared transaction.
- **Slide layout** assigns a template. **Reset layout** restores its geometry and formatting, preserving matching text. Direct content edits remain inherited; individual geometry/style edits detach. Obsolete placeholder text survives layout switches/template deletion as independent content.

Freeform objects are preserved when applying a layout, not automatically rearranged. Opaque/freeform content can cover master objects just as it can in PowerPoint. Review resulting placement. The preview approximates preset outlines and shape-specific text regions; only whole-frame text styling is supported. Dates/page-number markers do not generate automatic fields. Arbitrary imported masters/themes and unsupported object edits remain read-only; use an authored scene or matching `.aislide.json` project.

## Native Evidence

Generate a new fixture without overwriting earlier artifacts:

```sh
npm run core:build
node tools/authoring-demo.mjs
```

The verified fixture is `.artifacts/authoring-1789279503051/authoring.pptx`, SHA-256 `0629898b7840fb9056fea39c77d3f1eb0a336bd3f971d53a5ea8f1a7791ec13c`.

- 18 slides; 40 preset shapes; nine chart types; one table; one slide picture plus a common master logo; one group; two connectors; 18 note text frames.
- Two native masters and five native layouts. All 18 slides rendered in Microsoft 365 PowerPoint, and all nine embedded workbooks accepted a cell edit on a disposable copy.
- A second native check moved a layout placeholder 18 points and increased its font by 3 points: the slide inherited both changes without replacing its text. Editing the theme changed a native shape outline. Master footer edits rendered on the slide.
- Source SHA-256 stayed unchanged after both checks. PowerPoint was used only as a verification adapter on disposable copies, not as an application runtime dependency.
- Official Open XML SDK validation passed for the initial and final fixtures. The initial schema check did not detect a master/layout ID collision that PowerPoint rejected. The corrected writer allocates nonoverlapping master/layout IDs and separate theme parts; regression tests enforce those contracts.

```powershell
./tools/validate-openxml.ps1 -Path <generated-authoring.pptx>
./tools/verify-powerpoint.ps1 -Path <generated-authoring.pptx> -ExpectedSlides 18 -ExpectedCharts 9 -ExpectedPictures 1 -ExpectedGroups 1 -ExpectedConnectors 2 -VerifyChartData -CaptureSlides (1..18)
./tools/verify-authoring.ps1 -Path <generated-authoring.pptx>
```

The optional `--single-master` fixture mode changes only master topology for compatibility diagnosis. Synthetic values are not empirical results, and these selected native checks do not establish universal Office visual parity.

## Automated Evidence

The final local gate passed **135 distinct tests**: Rust workspace 85, Node bridge/SDK/MCP/intake/generation 20, Studio browser 23, generation browser 4, native request unit tests 2 and actual Tauri WebView workflow 1. Studio build, Tauri build, lint and the 131-file encoding check also passed. A pre-existing VS Code notice about the remote Tauri configuration schema being untrusted does not affect the successful native build; no editor trust settings were changed.

- Rust tests cover master/layout/theme validation, native parts/relationships, color binding, template propagation, detached overrides, obsolete placeholders, all new chart structures/workbooks, object factory limits and imported new-field rejection.
- SDK/MCP tests execute object insertion, theme/design changes, layout assignment, shape-text edits, undo, stale-revision rejection and project reopen.
- Browser tests cover direct multiline/IME/cancel/undo, table cells and row/column edits, group text, resize, theme colors and common master text. All nine chart kinds rendered without JavaScript errors.
- Authoring dialogs were checked at 1440px and 390px: no horizontal overflow and no serious/critical axe findings for the targeted WCAG tags. Screenshots are stored under ignored `.artifacts/authoring-*`. This is not a complete manual accessibility or baseline-image-diff audit.
- The actual rebuilt Tauri WebView passed direct text/IME editing, object insertion, common master text, theme/layout changes and undo, followed by existing source and model-fixture generation/cancellation checks. It used an owned process and disposable profile; the user's existing window was not closed.

Review agents had no filesystem access. They reviewed supplied critical excerpts rather than independently auditing the whole repository. Confirmed findings about Escape during commit, template-only inheritance flags and duplicate slide-placeholder indices were reproduced and fixed. Stateless core transports return candidate documents rather than committing remote server state; SDK late-cancellation rejection therefore does not leave a persisted server mutation behind. No measured code-coverage percentage or full security-audit claim is made.

Hosted CI remains blocked by the repository account billing/spending restriction. No public release, billing change, installer signing or project license choice was made.

## Phase2 Visual Tools, 2026-09-18

This checkpoint is separate from the historical 2026-09-13 counts above. Only synthetic in-memory fixtures and isolated test browsers are used; no Office, existing presentations, user browser, process termination, git or deployment operations are performed by this phase.

### Geometry

G10 adds quadratic/cubic Bezier boolean operands and **Fragment shapes** to the existing Union/Intersect/Subtract/XOR menu and SDK/MCP `combine_shapes`. Curves use bounded adaptive subdivision with a maximum control-hull-to-chord distance of **0.25 slide pixels**, at most 20 levels, before clipping with the existing `geo 0.33.1 / i_overlay 4.5.2` kernel. Output is straight polygon geometry, not exact Bezier curves. Native coordinate quantization adds rounding to 1/1,000,000 of output bounds.

Fragment is an actual area partition: each new operand splits existing areas into difference/intersection plus previously uncovered remainder; disconnected components become separate objects. IDs are `result_id`, `result_id-2`, etc.; collisions or invalid generated IDs reject the entire transaction. All outputs use the first supplied object's style. Up to 32 operands, 128 output regions, 4096 aggregate input/output vertices, 256 commands per compound path and 262144 edge-pair clipping work remain explicit budgets. Holes retain opposite winding. Degenerate, crossing/touching compound contours, transforms, hidden/locked objects, nonrepresentable effects/gradients, text-bearing shapes, dangling connections and unknown native content reject. Undo restores removed objects and metadata atomically.

### Styles And Sampling

See [the object painter and slide eyedropper contract](../authoring/proofing-format-painter.md). Native create/reopen/edit/Undo covers rect, polygon, shape, picture, table and chart styles plus retained text behavior. Shapes/paths/crop/data/links/alt remain target-owned. A missing or incompatible target rejects the entire multi-target operation. Additional WordArt presets are **not implemented**: `render.rs` currently rejects all text warp in static output, and expanding that shared renderer requires parent coordination. The existing three presets and approximate Studio previews remain unchanged; no general Office warp claim is made.

### SVG And Metafile Import

G18 accepts literal SVG `text`/`tspan` using locally installed fonts and `image` containing exact `data:image/png;base64,...` or `data:image/jpeg;base64,...` references. The original inert SVG is retained in PPTX with a locally generated PNG fallback. SVG bytes are capped at 256 KiB, XML at 2048 elements/depth 32, text at 16000 characters, embedded images at eight and 16 million aggregate decoded pixels. Each raster passes existing MIME/magic/4096px/64MiB decode guards. System font substitution is possible; font embedding and Office text-layout parity are not promised. No font download occurs.

SVG scripts, style elements/attributes, external fonts/images/paint, string image resolvers, nested SVG data images, processing instructions, DTD/entities, `use`, filters and unsupported attributes remain rejected. Both the preflight allowlist and the `usvg` image resolver disallow file/network loading. Local gradient/mask/clip references retain cycle/depth/expansion checks. Literal text is not interpreted as markup or commands.

`create_asset` and `create_graph_icon` accept **image/emf** and **image/wmf** through explicit inert conversion to SVG, not native OS playback. Studio imports `.emf/.wmf` through the shared file chooser. Original metafile bytes are not retained as an editable metafile; converted SVG plus PNG is the resulting native picture. The bounded subset is:

- EMF: exact 88-byte v1 header, no description/palette, EOF, select/create/delete solid/null pen or brush, rectangle, ellipse, MoveTo/LineTo, 32-bit polygon/polyline and alternate/winding fill mode. Stock objects: white/black/null brush and white/black/null pen only.
- WMF: placeable header with verified checksum plus 18-byte version-0x300 header, matching total/max record lengths; EOF, create/select/delete solid/null pen/brush, rectangle, ellipse, MoveTo/LineTo, polygon/polyline and fill mode.
- At most 256 KiB, 1024 records, 256 handles, 4096 points; positive bounds at most 4096 units per dimension; all geometry stays in bounds; solid/null pen width 0..100 (zero becomes one unit). No transforms, map-mode/window/viewport changes, text/font records, bitmaps, escapes, comments, EMF+, clipping or arbitrary records. Unknown/trailing/truncated records fail closed rather than being skipped. Stroke and font parity with GDI/Office is not asserted.

### Dependencies And Checks

No new PPTX engine or project license was introduced. Enabling `resvg 0.48.1` text/system-font features added these resolved generic dependencies: `fontdb 0.24.0` (MIT), `harfrust 0.12.0` (MIT), `unicode-vo 0.1.0` (MIT/Apache-2.0), `base64 0.23.1` (MIT OR Apache-2.0). Installed crate manifests were checked. Existing `resvg/usvg` are Apache-2.0 OR MIT; existing `geo/i_overlay` are MIT OR Apache-2.0. Fonts themselves remain installed-system resources and are not redistributed.

Focused commands: `node tools/cargo.mjs test -p aislide-core --test geometry_ops --test proofing --test visual`; `node tools/cargo.mjs test -p aislide-core --lib media::tests`; `node --test --test-concurrency=1 tools/geometry.test.mjs tools/proofing.test.mjs`; `playwright test tests/e2e/editing-workflows.spec.ts --grep 'phase2|G10 combination'` with a freshly allocated `AISLIDE_TEST_PORT`. Latest focused Node: **8 passed**. Studio: **5 passed** (1440/390 five-operation menu, object painter/eyedropper with pointer/numeric/swatch/HEX, SVG/EMF import/Undo), plus the two eyedropper cases rerun for opened-panel screenshots. Build and lint passed with three existing Fast Refresh warnings.

Final serialized gates:

- `node tools/cargo.mjs test --workspace --locked --offline`: **457 passed, 0 failed, 5 opt-in ignored, 44 suite summaries**, exit 0. Actual log and summary: `.artifacts/phase2-gates-PtncvJ/core.log` and `core-summary.json`. The touched suites in that gate passed geometry 14, proofing 9, visual 18 (plus one opt-in ignored), and media 6 tests. The full gate includes unrelated parent changes already present in the workspace and does not attribute all 457 tests to this phase.
- `node tools/cargo.mjs build -p aislide-cli`: passed after the final media change. `node --test --test-concurrency=1 tools/geometry.test.mjs tools/proofing.test.mjs`: **8/8 passed**, including WMF/EMF/SVG native edit/Undo and graph-icon preparation. `node --test --test-concurrency=1 tools/client.test.mjs tools/mcp.test.mjs`: **26/26 passed**.
- Both complete affected browser files (`editing-workflows.spec.ts`, `proofing.spec.ts`), fresh port 55930: **25 passed, 1 failed**, no retry/skip. The neighboring template-replacement test observed its unchanged four objects while the operation was still busy, exceeding the existing five-second assertion. It passed unchanged in isolation on fresh port 53867 (**1/1**). Preserve both reports (`ui.json`, `template-recheck.json` in the same artifact directory); this is not a clean all-pass full browser run. All five phase2 workflows and all three existing proofing/painter scenarios passed the complete-file run. No template code or timeout was changed.
- Targeted diagnostics: no errors. Exactly 25 touched source/document files passed strict UTF-8 with one BOM; four local documentation links resolved; no global reformatting or encoding rewrite was performed. Cargo's generated lockfile keeps its tool-managed encoding.

Pending parent gates: additional WordArt/render coordination, whole-product frozen browser/native/Office qualification, commit/push and desktop installation. No Office compatibility, unrestricted EMF/WMF, comprehensive accessibility or full feature-parity claim follows from these checks.

Final `npm run build` exited 0 (existing >500 kB chunk warning retained); `.artifacts/phase2-gates-PtncvJ/build.log` contains the output. An isolated preview was left at `http://127.0.0.1:50661/`, HTTP 200, without opening a user browser or modifying the desktop installation.

### Phase2 Changed Files

- Core: [geometry_ops.rs](../../crates/aislide-core/src/geometry_ops.rs), [vector.rs](../../crates/aislide-core/src/vector.rs), [visual.rs](../../crates/aislide-core/src/visual.rs), [media.rs](../../crates/aislide-core/src/media.rs), [proofing.rs](../../crates/aislide-core/src/proofing.rs), and the `SampleSlidePixel` addition in [protocol.rs](../../crates/aislide-core/src/protocol.rs).
- Dependency configuration: [core Cargo.toml](../../crates/aislide-core/Cargo.toml), generated root [Cargo.lock](../../Cargo.lock).
- SDK/MCP: [types.ts](../../packages/client/types.ts), [index.mjs](../../packages/client/index.mjs), [index.d.mts](../../packages/client/index.d.mts), [mcp.mjs](../../tools/mcp.mjs).
- Studio: [ObjectToolsPanel.tsx](../../apps/studio/src/ObjectToolsPanel.tsx), [SelectionTools.tsx](../../apps/studio/src/SelectionTools.tsx), [AssetPanel.tsx](../../apps/studio/src/AssetPanel.tsx), [api.ts](../../apps/studio/src/api.ts), [object-tools.css](../../apps/studio/src/object-tools.css).
- Tests: core [geometry_ops.rs](../../crates/aislide-core/tests/geometry_ops.rs), [proofing.rs](../../crates/aislide-core/tests/proofing.rs), [visual.rs](../../crates/aislide-core/tests/visual.rs), Node [geometry.test.mjs](../../tools/geometry.test.mjs), [proofing.test.mjs](../../tools/proofing.test.mjs), browser [editing-workflows.spec.ts](../../tests/e2e/editing-workflows.spec.ts). Media unit tests live in the existing core module.
- Documentation: this guide, [proofing-format-painter.md](../authoring/proofing-format-painter.md), and **only phase2's row** in [editing-completion.md](../planning/editing-completion.md). The shared static renderer, PDF owner files, image-edit AI core, desktop installer and git state were not edited or operated by this phase.