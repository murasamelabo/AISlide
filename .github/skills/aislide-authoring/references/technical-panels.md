# Technical Briefing Style

Use this reference when the user requests a technical briefing with keyword
headings, restrained color, meaningful icons, tinted panels, icon cards, role
shifts and step cards. It is an explicit visual choice, not the default for
every presentation.

## Fixed Visual Contract

- For `component_profile:"technical_briefing"`, obtain `component_styles` and use
  its shared frames. Short readings and takeaways are open text, not mandatory
  tinted rectangles. The reference chrome below is an explicit alternative;
  do not mix its geometry with a component profile or an imported template.
- Select one installed Japanese-capable font at presentation creation. Keep the
  same family across headings/body/notes and inspect fallback warnings. Font names
  do not install or license fonts.
- Use a white 1280x720 canvas, a small blue section label, a keyword heading and
  a thin header rule. Avoid a second long summary line repeating the heading.
- Reference chrome: section at (108,20), 12px; heading at (108,44), 32px;
  header icon at (56,42), 32x32, within a 48px pale circle; rule at y=98;
  source/footer at (48,680), 11px. Keep necessary caveats visible.
- For customer/PDF distribution call `set_references` with explicitly approved
  `publish:true` URLs and stable slide IDs; inspect `publication.excluded`.
  Reserve x32..1248/y616..672 for 16px reference markers above source/page labels
  at y680. Use a body bottom at or above y608 for this reserved-band variant.
  Markers search only the bottom quarter with at least 8px clearance. For dense
  imported slides explicitly choose `placement:"appendix_only"`: no marker or
  footnote is added to the cited slide; a separate appendix caption shows current
  1-based deck page numbers without copying private titles or notes. No automatic
  fallback or body shrinking occurs. Caption frames reserve maximum supported
  number widths and stay fixed through reorder, deletion and appendix movement.
  Ordinary edits keep reference placement and page order; resolve
  `REFERENCE_COLLISION` through body edits or explicit `set_references`.
  Include appendix pages in the exported PDF; subset PDFs retain full-deck
  numbers, not subset-local numbering. Never rely on notes alone.
- Use left-aligned body text, nominally 18px, in a fixed content region. Do not
  shrink text to make an overfull panel fit. Shorten with approval, enlarge the
  region, or split the explanation while preserving required content.
- Fix semantic color roles. The comparison defaults use a neutral pale left
  panel/lavender heading and pale green right panel/teal heading. Color is not
  evidence that an alternative is better. Override colors explicitly when needed.
- Icons must represent the row's meaning, not fill empty space. Reuse an installed
  icon library or approved image assets. Do not redraw source screenshots as icons.

## Choose By Relationship

<a id="selection-discriminators"></a>
### Selection Discriminators

Start with the reader question and the actual supplied relation, not the item
count or a favorite preset. Select the primary visual before resolving slots.
Compare at most three plausible candidates and record the selected reason and
why the nearest alternative is worse for this page. These are short decision
records, not a request for private chain-of-thought or a particular model's mind.

| Nearest choices | Prefer the first when | Prefer the second when | Reject or verify |
| --- | --- | --- | --- |
| Managed graph / layer stack | Subjects, boundaries, generation, dependency or communication must be connected | Supplied categories really are ordered or layered | A definition and tenant instances are not layers; label what each arrow means |
| Native table / `contrast/panels` | Alternatives share explicit criteria or comparable values | Two approaches need paired explanatory statements | Do not repeat the same labels in a diagram and table without adding a distinct reading task |
| `icon-columns` / `icon-cards` | Equal peers have short comparable descriptions | Each item bundles several attributes, points or a status | Cards are not a default for unrelated relations or a way to fill white space |
| `open-steps` / `vertical-flow/rail` | Short ordered actions should be seen together | Longer explanations need sequential vertical reading | Do not turn equal concepts into a causal sequence |
| Ordered steps / `roadmap` | The meaning is operation order | Supplied phase, action and outcome must be associated | Roadmap spacing is not elapsed-time proportion; do not invent dates |
| Full-width image above readings / image beside readings | A wide original diagram needs width to trace labels and paths | The source remains legible beside short explanatory text | Compare actual source-label size and aspect, not only whether the image fits |
| `screenshot-callouts` / `source-reading` | The reader must locate exact controls or states in a screenshot | The reader needs interpretation of the whole diagram | Positions come from the approved image; never guess coordinates or silently redraw it |

Use this order: question -> supplied relation -> primary visual -> viewing and
content constraints -> candidate comparison -> shared style roles -> resolved
layout -> representative image review. Multi-panel parts may occupy one full
visual slot; `focus/full` alone is not a semantic choice or evidence of quality.

### Contrastive Examples

The following are editorial examples, not new facts or prescribed deck outlines.

- Definition and instances: use one definition connected to supplied instances
  with the same relationship label. A stack plus a table repeating both names
  is weaker when there is no ordered hierarchy. Use `entity-instance-map` only
  for its bounded one-to-many contract; arbitrary architecture remains a graph.
- Wide protocol figure: keep the original large above two or three readings
  when tracing its arrows is the reader task. A narrow image beside redundant
  prose is unsuitable when its labels become unreadable. `source-reading`'s
  `visual-top` uses `media/figure-readings` with compact open readings. Compare
  actual contained image width, not slot width. Recommendations use the standard
  1280x720 technical profile and require a 10% gain; preview the actual deck.
- Settings screen: map supplied relative positions to callout explanations.
  A complete screenshot with small unreadable labels and an unanchored list is
  insufficient for locating a setting. `screenshot-reading` uses the existing
  managed callout part. Its `detail_views:true` keeps the complete original plus
  1-3 enlarged source crops, with numbers outside the original. The technical
  recipe enables this mode. Split pages if detail plus explanation cannot fit;
  do not silently fall back to an unreadable full-screen image.
- Three short peers: use open icon columns. Cards become useful when each peer
  also needs points or a status. More containers or more part kinds are not
  inherently better. Preserve a repeated layout when the same comparison is
  intentionally shown on several pages.

### Shared Components And Acceptance

Resolve `component_styles` once for the document before adding content. The
optional `technical_briefing` profile is shared by pattern composition and
recipes; its title/body/takeaway/source/page roles also guide manual elements.
Keep family, size, frame, padding, alignment and fill consistent by role. Give
conditional takeaways an explicit primary/context variant without dropping
conditions. Do not force cover, divider, source appendix and body diagrams to
share the same role.

### Containment And Emphasis

- A top or left accent strip is not a default component. Start with open text,
  aligned headings, meaningful icons and measured spacing. Keep a container when
  it communicates a real boundary, warning, comparison or bundled attributes.
- A one-sentence takeaway is a compact emphasized line, not a large empty band.
  Distinguish it from the source with a boundary or a deliberate container.
  Keep conditions nearby in a secondary role; do not hide them to simplify it.
- Specify important words as rich-text runs with deliberate bold/color. Preserve
  normal body text around them. Do not auto-highlight guessed keywords or color
  every sentence. A message slot accepts `kind:"text"` and explicit paragraphs.
- Use `list-horizontal/icon-columns` for short peers, with supplied Lucide icons
  that name their meaning. Do not put an icon in every box or add one just to
  compensate for empty space. Use cards when several attributes truly belong
  together, not for every sentence or every page section.
- Match content area to measured text and source media. Do not stretch a small
  note across a large rectangle, remove useful whitespace mechanically, or vary
  layouts only for novelty. Review density and grouping across adjacent pages.

Use `component_styles.emphasis_variants` to choose `inline`, `rule`, `outline`,
`tint`, `closing` or `solid`. `auto` uses supplied `strength` (`subtle`, `normal`,
`strong`) and `relationship` (`supporting`, `standalone`, `contrast`,
`conditional`): subtle context stays inline; a normal supporting conclusion or a
conditional qualifier gets a thin separator; a strong qualifier, standalone or
contrasting content gets an outline; a strong supporting point gets a tinted
surface. A qualifier never gets the tint automatically. `closing` and `solid` are
explicit only. Explicit `style` overrides this mapping.
These are display decisions, not inferred severity or semantic verification.
Give each slide at most one conclusion and choose its placement with
`component_styles.message_placements`: `pattern.lead:true` with a `lead-message`
slot for a claim read first (style `inline` or `rule` only), a bottom
`message_band` with `closing` or `solid` for a conclusion after the body, a
qualifier folded into its row with `detail_emphasis` on `list/icon-rows`, or no
summary when the title already states the claim. Do not combine a lead with a
closing line or solid band; use `solid` on a few summary slides only, and mark
phrases on it with bold rather than the accent color. The full decision table is
in `docs/authoring/technical-explainer.md`.
Recipes accept `takeaway_emphasis`; a pattern slot accepts `kind:"emphasis"`
with rich `paragraphs` and `emphasis`. Manual additions use the resolved variant
frames unchanged. Preserve explicit words, icons, conditions and source hierarchy.
Place a selected surface or separator before its text in the actual element
order; accessibility reading order is not the paint order. Inspect rendered
text visibility even when geometry and content-preservation checks pass.

For precise screen borders, supply `target_region:{x,y,width,height}` in 0-1
original-image fractions. It must contain the callout point. Core uses the same
region for the original outline, enlarged outline and leader attachment. A crop
extent is not automatically the UI control boundary. Oversized regions require
more detail area or split pages, not silently reduced zoom.

Accept a representative page only after checking: the reader can trace the
required relationship, the primary visual is evident, source labels are readable
at delivery size, callout explanations match locations, necessary caveats remain
visible, repeated text adds a different function, and equal roles use equal
formatting. Record a page-specific failure and the smallest correction. Automated
geometry and source-byte checks do not prove these human-reading properties.

| Information | Representation |
| --- | --- |
| Two states or approaches explained through paired statements | `contrast/panels` |
| 2-4 short equal-status concepts, each with icon and explanation | `list-horizontal/icon-columns` |
| 2-6 peers that each bundle multiple attributes, points or status | `list-horizontal/icon-cards` |
| 2-6 stacked statements, risks or capabilities with detail | `list/icon-rows` |
| 2-5 explicit from-to changes (roles, metrics, operating model) | `before-after/shift` |
| 2-4 ordered steps with outcomes, optional supplied images | `flow/cards` |
| Agenda or chapter list with durations | `list/agenda` |
| Numeric scores, common criteria, or three or more alternatives | Matrix/table |
| Architecture, boundaries and handoffs between systems | Managed graph |
| Equal-status topics without icons | Open rows/columns/grid |
| Required product screenshot | Original image plus focused annotation |
| Screenshot with numbered UI callouts | `list-enumeration/screenshot-callouts` |
| Headline metrics with change and status | `list-horizontal/kpi-cards` |
| Metrics against a target and qualitative ranges | `horizontal-bar-graph/bullet` |
| Plan versus actual by line item | `water-fall/variance` |
| Options rated on shared criteria (0-4) | `matrix/harvey-balls` |
| Comparable numbers where hot spots matter | `matrix/heatmap` |
| Responsibilities per task and role | `matrix/raci` |
| Scored risks with actions | `matrix/risk` |
| Few causes explain most occurrences | `vertical-bar-graph/pareto` |
| Process stability over time | `line-graph/control-chart` |
| Candidate causes for one effect | `tree/fishbone` |
| Handoffs between roles or systems | `flow/swimlane` |
| Quantities that split and merge across stages | `flow/sankey` |
| Customer stages, touchpoints and emotions | `flow/journey` |
| Containers inside one system boundary | `correlation/c4-container` |

Plan the page mix by reading task: open columns, rows, shifts, steps, graphs,
required tables and occasional justified cards. Do not convert every page to cards.
In `compose_slide`, the `comparison` block remains a matrix. To request these
visuals, use a `part` block with the preset (pass `show_title: false` on the
`part` block to omit the 88px internal part title band when `spec.title` is set,
including for `interactive/*` and `diagram/custom`), or insert it with managed
`add_part`.

When the server exposes `layout_patterns` and the user has not asked for free
placement, no layout patterns or an existing template, choose the body pattern
by relationship: `split/1-1` for two approaches, `split/2-1` for a main
image/diagram with explanation, `focus/full` for one overview, `columns/3` for
three peers, `media/screenshot-callouts` for numbered UI screenshots. Resolve it
with `deck_id` and pass slot frames unchanged as `PartSpec.layout` or element
frames. When a slot holds a part, add `part_preset` and copy `part_fit.fit` to
`layout.fit`; to choose a pattern for a shape-sensitive part (cycle, radial,
pyramid, map), search with `part_preset` and prefer the top-ranked stretch
slot. A multi-panel part such as `contrast/panels` fills one slot; do not split
it across slots. When `fits` is false, follow `fallback` or split the content.

## Icons

1. `lucide_icons({query})` searches the installed Lucide library (ISC) by name,
   tag or category. Choose icons for meaning; one icon per concept.
2. `lucide_icon_assets({icons:[{name,color}]})` renders up to 32 icons and
   returns `icon:{asset_id,mime_type,alt}`. Pass that object unchanged to part
   or graph `icon` fields; never generate SVG/base64 through the model.
3. Use the accent color of the card/row as the icon color. Identical renders
   reuse one handle; `close_asset` unused handles because the registry holds 32.

Without supplied icons no decorative stand-ins are generated. Use approved
images for step cards through `register_asset`/`prepare_assets`.

## Briefing Part Contracts

Fetch `part_catalog({preset_id})` and the complete `apply_operations` schema.
A typical body frame below the reference chrome is
`"layout":{"x":48,"y":120,"width":1184,"height":540,"show_title":false}`.
For the reserved reference-band variant, use height 488 (body ends at y608)
instead of 540. Longer reference markers may require a taller reserved band.

- These parts render at the layout frame size; they are not scaled. Fonts are
  fixed: headings 18px, body `body_size` 14-22 (default 16), captions, tags and
  pills 12-13px. Overflow rejects with the failing text; shorten it with
  approval, enlarge the frame or split the slide. Run preflight with
  `min_font_size:12` for this style or omit captions/tags.
- Cards and steps shrink to their measured content height and rows use at
  most 112px pitch, top-aligned. Remaining frame space stays empty; size the
  frame or add a separate element instead of stretching content. Step
  chevrons align with supplied images, otherwise with the step pills.
- Optional `message:{text,detail?,fill?,color?}` adds a takeaway band directly
  below the content (default dark `@dk1` with `@lt1`, e.g. `color:"D9F0A3"`;
  a pale `fill` with a matching dark `color` for cautions). Not on agenda.
- Labels allow at most three LF lines. Colors are hex or theme keys.

| Preset | `data.kind` and fields |
| --- | --- |
| `list-horizontal/icon-cards` | `icon_cards`: `cards` 2-6 `{label<=48, caption<=64, detail<=240, points<=4x80, tag<=40, icon, accent}`, `columns` 2-4 (max two rows), `numbered`, `body_size`, `message` |
| `list/icon-rows` | `icon_rows`: `rows` 2-6 `{label<=60, detail<=200, icon, accent}`, `boxed`, `body_size`, `message` |
| `before-after/shift` | `shift_rows`: `rows` 2-5 `{from<=48, to<=48, caption<=64, detail<=200}`, `from_label`/`to_label` <=24, `accent`, `body_size`, `message` |
| `flow/cards` | `step_cards`: `steps` 2-4 `{label<=48, detail<=240, points<=4x80, outcome<=64, image, icon}`, `step_label` <=12 (default STEP), `accent`, `body_size`, `message` |
| `list/agenda` | `agenda`: `items` 2-7 `{label<=60, detail<=120, meta<=16}`, `accent` (number badge, default `@dk1`) |
| `list-enumeration/screenshot-callouts` | `screenshot_callouts`: `image` (registered screenshot), `callouts` 1-6 `{x, y, label<=48, detail<=200}` with `x`/`y` as 0-1 fractions of the source image, optional `detail_views:true` for 1-3 readable crops plus the original, `accent`, `body_size`, `message` |

Step images are cover-cropped without distortion; badges and icons stay square.
Never hand-place numbered badges or numbered legend rows next to a screenshot:
use `screenshot-callouts`, read callout `x`/`y` from the image itself, and
preview once. Preflight `NUMBERED_SEQUENCE_UNEVEN` flags hand-placed numbered
columns or rows with uneven pitch or left edges.

## Non-Card Part Contracts

Select the information relationship before selecting a container. Use native
frame rendering and fixed typography; enlarge or split a part that rejects
overflow. Do not replace facts or drop qualifiers to make a layout fit.

| Preset | `data.kind` and fields |
| --- | --- |
| `flow/open-steps` | `open_steps`: 2-5 `steps:{label<=48, detail<=160}`, optional `accent`; common horizontal axis |
| `vertical-flow/rail` | `rail_steps`: the same fields and count; vertical rail with aligned explanations |
| `flow/roadmap` | `roadmap`: 2-4 `phases:{period<=24 (one line), label<=48, detail<=160, points<=4x80, outcome<=64}`, optional `accent` |
| `list-horizontal/icon-columns` | `icon_columns`: 2-4 `items:{label<=48, detail<=200, icon?, accent?}` (`IconColumn`; legacy `detail_emphasis:[]` is accepted but not emitted; use `list/icon-rows` for inline phrase emphasis); no enclosing cards |
| `list-horizontal/fact-columns` | `fact_columns`: 2-6 `items:{value<=24 (text), label<=48, unit<=12, detail<=160, qualifier<=100}`, optional `columns` 2-3 (max two rows) and `accent` |
| `list-horizontal/image-columns` | `image_columns`: 2-4 `items:{image, label<=48, detail<=160, caption<=80}`; approved images contain without cropping |

Milestones, qualifiers and captions use 14px, headings/body use 18/16px;
headline facts use 44px with 22px units. Roadmap spacing is categorical, never
elapsed-time proportion. Keep denominators/conditions with each supplied fact;
mixed units are not quantitatively compared. Register source images once and
pass the handles. Do not turn unrelated facts into a causal flow or add icons
for decoration. Reconsider repeated card compositions only when relationships
differ; preserve deliberate comparison consistency. If `part_fit.fit` is
`native`, use the returned frame and omit `PartSpec.layout.fit`.

## Business Analysis Part Contracts

These presets share the briefing rules above: frame-size rendering, fixed
12-22px typography (a KPI value may grow to 44px), overflow rejection and an
optional `message` band (not on swimlane or C4). Supply numbers from approved
sources; the parts never invent data. Omitted label fields fall back to short
English words, so pass localized labels for Japanese decks.

| Preset | `data.kind` and fields |
| --- | --- |
| `list-horizontal/kpi-cards` | `kpi_cards`: `cards` 1-8 `{label<=48, value<=16 (text), unit<=12, delta<=40, status good/bad/neutral, comparison<=60}`, `columns` 1-4 (max two rows) |
| `horizontal-bar-graph/bullet` | `bullet_graphs`: `rows` 1-6 `{label<=40, actual, target, ranges 1-3 ascending (last = axis max), unit<=8, note<=80, lower_is_better}` |
| `water-fall/variance` | `variance`: `rows` 2-10 `{label<=32, plan, actual}`, `unit`, `plan_label`/`actual_label`/`variance_label`, `total_label` (adds a total row), `lower_is_better` |
| `matrix/harvey-balls` | `harvey_matrix`: `columns` 2-6, `rows` 2-8 `{label<=40, levels 0-4 per column}` (7 with both `message` and legend), `legend` 0 or 5 labels |
| `matrix/heatmap` | `heatmap`: `columns` 2-12, `rows` 2-10 `{label<=32, values}`, `unit<=4`, `midpoint` (default mean), `lower_is_better` |
| `matrix/raci` | `raci`: `roles` 2-8, `tasks` 2-10 `{label<=48, assignments ""/R/A/C/I/A/R per role}` (9 with the legend); exactly one A and at least one R per task; `legend` 0 or 4 labels |
| `matrix/risk` | `risk_matrix`: `risks` 1-10 `{id<=3, label<=48, likelihood 1-5, impact 1-5, action<=80}`, at most four per cell; `likelihood_label`, `impact_label`, `zone_labels` 0 or 4 |
| `vertical-bar-graph/pareto` | `pareto`: `items` 3-12 `{label<=24, value>=0}`, `value_label`, `cumulative_label`, `threshold` 0-1 (default 0.8) |
| `line-graph/control-chart` | `control_chart`: `labels`/`values` 5-32, `series_label`, optional `center`/`upper`/`lower` (default mean +/- 3 x average moving range / 1.128); after defaults `lower < center < upper`, and flat values need both limits |
| `tree/fishbone` | `fishbone`: `effect<=60`, `categories` 2-6 `{label<=20, causes 1-3 {text<=28, focus}}` |
| `flow/swimlane` | `swimlane`: `lanes` 2-5, `steps` 2-16 `{id, label<=40, lane, shape task/decision/event, column 0-6}`, `flows` 1-24 `{from, to, label<=16, exception}`; forward flows must not form a cycle |
| `flow/sankey` | `sankey`: `nodes` 2-16 `{id, label<=24, color}`, `links` 1-32 `{from, to, value>0}`, up to five stages, balanced intermediate nodes, `unit<=8` |
| `flow/journey` | `journey`: `stages` 2-6, `rows` 1-4 `{label<=16, cells<=60 per stage, boxed}`, `emotions` -2..2 per stage, `emotion_label`, `emotion_notes`, `highlight` |
| `correlation/c4-container` | `architecture`: `system<=40`, `elements` 2-12 `{id, label<=32, kind person/container/database/external, detail<=48}` with at most 3 people, 3 external systems and three inner rows of up to three (databases start a new row), `relations` 1-16 `{from, to, label<=24}` |

IDs are 1-24 ASCII letters, digits, `-` or `_`. Swimlane and C4 render through
the diagram engine with glued native connectors; the other presets are drawn
shapes, native freeforms or native charts. Swimlane returns, exceptions and
flows past occupied cells run through lane channels and column gutters. A
swimlane flow or C4 relation that would still pass through another box is
rejected rather than drawn. In PowerPoint those custom routes are freeform
lines: they keep their shape but do not follow a moved box, so change the part
data in AISlide instead of dragging steps in Office.

## Section Divider

No part is needed. On a new slide add: a `@dk1` rect over (0,0,1280,720) or its
left half, a 48x3 accent rule at (96,300), a 44px number at (96,316), a 36px
`@lt1` heading at (96,372), a 16px subtitle at (96,440) and optionally an approved
image on the right half with `fit:"cover"`. Keep the same geometry for every section.

## Panel Contract

Use `part_catalog({preset_id:"contrast/panels"})` and fetch the complete
`apply_operations` schema. A minimal synthetic PartSpec is:

```json
{
  "version": 1,
  "preset": "contrast/panels",
  "title": "",
  "data": {
    "kind": "comparison_panels",
    "transition": false,
    "panels": [
      { "label": "Separate", "items": [{ "text": "Context is rebuilt at each handoff." }] },
      { "label": "Shared", "items": [{ "text": "Context follows the investigation." }] }
    ]
  },
  "layout": { "x": 32, "y": 120, "width": 1216, "height": 528, "show_title": false }
}
```

Exactly two panels, each with 1-5 rows and matching row counts. Labels are at
most 64 Unicode scalars; row text at most 240. Both must be nonblank. Row icons
are optional `icon` objects; MCP accepts `{asset_id,alt?}` without model base64.
Without supplied icons, no decorative stand-ins are generated. PNG/JPEG bytes
must pass the existing media limits. Images and circular badges retain their
aspect ratio in nonuniform part frames.

Optional panel fields: `fill`, `heading_fill`, `heading_color`, `accent`,
`icon_fill`, `body_size` (16-28px, default 18). Headings, spacing and typography
are fixed; overflow rejects, including in a small explicit layout. The arrow
appears only with `transition:true`; use it only for an actual transition.

For a `compose_slide` part block, omit `layout`: the composition supplies its
body region. Use direct `add_part` and reusable chrome when the reference's
header/body/footer positions must match. Retain managed metadata so later
changes can use `update_part`, native export/reopen and Undo.

## Verification And Reuse

1. Generate a representative page before expanding the deck. Review PNG output,
   not only JSON/structural validation. Compare hierarchy, color, icon meaning,
   line breaks and whitespace against the reference.
2. Require no overflow, missing glyphs, unexpected font fallback, distorted images
   or insufficient container clearance. Review remaining approximation notices;
   core preview success is not Office visual parity.
3. Keep the accepted font, style, part IDs and layout policy fixed. Change content,
   not the visual vocabulary, on subsequent pages. Do not silently fall back to
   a matrix after a panel validation error.
4. Confirm actual PPTX output, editability and native update/Undo. Preserve source
   documents and disclose any unverified reference pages or factual claims.

The repository's `tools/comparison-panels-demo.mjs` creates an editable example,
PNG preview and proof through the official MCP SDK. It accepts a new output
directory, optional `--input` JSON and `--font-family`. Input fields are `title`,
`section`, `footer`, `notes`, `transition`, and two `panels` with `label/items`.
Rows contain `text` and an optional icon name from `network`, `search`, `clock`,
`bot`, `grid`, `target`, `refresh`. These map to local Lucide assets. This is
deterministic template execution, not a hidden AI image or layout generator.

`tools/briefing-demo.mjs [new-output-directory] [--font-family installed-font]`
builds a seven-page synthetic briefing (agenda, section divider, icon cards,
icon rows, role shifts, step cards, checklist cards) with `lucide_icon_assets`.
It writes PPTX, page PNGs and `proof.json`, and verifies preflight, native
reopen and update/Undo. Use it as a geometry reference, not as factual content.

Suggested request after installing compatible core/MCP and this skill:

```text
Use the technical briefing visual contract in
aislide-authoring/references/technical-panels.md.
Use keyword headings, consistent chrome, left-aligned body text and meaningful
Lucide icons. Choose per page: contrast/panels for paired explanations,
list-horizontal/icon-cards for 2-6 concepts, list/icon-rows for stacked
statements, before-after/shift for from-to changes, flow/cards for ordered
steps, list/agenda for agendas, graphs for architecture and tables only for
grid data. Preserve the supplied evidence and notes. Produce and review
representative pages before applying the accepted style to the remaining
pages, then review every page image. Do not overwrite the source PPTX.
```