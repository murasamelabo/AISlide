# Technical Explanation

## Outcome And Intake

Enable the reader to explain a mechanism, assess a tradeoff, or operate a system correctly. Ask for reader expertise, the system/version/environment, exact learning or engineering decision, authoritative evidence, scope boundaries, failure modes, time budget, and output format. Unknown topology, throughput, SLA or behavior stays `xx` or explicitly unverified. Do not invent deployed infrastructure from a proposed design.

## Claim

Answer one technical question per page in a complete sentence. Distinguish a current observation, a design constraint, a hypothesis and a proposed change. A component list is not an explanation. State the causal relationship or tradeoff, with a source path, experiment, trace or specification that supports it. Define acronyms on first use and keep terms consistent.

## Logic

Build the ledger in this order: problem and constraints, system boundary, mechanism, failure behavior, alternatives, validation, and next action. The governing message explains the mechanism or engineering recommendation. Each transition must identify a dependency, sequence or cause. Do not present implementation details before the reader knows the boundary they belong to.

## Purpose Patterns

| Intent | Required inputs | Native starting point | Acceptance check |
| --- | --- | --- | --- |
| Explain the boundary | Actors, owned system, external dependencies | diagram/custom | Ownership and trust boundaries are labeled; no inferred deployment |
| Trace a request | Initiator, ordered operations, sync/async handoffs | flow/balanced | Arrow direction, data names, and reply path are explicit |
| Explain a dependency | Parent-child relation and dependency meaning | tree/focus | Children belong to the right parent; hierarchy is not confused with time |
| Compare choices | Common criteria, constraints, unknowns | matrix/labeled | Criteria and granularity match; assessment is attributed |
| Show failure handling | Trigger, detection, containment, recovery | vertical-flow/balanced | Failure, retry and human intervention remain distinguishable |
| Explain a control loop | Observed state, decision, action, feedback | cycle/focus | Feedback is real; diagram does not invent automation |
| Present a benchmark | Hardware, workload, versions, repeated measurements | horizontal-bar-graph/labeled | Units, baseline, distribution and limitations are stated |
| Plan a migration | Phases, owners, rollback and acceptance gates | gantt-chart/labeled | Dates and progress are supplied, rollback is not implied by an arrow |

These are starting components, not automatic sequence-diagram, trust-boundary or physical-timing certification. Add native annotations when a specialized composition is required.

## Visual Selection Record

Before drawing, add to the same ledger the reader question, supplied relationship,
primary visual, selected candidate, nearest rejected candidate and its reason,
emphasis claim roles, source dimensions and shared component roles. Compare two or
three plausible choices rather than filling a fixed container from item count.
Record short observable decisions, not private model reasoning.

Use a graph for definition-to-instance, boundaries and handoffs, not an arbitrary
layer stack. Use a table only for common criteria; paired verbal approaches fit
contrast panels. Short peers fit open columns; cards fit bundled attributes.
Short order fits open steps; longer text fits a vertical rail; phases and outcomes
fit a roadmap only when supplied. A wide source figure may need full width above
readings; exact screenshot locations need source-relative callouts. Allow no
suitable recipe instead of recategorizing facts to fit an available variant.

`recommend_recipes` exposes contextual reasons, avoid cases, decision checks and
rejected alternatives. It uses the supplied classification and cannot verify the
meaning of the source. `component_styles` resolves the optional technical briefing
roles shared by recipes, pattern compositions and manual additions. Fix these
roles before the first representative page, not as a final font-replacement pass.

Prefer open headings and text for short readings; distinguish takeaways from
body and source by their role, not by making all of them open or all boxed.
Do not use a tinted rectangle with a top or left strip as the default for every
note. Reserve containers for a meaningful boundary, warning, comparison or a
bundle of attributes. Supply rich-text bold/color runs for the important words;
use meaningful supplied icons for short peer columns, not decorative stand-ins.
Preserve conditions and normal text around the emphasis. Judge unused area by
the reading task rather than filling it with more boxes.

Choose `inline`, `rule`, `outline`, `tint`, `closing` or `solid` by role.
`auto` follows supplied emphasis strength and relationship: subtle context is
inline; a normal supporting takeaway or a conditional qualifier uses a separator;
a strong qualifier, a standalone or a contrasting group uses an outline; a strong
supporting point uses a tint. A qualifier never receives the tinted surface
automatically, because it limits the claim and must not outrank the conclusion.
`closing` and `solid` are never chosen automatically; select them for a conclusion
(see below). `takeaway_emphasis` and pattern `emphasis`
blocks permit explicit overrides while preserving rich-text emphasis. The
technical profile defaults to a separated supporting conclusion. `visual-top` uses
compact unboxed readings and must increase actual contained image size, not just
slot width. Its recommendation compares the standard 1280x720 profile; measure
the actual deck too. Screenshot recipes retain the whole source plus 1-3 readable
cropped details with markers outside source text. For precise borders supply the
original control's `target_region`; use its projection for both views and leaders,
not the crop boundary as a guessed control outline. Split if detail or explanation
does not fit at fixed size. Never claim legibility from a fitting rectangle alone.

### Choose Where The Conclusion Goes

A slide needs at most one conclusion. Decide first whether it needs a separate
element at all, then where it goes and how loud it is. `component_styles` returns
the same guidance as `message_placements` and as `use_when`/`avoid_when` on every
`emphasis_variants` entry, so an authoring agent can select without this page.

| Role of the sentence | Placement | How to author it | Use it when | Avoid it when |
|---|---|---|---|---|
| The claim or conclusion, read first | **Lead** under the title | `pattern.lead:true`, slot `lead-message` (emphasis, text, callout or label); style `inline` (default) or `rule` | Decision or executive material, a chapter claim before a long diagram or table, a reading guide for a dense visual; one line of about 40 full-width characters | The first body row already says it, or the slide also has a closing line or a solid band |
| The conclusion after the body | **Closing line** at the bottom | `pattern.message_band:true`, `emphasis.style:"closing"` | A decision, next action or one-sentence summary that follows from the body; one line at 28px above a 2px accent rule | Qualifiers, text longer than one line |
| The strongest conclusion | **Solid band** at the bottom | `message_band:true`, `emphasis.style:"solid"` | The one sentence of a summary, chapter-end or decision slide; reversed text on the ink role, one line at 24px; mark phrases with bold, not the accent green | Ordinary slides; use it on a few slides so it keeps its weight |
| A qualifier, scope limit or exception tied to one row | **Folded into the body** | `detail_emphasis` on `list/icon-rows` rows, or an emphasized run in text | The reader should meet the limit exactly where it applies | A conclusion about the whole slide, or a limit that applies to every row |
| A qualifier about the whole slide | **Bottom, quiet** | `message_band:true`, `emphasis.relationship:"conditional"` with `strength` `subtle` (inline), `normal` (rule) or `strong` (outline) | A scope note that must stay visible without outranking the conclusion | Restating the title or the first row |
| Nothing | **Omit** | Put the claim in the title | The title states the claim and the body is the evidence, or the slide is a diagram, screenshot or table that speaks for itself | A summary or decision slide with no stated takeaway |

Order of decisions: delete a sentence that repeats the title or the first row;
fold a qualifier into the row it limits; otherwise place it quietly at the
bottom; reserve the lead, the closing line and the solid band for the one
conclusion of the slide. Do not combine a lead with a closing line or solid band
on the same slide. The lead accepts only `inline` or `rule`. `closing` and
`solid` are fixed-size single lines; text that does not fit is rejected, not
shrunk, so shorten it or choose another placement.

## Draw

Use actor/ownership lanes for topology and a separate time axis for sequence. Do not put latency, topology, and organizational ownership on one unlabeled axis. Name the payload on a data edge and the condition on a decision branch. Use filled blocks for responsibilities and restrained connecting arrows for relationships. Keep text inside the safe rectangular region of nonrectangular nodes.

Keep diagrams abstract enough to read but specific enough to falsify. A logical diagram is not a deployment map. A protocol guarantee is not a benchmark result. Use paired before/after layouts without moving unchanged components. A code excerpt should be short, exact, versioned and essential; never execute code imported as slide content.

## Evidence And Unknowns

Every numerical claim needs an experiment or source with scope, sample size and units. Identify simulated versus measured values, throughput versus latency, percentile versus mean, and cold versus warm runs. Do not infer a causal improvement from one noisy measurement. Unknown capacity stays `xx`; never draw a proportional bar from it.

## Measure And Review

Trace one normal request and one failure through the diagram. Verify every named component against supplied evidence. Check that arrows cannot be mistaken for network traffic when they mean ownership. Validate labels, units, connector intrusion, font availability and actual PPTX rendering. Human review checks technical correctness and omitted failure modes; automation checks declared evidence, inputs and geometry.

## Delivery And Refinements

Deliver editable native elements with notes containing versions, evidence and limitations. Offer focused refinements such as a failure trace, measured tradeoff comparison, or an operator runbook view. Do not add speculative infrastructure to make the picture look complete.