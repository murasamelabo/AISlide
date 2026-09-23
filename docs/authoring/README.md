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
6. Use `preview_presentation({deck_id,options:{page_indices:[0],max_dimension:1280}})` for actual MCP PNG image content. Use `layout:"contact_sheet"` for an overview. No file write is needed.
7. Run `preflight_presentation({deck_id,options:{page_indices:[0],min_font_size:24}})` for renderer warnings, possible collisions and readability heuristics. Review the image before treating geometry findings as defects.
8. Preview an authorized correction with `preview_slide_revision`, inspect before/after images, then explicitly call `apply_slide_revision` with the candidate ID and exact base revision/hash. One Undo reverses an applied batch.
9. `finalize_presentation` publishes a requested delivery bundle and a hash-bound manifest under the server's operator-approved output directory. Individual `export_pptx` / `export_static` saves remain available. Return the actual saved paths and validation scope.

The `author_presentation` MCP prompt and `aislide://authoring/workflow` resource describe this loop. Existing server configuration is sufficient; restart an already-running MCP connection after updating the source/CLI. Restarting releases process-local deck handles and candidates, so export or explicitly retain a recovery envelope first.

The SDK equivalents are `client.bestPracticeProfiles()`, `client.bestPracticeGuide(profileId)`, `client.validateGuidedPresentation(input)` and `client.createGuidedPresentation(id,input)`. Creation returns `{session,validation,profile_id,model_inference:false}` and rejects a late success after cancellation. Normal subsequent session edits remain revision-checked and undoable. The initial compiled document has no undo entry for its creation; read the returned revision instead of assuming zero.

## Input Contract

`GuidedInput` contains `version:1`, `profile_id`, `title`, `audience`, `purpose`, `governing_message`, `language` (`ja` or `en`), optional six-digit `brand_color`, optional `authoring`, `evidence`, optional `issues`, and `slides`.

Omitting `authoring` retains the previous layout and typography. Supplying `{}` enables contextual defaults: projection for `event-talk`, reading for the other profiles. All sizes below are scene pixels, not PowerPoint points.

| Authoring setting | Values | Default when authoring is supplied |
| --- | --- | --- |
| `context` | `reading`, `projection` | Profile-dependent |
| `body_font_min` | 12-40 | Reading 16, projection 24 |
| `headline_font_size` | 28-64 | Reading 34, projection 40 |
| `density` | `comfortable`, `compact` | Comfortable; changes body margins and paragraph spacing |
| `spacing` | `standard`, `relaxed` | Standard; changes body gap and text line spacing |
| `font_family` | Explicit nonempty family, at most 100 characters | Existing Yu Gothic theme |

The effective body floor applies recursively to text, shape labels and table text, including rich runs and scaled groups. Citations, section labels, page numbers and chart-internal labels are not governed by that floor. Dense content that does not fit rejects instead of silently shrinking below the floor. Large headlines reserve two lines; decision tables and schedules reserve the citation region. These settings apply at guided creation, not as a persistent restyling policy for later `update_part` calls. Use previews and diagnostics after edits.

Evidence entries have `id`, `kind` (`source`, `assumption`, `unknown`), `reference` and `statement`. Put a precise source locator or an assumption's method in `reference`; use a concise supporting statement, especially when it must fit an executive-summary cell. These declarations are not authenticated source documents or the SDK's live data bindings.

Each slide contains `id`, `section`, `headline`, `sentence_form`, `pattern_id`, `question`, `parent_message`, `transition`, `parallel_basis`, `support`, optional `numbers`, optional `speaker_notes`, and a `part` for `native-part`. Speaker notes accept at most 4000 Unicode scalars and append to the evidence/ledger notes; the combined 8000-scalar notes limit still applies. The parent is `governing` or an earlier slide ID. Supported sentence forms are causal, conditional, contrast, causal-focus, evaluation, proposal, explanation, comparison and outcome.

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

### Visual Review And Revisions

Preview and preflight accept 1-8 unique zero-based page indices. Omitting selection means all pages only if there are at most eight. Preview images have a 160-1600px maximum edge, 2MiB combined encoded-image budget, and 4MiB response budget. Images are returned directly, not as disk paths; metadata-only clients can use `include_images:false`.

Preflight returns stable slide/element IDs, `scopes` (slide/master/layout), transformed bounds, severity, evidence category and repair suggestions. Renderer warnings include actual shape-padding/table clipping and font/glyph warnings. Geometry checks flag possible text-frame overlap, connector/label interference and off-slide objects; font floor and density are heuristics. Background containment and attached endpoint nodes are excluded. At most 1024 visible objects per selected page and 256 findings are allowed; exceedances fail explicitly. Orphan lines, semantic truth, full accessibility and Office parity remain separate checks.

`preview_slide_revision` requires `deck_id`, `expected_revision`, `expected_hash`, `slide_id` and 1-16 typed `edits`. Supported edits are `translate`, `align`, `set_text_frame`, `replace_text`, `update_part`, and `update_graph`. Translation/alignment target up to 32 top-level IDs; text edits may target nested IDs, with frame coordinates in the parent space. Frame changes retain font size. Locked/hidden targets, stale managed updates and unsafe native edits reject. Editing part children can mark their metadata stale, which is reported rather than silently regenerated.

The returned `candidate_id` refers to at most 128KiB of immutable edit instructions. Sixteen candidates are retained in memory, for ten minutes, until a document revision changes or its handle closes. There is no disk persistence. `apply_slide_revision({deck_id,candidate_id,expected_revision,expected_hash})` recomputes and verifies the candidate hash before one atomic commit. Wrong-document, stale, expired and already-consumed candidates fail. No-op application adds no Undo entry. Unrelated slides and the immutable source are retained. Preview does not mutate the document/history; cancelled SDK operations reject late results before committing.

`ready:true` means the typed input, references and measured native text layout are compilable. It does **not** mean semantic truth, causal entailment, complete argumentation, universal absence of visual collisions, or Office parity. Every response retains `semantic_truth_verified:false`, `office_visual_parity:false`, and explicit `review_required` items. `ready:false` includes unmet checks; creation rejects that input without returning a handle.

Actual charts retain their existing renderer's conventions. Common-axis small multiples, forecast line styles, direct end labels and quantitative area plots may require manual composition. An arbitrary part is not automatically transformed into a fully compliant consulting compound visual. Follow the capability metadata and inspect the actual PPTX.

The complete ledger and evidence text are preserved in slide notes, including assumptions and source references. Review them before redistribution. Subsequent manual edits can invalidate the original reasoning or source declarations; they are not continuously revalidated by the guided compiler. Existing native part fingerprints continue to protect manual changes from destructive regeneration.

## Delivery Bundles

`finalize_presentation` requires the current `deck_id`, `expected_revision`, `expected_hash`, and a new `name` (1-60 ASCII letters/digits/underscore/hyphen; first character alphanumeric; Windows reserved names rejected). It never overwrites existing outputs or changes the document/Undo history. The server must have an operator-approved `--output-dir`.

```json
{
  "deck_id": "<current deck handle>",
  "expected_revision": 1,
  "expected_hash": "<current document SHA-256>",
  "name": "review-delivery",
  "options": {
    "page_indices": [0, 1, 2],
    "pdf": true,
    "preview": "contact_sheet",
    "preflight": true,
    "notes": true,
    "source_report": true,
    "max_dimension": 1280
  }
}
```

Only enable `notes` / `source_report` when separate plaintext exports are intended. Both default off. The PPTX itself still retains notes and may include sources/bindings; finalization is not a redaction tool.

| Output | Scope and default |
| --- | --- |
| `name.pptx` | Always the complete presentation, preserving normal export/native/source checks |
| `name.manifest.json` | Always; file names, sizes, SHA-256 hashes, document revision/hash, producer, actual checks and limitations |
| `name.pdf` | Optional, default off; selected pages in requested order |
| `name-overview.png` | Default contact sheet of selected pages; `preview:"pages"` writes `name-page-NNN.png`, `"none"` disables PNG |
| `name-notes.txt` | Optional, whole-deck notes and ledger; UTF-8 BOM, plain text, not executed |
| `name-sources.json` | Optional, whole-deck source hashes/attributions/binding locators; no source body, rows or raw values |

PDF, images and visual preflight select 1-8 unique zero-based pages. Omitted selection means all pages and rejects more than eight when any visual output/check is requested. A full-PPTX-only delivery can use `preview:"none", preflight:false` on larger decks. The manifest and each file record their actual scope; selected visual checks do not certify the other PPTX pages. PNGs retain the 160-1600px/2MiB preview limits; `min_font_size` defaults to 16. The combined decoded files plus manifest are limited to 32MiB (`max_output_bytes` may lower this), the core response also obeys its capacity profile, and the MCP response is at most 4MiB. A single returned preview can be omitted with `include_images:false` without disabling saved previews.

All artifact generation, hashes and response sizing finish before filesystem publication. Every destination is checked first, all files are staged with exclusive temporary creation, and outputs are hardlinked in order with the manifest last. This is **not crash-atomic** or a guarantee against hostile local directory replacement. No published file is removed on failure. `BUNDLE_PUBLICATION_FAILED` returns `not_published`, `partially_published` or `published_with_error`, exact `published_paths`, `pending_filenames` and cleanup diagnostics. Inspect existing files/manifest hashes; use a new name for another complete delivery. Cancellation can occur after publication, so never assume a cancelled request wrote nothing. There is no automatic retry or resume.

`status:"complete"` means requested files were generated and published, not that preflight found no warnings/errors. Inspect `checks` and the full manifest. Structural/native preservation, renderer findings, source-binding consistency and their page scopes are distinct from Office visual parity, source authenticity/freshness, semantic truth and accessibility certification. The latter are not performed. Source attributions and locators can still be sensitive even without raw content. Guided declarations remain in notes, not authenticated source records.

Core `prepare_delivery` and SDK `session.prepareDelivery(options,{expectedRevision?,expectedHash?,signal?})` return an in-memory bundle only. File publication belongs to the MCP host; no new filesystem or network authority is added to core or SDK.

## Reproduce

```sh
npm run core:build
node tools/guided-demo.mjs .artifacts/my-guided-examples
node tools/parts-demo.mjs .artifacts/my-parts --japanese
```

Use new or empty output directories. The guided demo uses the official MCP SDK over stdio to retrieve guides, validate/create all four Japanese examples, export, reopen, edit and undo exactly. It writes the structured inputs beside each sample. The Japanese parts demo requires local Studio at port 4174 and verifies all 108 inserted slides; the normal English demo reuses screenshots from the existing all-preview test. [Authoring verification](../testing/parts-library.md) records the measured scope and known limits. No commit, publishing, or model inference is performed by these examples.