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