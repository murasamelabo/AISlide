# Blank Startup, Icon Categories And Master Presets

Date: 2026-09-16. These changes are local and uncommitted. They do not authorize a source push, binary publication, signing or a project-license choice.

## User Workflow

Studio starts with one empty slide named `Slide 1` in `Untitled presentation`. No sample report is requested at startup, and the untouched blank document is not marked dirty. **New report** remains the explicit command for the synthetic twelve-slide example. **New presentation** still creates another blank document with the existing unsaved-change protection.

**Insert icons > Category** filters the complete Lucide React 1.43.0 catalog: 1,818 canonical icons in 42 official categories. Category and keyword search intersect, and either change resets the 60-item page to its beginning. Icons can belong to multiple categories; Database is in both Devices and Coding & development. The same picker and filter are used for architecture-node icons. Existing Japanese search tags remain available alongside the official English tags.

For a master preset, open **Edit masters and layouts > Browse presets**, select a style, preview its layouts and inspect the color swatches, font names, side margins, gutter and content-region coordinates. **Use preset** applies the design in one document transaction. Then choose **Slide layout** to add the appropriate native placeholders. New slides use the active preset's blank layout. Undo restores the prior design and slide content.

## Preset Definitions

All measurements below use the existing 1280 x 720 slide coordinate space. Side margin means the left/right content inset, not every edge or every cover ornament. Heading/body sizes are content-layout defaults; cover and section headings have larger explicit sizes.

| ID | Style | Main Accent | Side Margin | Gutter | Heading / Body | Heading Font | Body Font |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `public` | Civic clarity | `#0017C1` | 64 | 48 | 42 / 26 | Noto Sans JP | Noto Sans JP |
| `minimal` | Minimal space | `#007A74` | 96 | 56 | 40 / 24 | Noto Sans JP | Noto Sans JP |
| `stylish` | Studio editorial | `#006B68` | 72 | 40 | 48 / 24 | IBM Plex Sans | Noto Sans JP |
| `pop` | Color play | `#C72B67` | 56 | 32 | 48 / 26 | Noto Sans JP | Noto Sans JP |
| `dynamic` | Bold momentum | `#C92D32` | 48 | 40 | 60 / 26 | IBM Plex Sans | Noto Sans JP |
| `trust` | Trusted report | `#1D5093` | 72 | 48 | 42 / 26 | IBM Plex Sans | Noto Sans JP |
| `luxury` | Quiet prestige | `#7A6435` | 104 | 64 | 44 / 24 | Yu Mincho | Yu Gothic |

Each preset defines all twelve OOXML color roles, heading/body/East Asian/complex-script font names, one master, seven layouts and explicit content regions. Noto Sans JP and IBM Plex Sans are locally bundled for browser rendering. Yu Mincho and Yu Gothic are requested system fonts, not redistributed or embedded Office fonts. East Asian text uses Noto Sans JP except in Quiet prestige, which specifies Yu Mincho. Office and other machines can substitute unavailable fonts.

The styles differ in layout treatment as well as color: restrained Civic rules, wider Minimal whitespace, a dark editorial cover with a vertical teal panel, a light Pop cover with distinct accent blocks, a red Dynamic cover with larger type, blue Trust hierarchy, and centered Mincho typography with thin rules for Quiet prestige. No company logo, source slide, screenshot or proprietary artwork is copied.

## Layouts And Placement

Every preset contains these editable native layouts:

- `preset-blank`: empty slide-content layer with the preset's common master graphics.
- `preset-cover`: title and subtitle with style-specific placement, scale and background.
- `preset-title-content`: title and a full-width body placeholder.
- `preset-two-columns`: title and two body placeholders separated by the preset gutter.
- `preset-section`: a large section title over the accent background.
- `preset-three-cards`: title and three native content panels with separately editable body placeholders.
- `preset-visual-content`: title, left-side body and a right-side native visual region for a chart, graphic or part.

Content headings begin at y=64 in a 112px-high frame. Body regions begin at y=192, or y=208 for Bold momentum, and end at y=640. Column and panel widths are derived from the side margins and gutter, not independently guessed. Region coordinates are returned by the shared core and displayed in the preset browser.

New metadata parts and architecture graphs inserted into **Text and visual** fit inside its `preset-visual-region` with their aspect ratio retained. Their child coordinates, font sizes and line widths are normalized into the fitted coordinate space, so PowerPoint does not retain full-size text inside a smaller group. Updating a part retains that coordinate space and its outer placement. Dense parts can have small labels after fitting; use a wider layout or fewer items when needed. This is not automatic text reflow or automatic placement for every object type. Existing objects are not rearranged.

## Preservation And Limits

Preset application retains the original masters/layouts and existing slide text, notes and objects. It updates the dedicated preset master and recognized preset layout roles, while inherited placeholder geometry and style follow the existing design engine. Color values matching the previous theme become theme references; unrelated custom colors and raster image colors remain fixed.

A default authored deck has two masters and eleven layouts after applying a preset: its original one/four plus the preset one/seven. Switching styles replaces that preset set instead of repeatedly adding masters. Custom elements added alongside preset elements and additional custom layouts remain. Edited or removed generated template elements, foreign master IDs, conflicting custom IDs and insufficient capacity fail before committing; they are not silently overwritten. Numerical comparison allows only the small coordinate rounding required by native PPTX serialization.

The existing limits remain eight masters and thirty-two layouts. Reopened native packages cannot gain arbitrary new master/layout parts. Consequently, a presentation without an existing preset master must receive its preset before first export, or be created anew; an already preset-based PPTX supports compatible preset replacement after reopening. Unsupported native or imported changes still fail closed. A saved preset is ordinary PresentationML, not a cached scene that overrides PowerPoint edits.

## Shared Interfaces

- Core `design_presets` returns the seven `{id, name, design, rules}` entries.
- Core `apply_design_preset` accepts `{deck, preset_id}` and returns a candidate deck; normal document transactions provide revision checking and Undo.
- SDK `client.designPresets()` and `session.applyDesignPreset(id, options)` share that implementation.
- MCP `design_presets` returns `{presets}`. `apply_design_preset` takes `deck_id`, `expected_revision` and `preset_id` and uses the same session path.

Use [the shared API reference](../api.md) and [SDK guide](../../packages/client/README.md) for the existing document contracts.

## Reproducible Samples

`node tools/master-presets-demo.mjs` creates a new directory under `.artifacts/` with seven PPTX files, one for each style, each containing seven layouts. It adds Japanese/English demonstration text and a native flow part, validates export, reopens each file, checks part metadata and tests compatible preset replacement. A manifest records the exact palette, fonts, rules, bytes and SHA-256 for every file. It never overwrites a prior sample or the user's presentation.

The current corrected samples are under `.artifacts/master-presets-TjlBv4/`. Earlier `.artifacts/master-presets-BXp44U/` samples exposed PowerPoint retaining original font sizes when only a part's outer group was scaled; they are not the final samples. The fitted-canvas regression now covers header/subtitle spacing, native reopen and subsequent updates. PowerPoint inspection uses disposable copies and verifies the source hash afterward, never the source file itself.

## Verified Results

This checkpoint has **251 distinct passing tests**: Rust workspace/CLI 140; Node bridge, SDK, MCP, extraction, generation and icon metadata 33; Studio browser 64; generation browser 4; native unit 4; native WebView 1; and setup configuration/installed lifecycle 5. The full browser run passed 62 cases; two keyboard-focus checks needed actual Tab navigation after the sample fixture clicked Save, then both passed unchanged application code. The native test likewise gained an explicit unsaved-content confirmation after creating the sample. No protection or focus policy was weakened.

After the final fitted-canvas change, all 140 core tests, the 16 affected SDK/MCP tests, the native build/unit/workflow and the installed setup test were rerun successfully. The seven corrected sample decks, totaling 49 slides, exported and reopened with valid part metadata. Browser preview checks traversed all seven layouts of all seven presets, checked typography and text fit, and tested category filters and preset controls at desktop/mobile widths.

PowerPoint opened and rendered all 49 original sample slides using disposable copies. Inspection found a group-scaling header/subtitle overlap; the corrected canvas normalization was verified with a newly generated Quiet prestige sample in PowerPoint, including the affected visual-content slide. Its source SHA-256 remained `a3482bb3d09b13391a90ccb109643d0adb84d01c8b14437f21d5d918eb37a336`. The final visual-content capture was inspected and the overlap was absent. This does not claim full Office visual parity, exhaustive editing of every master, connector-motion qualification or schema validation of every sample.

The frontend, normal embedded desktop and NSIS builds passed. The all-icon/category panel remains lazy-loaded and is 1,040.67 kB minified, 268.69 kB gzip; Vite's normal large-chunk warning remains visible and its threshold was not raised. These are size measurements, not latency guarantees. Scoped React/Rust/security reviews were limited to supplied excerpts or descriptions; they are not independent full-source audits. The review-identified preset ownership, capacity and stale-decoration cases were reproduced and fixed before final validation.

The [updated installer](windows-setup.md#blank-and-master-refresh) is unsigned and development-only. Existing application windows and source presentations were not updated or closed by this task. No commit or push was performed.

## Category Provenance

[The generator](../../tools/lucide-categories.mjs) reads JSON metadata from the official Lucide `1.43.0` source archive. Its pinned SHA-256 is `f218860ba3a517abbbdb3a2e183a4ff15cc1e193d8e1283b21787f10226cc2d5`; the archive is checked before extracting only the icon/category metadata and license. The application bundles the generated data locally and makes no external category request at runtime. `npm run test:icons` checks installed-version agreement, every canonical icon, every category reference and the upstream license. CI runs this offline check. Updating Lucide requires regenerating and reviewing matching metadata.

The [redistributed license](../../apps/studio/public/lucide-LICENSE.txt) preserves Lucide's ISC and Feather-derived MIT notices. Font licensing and source-asset licensing remain separate from the project's unselected license.

## References

- [Digital Agency Design System](https://www.digital.go.jp/policies/servicedesign/designsystem), [typography](https://design.digital.go.jp/dads/foundations/typography/) and [layout](https://design.digital.go.jp/dads/foundations/layout/): readability, font roles, grid, margins and gutters.
- Slideland taste collections: [minimal](https://www.slideland.tech/docs/taste/minimal), [stylish](https://www.slideland.tech/docs/taste/cool), [pop](https://www.slideland.tech/docs/taste/pop), [dynamic](https://www.slideland.tech/docs/taste/dynamic), [trust](https://www.slideland.tech/docs/taste/trust) and [luxury](https://www.slideland.tech/docs/taste/luxury).

The collections informed broad mood choices only. The presets are independently authored deterministic designs, not AI-generated artwork, copies of the referenced presentations, Digital Agency certification or general Office visual-parity guarantees.