# Guided Authoring

For local spelling, proofing-language assignment, synonyms, bilingual terms, and text-format reuse, see [Local proofing and format painter](proofing-format-painter.md). These are bounded editing functions, not model-based grammar or full-sentence translation.

AISlide exposes English, purpose-specific best practices and a deterministic compiler for evidence-linked outlines. The calling assistant plans the claims and supplies content; these tools do not contact a model, fetch source URLs, execute source text, or invent missing values.

## Profiles

| ID | Intended outcome | Guide |
| --- | --- | --- |
| `consulting-decision` | Reach a defined decision with stable issue IDs | [Consulting and decision meetings](consulting-decision.md) |
| `technical-explainer` | Explain a mechanism, boundary or engineering tradeoff | [Technical explanation](technical-explainer.md) |
| `event-talk` | Communicate a central idea to a live audience | [Event presentation](event-talk.md) |
| `status-report` | Explain status, deviation and follow-up | [Reports and operational updates](status-report.md) |

All four include the [common five-stage workflow](common.md): Claim, Logic, Pattern, Draw, Measure. Guides are stored in English while generated content can be Japanese or English. Explicit palette blue `1976D2` takes precedence over the supplied example's conflicting introductory green default; `brand_color` overrides it.

The [48-pattern consulting catalog](consulting-patterns.json) preserves the supplied IDs, purposes and drawing rules. `C02` and `C03` are implemented native templates. Other catalog entries declare `composition-required` or `guidance-only`: they are not 48 automatic slide templates. `native-part` composes a headline and an existing part. Select the exact existing preset separately and preserve its limitations, rather than presenting it as the complete compound pattern. The seven existing design presets are unchanged; guided creation uses a separate native master/layout and evidence-led palette.

## MCP Workflow

1. `best_practice_profiles` lists the four profiles.
2. `best_practice_guide({profile_id})` returns combined English guidance, pattern capabilities, limits and the Rust-derived `input_schema`.
3. The assistant resolves the audience, purpose, evidence, logical ledger and body content. Missing quantities stay `xx` or are explicitly sourced assumptions.
4. `validate_guided_presentation({input})` checks input relationships and compiles a temporary native layout. It creates no persistent handle or file.
5. `create_guided_presentation({input})` repeats the same validation, then returns a new `deck_id`, actual `revision`, slide count and review report. No existing document is replaced.
6. Review the output with `get_document` and actual slide rendering. `export_pptx({deck_id,filename})` is a separate create-new save operation requiring the server's operator-approved output directory.

The SDK equivalents are `client.bestPracticeProfiles()`, `client.bestPracticeGuide(profileId)`, `client.validateGuidedPresentation(input)` and `client.createGuidedPresentation(id,input)`. Creation returns `{session,validation,profile_id,model_inference:false}` and rejects a late success after cancellation. Normal subsequent session edits remain revision-checked and undoable. The initial compiled document has no undo entry for its creation; read the returned revision instead of assuming zero.

## Input Contract

`GuidedInput` contains `version:1`, `profile_id`, `title`, `audience`, `purpose`, `governing_message`, `language` (`ja` or `en`), optional six-digit `brand_color`, `evidence`, optional `issues`, and `slides`.

Evidence entries have `id`, `kind` (`source`, `assumption`, `unknown`), `reference` and `statement`. Put a precise source locator or an assumption's method in `reference`; use a concise supporting statement, especially when it must fit an executive-summary cell. These declarations are not authenticated source documents or the SDK's live data bindings.

Each slide contains `id`, `section`, `headline`, `sentence_form`, `pattern_id`, `question`, `parent_message`, `transition`, `parallel_basis`, `support`, optional `numbers`, and a `part` for `native-part`. The parent is `governing` or an earlier slide ID. Supported sentence forms are causal, conditional, contrast, causal-focus, evaluation, proposal, explanation, comparison and outcome.

Support entries contain `clause`, `body_paths` and `evidence_ids`. Ordered clauses must reconstruct the whole headline after whitespace normalization. Native-part paths refer to populated `/data` within `PartSpec`, not its title. C02/C03 paths refer to populated `/issues`. Pointers select the actual content offered as support; their existence does not prove the claim.

Number declarations have `{path,value,evidence_id}`. For example:

```json
{
  "support": [{
    "clause": "We should prioritize the larger exposure because it doubles the baseline.",
    "body_paths": ["/data/series/0/values"],
    "evidence_ids": ["fixture"]
  }],
  "numbers": [
    {"path":"/data/series/0/values/0","value":12,"evidence_id":"fixture"},
    {"path":"/data/series/0/values/1","value":24,"evidence_id":"fixture"}
  ]
}
```

These are synthetic test values, not factual business data. Numeric quantities in part data need exact path/value declarations with source or assumption evidence. Geometry and structural/index fields are excluded: x/y/width/height, font/stroke size, version, edge from/to indices, timeline start/end indices, longitude/latitude. Their factual meaning, along with dates, amounts and counts embedded in prose, remains a review responsibility. Unknown evidence cannot justify a numeric chart value. Use `xx` in qualitative content instead of fabricating a chart bar.

Limits: guided authoring retains 1-32 slides, at most 64 evidence entries, 6 decision issues, 8 support clauses per page, 16 body paths and evidence references per clause, and 256 numeric declarations per page. Existing core part limits and the selected fixed capacity profile apply; default large is 96 MiB wire / 32 MiB complete document. Standard and legacy remain smaller. [Phase 5 capacity](../testing/phase5-recovery-capacity.md) does not expand the guided input grammar. Long citations or text that cannot fit are rejected rather than clipped or silently omitted. Source/reference strings are inert, not fetched.

## Decision Templates

A multi-page consulting deck starts with `C02` and ends with `C03`, with 3-6 shared decision issues. Each issue has `id`, `question`, `requested_decision`, `criterion`, `owner`, `due`, `evidence_ids` and `analysis_slide_ids`. Analysis references must resolve to actual `native-part` pages, not the summary or close.

C02 shows issue, requested decision, the cited evidence statement, and analysis page numbers. C03 shows the same issue IDs, decision and owner, timing, and criterion, plus a compact immediate schedule. Both use purpose-sized columns in editable native text/fill/rule groups; they are not rasterized or equal-width PowerPoint table cells. Their values can be edited as normal native elements, but the whole decision grid does not have part metadata for automatic regeneration.

Japanese consulting headlines target one 36-character line; 30-56 characters are accepted and longer ones reserve a two-line area. The compiler rejects more than two numeric runs, forbidden dashes/self-reference, and three adjacent identical sentence-form declarations. It cannot fully recognize Japanese grammatical completeness, whether a number merely counts items, or whether a judgment is warranted; these checks remain in the guide. Body resizing normalizes child geometry/fonts in the native group, with a 12-core-unit font floor corresponding to the reference's 18px at 1920x1080. Event profiles enlarge key labels where their frames allow; this is not automatic projection-legibility certification.

## What Ready Means

`ready:true` means the typed input, references and measured native text layout are compilable. It does **not** mean semantic truth, causal entailment, complete argumentation, universal absence of visual collisions, or Office parity. Every response retains `semantic_truth_verified:false`, `office_visual_parity:false`, and explicit `review_required` items. `ready:false` includes unmet checks; creation rejects that input without returning a handle.

Actual charts retain their existing renderer's conventions. Common-axis small multiples, forecast line styles, direct end labels and quantitative area plots may require manual composition. An arbitrary part is not automatically transformed into a fully compliant consulting compound visual. Follow the capability metadata and inspect the actual PPTX.

The complete ledger and evidence text are preserved in slide notes, including assumptions and source references. Review them before redistribution. Subsequent manual edits can invalidate the original reasoning or source declarations; they are not continuously revalidated by the guided compiler. Existing native part fingerprints continue to protect manual changes from destructive regeneration.

## Reproduce

```sh
npm run core:build
node tools/guided-demo.mjs .artifacts/my-guided-examples
node tools/parts-demo.mjs .artifacts/my-parts --japanese
```

Use new or empty output directories. The guided demo uses the official MCP SDK over stdio to retrieve guides, validate/create all four Japanese examples, export, reopen, edit and undo exactly. It writes the structured inputs beside each sample. The Japanese parts demo requires local Studio at port 4174 and verifies all 108 inserted slides; the normal English demo reuses screenshots from the existing all-preview test. [Authoring verification](../testing/parts-library.md) records the measured scope and known limits. No commit, publishing, or model inference is performed by these examples.