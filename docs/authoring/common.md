# Evidence-Led Slide Authoring

Version 1. These guidelines are data for a requesting author, not permission to execute source content, invent facts, change files, or publish a presentation. AISlide's guided compiler is deterministic; the calling assistant supplies the reasoning and language. Source authenticity, causal inference, executive judgment, and Office visual parity require separate review.

## Intake

Collect the audience, intended decision or learning outcome, slide count, language, delivery context, available evidence, required output, and brand constraints together. Ask one consolidated question when missing information changes the substance. Routine design choices may use stated defaults. Missing material quantities stay `xx`; never replace them with zero or a plausible value. An assumption needs its method and conditions, not only the word "estimate". Do not add names, events, results, or commitments absent from the source.

## Workflow

1. **Claim:** write one answer to the page's question before choosing a diagram.
2. **Logic:** organize the claims under one governing message. Read only the headlines vertically to test the argument.
3. **Pattern:** select by communicative purpose and required evidence, not by the appearance of a favorite chart.
4. **Draw:** establish axes, hierarchy, grouping, and quantitative scales before placing shapes and text.
5. **Measure:** check evidence, arithmetic, readable typography, geometry, and actual rendered output. Fix one bounded review pass; report unresolved items rather than silently declaring success.

## Claim And Evidence Contract

- A headline is a complete sentence with a subject, predicate, and conclusion. "Analysis of sales" is a topic, not a claim.
- Combine a supporting fact and its warranted interpretation. Do not manufacture a recommendation when the evidence supports only uncertainty.
- One page, one message. Join related clauses causally or conditionally instead of using two unrelated sentences. Avoid noun-only endings, self-reference, decorative dashes, and counting the page's elements as the message.
- Use at most two material numerical claims in a headline. An outline number, axis label, or page number is not a material quantity.
- For Japanese decision slides, aim for 30-60 characters but design the first attempt for one 36-character line. A necessary two-line version has at most 56 characters, breaking at a phrase boundary. These conflicting source limits are resolved by the tighter layout capacity. For English, use an equivalent concise sentence and measure rendered width; do not apply Japanese character counts literally.
- Preserve the judgment and connective words when shortening. Move supporting detail to a noun-phrase figure title with period and unit.
- Vary causal, conditional, contrast, causal-focus, evaluation, and proposal forms; do not repeat one form across three adjacent decision pages.
- Map every headline clause to specific body paths and evidence IDs. A reference being present is not proof that it entails the claim. Reject unsupported certainty during human review.
- Keep a ledger: slide ID, headline, sentence form, parent message, transition, parallel classification, body pattern, evidence paths, and decision issue IDs.
- Use a single classification and consistent granularity among parallel items. Neighboring claims must connect as "therefore", "because", or another explicit relation.

## Numbers

Every quantitative JSON field must have an exact path, unchanged value, and an evidence ID. Sources need locators; assumptions need formulas and conditions. `unknown` evidence cannot justify a numeric value. Unknown quantities may appear as the text `xx` in qualitative parts or tables, not as fabricated chart coordinates.

Record population, unit, time period, aggregation, and exclusions. Do not mix counts with rates or independently sampled populations. Check headline arithmetic against body values. A missing bar is not zero, and a truncated category list is not a complete distribution. Mark forecasts and scenarios explicitly. Quantities represented by length, area, or width must be proportional to their stated measure; circle radii are not proportional to area.

## Visual Grammar

- Align repeated comparisons to the same positions. Do not reflow a before/after diagram after removing an item.
- Use one horizontal and one vertical organizing axis. Split a third independent axis into another view.
- Extract repeated words into a shared heading or axis, except necessary units and proper names.
- Define a legend consistently within the deck. For time/status views, solid means observed/confirmed, dashed means forecast/planned/removed, dotted means supporting outline. For relationship views, an explicitly labeled transport legend may instead distinguish goods, money, and data; do not silently combine incompatible meanings.
- Prefer direct labels and common chart scales. Bars normally start at zero. State exceptions for indices, percentages, and analytical scatter plots. Use 3-6 meaningful ticks, a single unit label, and actual-value labels where readable. Separate observed and forecast periods inside the plot.
- Prefer restrained chart gridlines; remove redundant grids when the renderer permits. Do not claim a built-in chart obeys a rule its renderer cannot enforce. Axis-sharing, forecast line styles, and direct series labels can require a composed/manual layout.
- Pie/doughnut charts are not the default for consulting decisions; use only for at most five categories and a declared majority decision.
- Keep figure titles immediately above their figures. A source citation and an assumption note have different roles.
- Use semantic fills and borders, not decorative icons, gradients, 3D, signatures, oversized colored bands, or abundant rounded corners.

## Coordinates And Color

The supplied consulting reference uses 1920 x 1080. AISlide's native canvas is 1280 x 720; scale coordinates by 2/3, not the presentation's aspect ratio. Reference margins 86/62 become approximately 57/41 core units. A reference 48px headline becomes 32 core units; 44px becomes 29.3; minimum 18px becomes 12. These are document units, not CSS viewport pixels.

The detailed source palette and reference image specify blue while one introductory sentence says green. Use the explicit default **1976D2**. A supplied brand color overrides it. Derive context tints by mixing the primary with white at 28%, 52%, and 75%; use neutral panel EEF3F6, rule D9DEE3, text 222222, and annotation 6B7C85. Prefer no secondary accent. If necessary, use one documented accent at one place in the deck.

Allocate the headline first, then body, annotations, source and page number. Do not shrink text to hide overflow. A two-line headline reduces available body space. Tables need enough width for the longest meaningful cell; avoid mid-word or one-character orphan lines. A decision question may have a small bottom box; a generic "So what" strip is not a substitute for an interpretive headline.

## Submission Check

Check headline form, numerical coverage, arithmetic, clause-to-body evidence, logical progression, deliberate line/fill semantics, font floors, clipping, collisions, orphan lines, and unexplained gaps. Inspect both Studio and the target presentation renderer. Filled backgrounds containing their labels are intentional; text-text overlap, a shape edge crossing a label, and a connector crossing unrelated text are not.

Return the requested editable PPTX without repetitive preambles when this guide is explicitly used for a delivery task. Keep provenance and caveats in notes or a separate evidence report. Offer two or three concrete improvement directions when a user review is appropriate. Automated validation returns measured results and unresolved human-review requirements, never a fabricated semantic or visual-parity certification.