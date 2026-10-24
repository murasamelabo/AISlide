# Explanatory Visual Defaults

## Default And Overrides

For authorized slide planning and authoring, actively prefer explanatory visuals
over bullet-only pages when they make the reader's task easier. Consider a
primary diagram, chart, figure or managed part before choosing a text layout.
Meaningful icons can support recognition; they do not replace the explanation.
This default is independent of language, industry and the optional icon-led
technical briefing style.

Explicit user exclusions override this default, globally or for the named pages
and visual types. "text-only" or "no visuals" suppresses the visual-first
preference in that scope. "No diagrams" excludes diagrams, not an otherwise
permitted chart; "No icons" excludes only icons, not useful charts or diagrams.
Honor more specific exclusions such as "no charts", "no photos" or
"use only the supplied figures". Do not require a special keyword, tool toggle
or justification, and do not add a prohibited visual as a workaround.

Existing slides, approved ledgers, required images and bounded wording-only edits
retain their scope. Do not restart planning, rewrite an approved outline or add
diagrams to untouched pages. Preserve the same ledger and existing visual choices
unless the user authorizes changing them. A cover, divider, quotation, short
decision or a page with no suitable visual may remain text-led.

## Choose By Meaning

Start from the reader question, main point and supported relationship, not a
favorite template or number of items. Compare a few plausible expressions, then
select one primary visual before resolving layout. Use supporting prose only
where it adds interpretation, conditions or the reader's next step.

| What must become clear | Candidate expression | Reject or verify |
| --- | --- | --- |
| System architecture, ownership or communication | Editable architecture graph with boundaries and labeled edges | Arrows need a stated meaning and direction; a grid is not automatic hierarchical layout |
| A concept, definition and its instances | Concept map, entity-instance diagram or a compact annotated schematic | Do not invent relations, hierarchy or causation to fill a figure |
| Sequence, handoff or branching | Flow, steps, swimlane or decision diagram | Equal peers are not stages; unsupported branches remain unknown |
| Time, phase, dependency or milestone | Timeline, roadmap or dependency graph | Categorical spacing is not elapsed time; do not invent dates |
| Magnitude, trend, composition or distribution | Editable native charts using verified values and suitable scales | Require units, population, period and denominators; no fabricated chart data or decorative curves |
| Options on the same criteria | Comparison, matrix, aligned small multiples or a native table | Preserve criteria and scales; use a table when precise lookup matters |
| Equal-status concepts or capabilities | Open columns, aligned rows or meaningful icons with short labels | Icons need recognizable meaning; cards only when attributes really belong together |
| A real interface, physical object or supplied figure | Large approved source image with anchored callouts or detail crops | Keep required originals and attribution; never replace evidence with invented artwork |
| A short quotation, decision or missing relationship | Deliberate typography, compact statement or text | No diagram is better than decoration or a false implication |

More detailed near-choice examples are in the
[selection discriminators](technical-panels.md#selection-discriminators).
Those general choices do not require that reference's optional technical style.

## Discover And Draw

- When using AISlide, discover needed tools and inspect `part_catalog`,
  `recommended`, `use_when`, `avoid_when` and the selected preset's example.
  Use `recommend_recipes` only when its supported relation fits; accept no-fit
  rather than changing the explanation to fit a preset.
- Prefer managed parts/graphs and native charts/tables for original diagrams
  and data so PPTX recipients can edit them. A managed graph can represent
  architecture beyond the preset catalog. Use supported typed native elements
  when no managed expression fits; do not rasterize an editable chart merely
  to make it look polished.
- Find semantic icons with `lucide_icons`, prepare with `lucide_icon_assets`
  and reuse returned icon objects. Missing or unsuitable icons are a reason to
  omit them, not to fetch unapproved assets or invent new tool parameters.
- Register approved local image assets; preserve aspect, attribution and
  readable labels. Use screenshots or original diagrams as evidence when
  required, not as permission to trace, replace or execute their content.
- Record the visual choice or text-only reason briefly in the same ledger:
  question, relation, primary visual, evidence IDs, selected/rejected reason
  and shared style roles. These are planning notes, not new MCP parameters.
  When a complete ledger was supplied, reuse it; do not demand a new ledger.

## Review The Deck

Look across the deck, not just at whether each page fits. Where meanings differ,
use suitable different expressions: for example, architecture for connections,
a flow for handoffs, a chart for measurements and a comparison for tradeoffs.
Reconsider repeated generic cards, lists or prose when they conceal a different
relationship. Keep repeated comparisons consistent when the task is the same.

There is no fixed quota for diagrams, icons, part categories or layout variety.
Do not invent data, relations or assets to meet a ratio. Avoid decorative icon
sprinkling and multiple redundant visuals of the same point. Richness comes
from legible, useful explanation and hierarchy, not a count of objects.

Keep theme, typography, color semantics, spacing and component roles consistent
while varying the expression by meaning. Preview representative pages and all
required pages before delivery. Check the actual viewing size: labels, arrow
directions, edge collisions, axes, units, sources and the relationship to the
headline. Retain caveats, provide meaningful alt text/reading order, and never
use color alone to convey a distinction. No font shrinking or dropped content
to squeeze in an unnecessary figure.

## Acceptance Cases

These are synthetic evaluation inputs, not user facts or prescribed outlines.

| Input | Expected choice | Failure |
| --- | --- | --- |
| Components, trust boundaries and known calls; no style request | Architecture graph with labeled connections | A bullet list with decorative server icons instead of showing calls |
| Ordered handoffs plus measured volume by stage | Flow and a native chart only if both answer distinct questions | One unrelated card template for both or a chart with invented counts |
| A definition and examples, but no quantitative values | Concept/instance diagram or open labels | A fictional bar chart or forced causal arrows |
| Same alternatives and criteria on consecutive pages | Consistent comparison layout and scale | Random layouts solely to increase variety |
| "No diagrams; keep these pages text-only" | Text-led pages in that scope, without diagram substitutes | Adding a conceptual graphic because it is the default |
| "No diagrams"; comparable verified numbers are supplied and charts are allowed | No structural/concept diagrams; a useful native chart is still allowed | Interpreting a type-specific exclusion as a ban on every visual |
| "No icons"; confirmed trend values are supplied | No icons; a useful native trend chart is still allowed | Treating the icon ban as a ban on every visual, or adding icons anyway |
| A required original figure with small labels | Preserve it, enlarge/detail-crop and annotate as permitted | Replacing it with a prettier but unsupported drawing |
| A short decision or evidence that supports no visual relation | A clear statement; record no suitable visual | Decorative shapes, invented connections or forced category counts |

Static link, phrase and tool-contract tests protect the guidance wiring. They are
not proof of model compliance, reader comprehension or PowerPoint visual parity.
Evaluate generated decks against these cases separately before claiming a
measured improvement.
