# Local Proofing and Format Painter

## Implemented Scope

G04 remains **partial**, not PowerPoint proofing parity. Find/replace and font replacement remain available. The new Proofing tab adds explicit proofing-language assignment, local word-list checks with suggestions, local synonym lookup, and exact bilingual **Term translation**. There is no grammar engine, full English dictionary, sentence translation, or model inference in this path.

G11 format painting supports **top-level text, rectangles, polygons, shapes, pictures, tables and charts**. Typed snapshots copy supported appearance without content or geometry. Nested groups, connector formats, font-file transfer and arbitrary unrepresented Office formatting remain unsupported. This does not complete every G11 feature: additional WordArt presets remain blocked by the shared static renderer's explicit text-warp rejection.

## Local Dictionary

Open **Find and replace > Proofing**. Choose a text object and a language tag, for example `en-US`, `fr-FR`, or `ja-JP`. **Apply proofing language** sets that tag on the object's runs in one undoable transaction; it does not install dictionaries or translate content. Empty, hidden, locked, and nested targets are rejected for language assignment.

**Dictionary file** accepts an explicitly selected UTF-8 `.txt` word list (one word per line), or `.json` containing words, synonyms, and bilingual term maps. A single leading UTF-8 BOM is accepted. Invalid UTF-8, unknown JSON fields, malformed language tags, and language mismatches fail without replacing the active dictionary. No filesystem paths are accepted by the core API. Supply only data you are licensed to use; AISlide does not verify or confer those rights.

```json
{
  "language": "en-US",
  "words": ["hello", "world", "report"],
  "synonyms": { "report": ["summary", "account"] },
  "translations": { "ja-JP": { "report": "報告" } }
}
```

This is a tiny illustrative mapping, not a complete dictionary or context-sensitive translation. Keys are matched using Unicode lowercase conversion, and language tags case-insensitively. Duplicate case-folded mapping keys are rejected. Translation looks up the complete supplied term; it never composes a sentence or silently falls back to a model.

The built-in en-US sample contains only representative common words and two illustrative synonym entries. It is deliberately labeled **not a complete dictionary**, and returns `sample_dictionary:true` and `complete_dictionary:false`. Supplied lists also retain `complete_dictionary:false`; their completeness is not certified. Other languages require a matching supplied dictionary. No OS/global dictionary installation or npm files are required by the Rust core.

Word-list checking reports words absent from the list, not confirmed spelling mistakes. It recognizes alphabetic sequences with internal apostrophes; it does not implement Hunspell affixes, stemming, locale-specific segmentation, Unicode normalization, or contextual grammar. Suggestions are deterministic, at most five lexicographically ordered entries within one insertion, deletion, substitution, or adjacent transposition. No ranking or comprehensive linguistic quality claim is made. Hunspell `.aff/.dic` files are not supported by this slice.

Limits: 512 KiB raw and serialized dictionary, 10,000 words (64 scalars each), 2,000 synonym keys (16 alternatives each), 16 target languages (2,000 mappings each), 128-scalar mapping terms, 4,000-scalar checked text, and 100 unknown-word results. Exceeding a limit rejects the operation, not a truncated success. Offsets are Unicode scalars with exclusive ends, not JavaScript UTF-16 offsets.

**Check spelling** checks the chosen object's current text. Clicking a suggestion reuses the core literal-search and selected-replacement path with exact expected text, scalar range, revision, and native-preservation checks. Thesaurus and term-translation results are read-only. Language selection for checking is independent of document language assignment. Dictionary data/results are held only in the panel's memory, never saved inside the presentation or fetched externally. Closing the Proofing tab clears them. Cancellation discards late results; edits remain subject to the SDK's cancellation and Undo contract.

## Painter

Select one supported object and choose **Copy format**. Rich text sources offer paragraph and run selectors; their indices are zero-based in APIs and one-based in UI labels. Non-text sources require zero indices. Copy validates the source and captures only a typed style snapshot. It changes neither document revision nor Undo history. The Studio snapshot is usable only in the same document session and never uses the system clipboard.

Select one or more compatible targets, then **Apply format**. All targets are validated and applied atomically in a single revision/Undo entry. One missing, duplicate, hidden, locked, incompatible, or native-unrepresentable target rejects the entire batch. The API accepts 1-128 unique IDs on one slide.

Copied formatting: resolved source run overrides over its frame defaults (bold, italic, underline, font size, color, family token/name, baseline), selected paragraph alignment/list/indent/spacing/tabs, and text-frame vertical alignment. The same paragraph style is applied to every target paragraph, retaining their count and run content. Baseline defaults to zero; absent source highlight and language do not clear the target's values.

Preserved: IDs, coordinates, dimensions, rotation/flips, paths, shape presets/adjustments, picture crop/mask/bytes/SVG/alt, text, paragraph count, run highlight/language, hyperlinks, field UUID/kind/cache, placeholder identity, chart values/categories/statistical settings, table rows/track sizes/merges, and unrelated objects. Layout inheritance is detached on painted text. Target text-frame font defaults remain intact; effective formatting is written to rich runs/paragraphs. Adjacent identical ordinary runs may coalesce through the existing rich-text API. No font binaries, embedding rights, source bindings, or external relationship payloads are cloned. Theme tokens remain tokens within the current document theme.

Object snapshots are a strict union. Legacy `{run, paragraph, vertical}` text snapshots still work. New text snapshots optionally include surface, effects and the already-supported text warp. Filled objects copy fill, compatible outline, opacity, gradient and effects. Pictures copy opacity/effects only; masks remain geometry. Rectangles cannot receive a nonzero outline because that element type has no outline field. A shape snapshot with a surface requires a compatible filled text shape, not a plain text box.

Tables require the same row/column count, copy base font size plus per-cell fill, outline, padding, alignment and text style, and retain target track geometry and merges. Each rich source cell uses its first paragraph/run as the style reference for the target cell's existing paragraphs/runs; it does not clone mixed source content. Charts require matching series count and copy series colors, legend, data-label formatting and axis number formats. Chart family, series names/values, axis scales, trendlines, error data and statistical/hierarchy options stay unchanged. Invalid chart-family/style combinations reject atomically.

## Slide Eyedropper

**Advanced object settings > Slide eyedropper** samples a composited pixel of the current slide rendered locally by the shared static renderer. **Load slide color preview** provides a pointer target. Numeric **Sample X/Y**, keyboard activation, slide color swatches, a color input and **Color HEX** provide alternatives. **Apply sampled color** explicitly applies fill, outline or text color to the selected supported object in one undoable transaction. Sampling alone creates no history. Applying a flat fill clears an existing gradient; alpha/effects otherwise remain.

The SDK/MCP `sample_slide_pixel` input is `{slide_id, x, y}` with zero-based integer slide-pixel coordinates inside the canvas. It returns `{color, rgba, x, y, warnings, office_parity_verified:false}`. Studio uses `session.sampleSlidePixel(slideId, x, y)`. No screen capture, global eyedropper, remote image, OS clipboard or browser permission is used. Font substitutions and other renderer warnings are returned. A slide containing a statically unsupported effect, including existing text warp, can reject sampling instead of silently sampling an incomplete rendering. This is the core's rendered color, not an Office or display-color-management parity guarantee.

Native edits use the unchanged package-preservation guard; unknown or unsupported native formatting can reject before commit. Native reopen and exact-byte Undo tests do not establish Office rendering parity. Office was not launched for this work, and no user presentations or protection settings were modified.

## Shared API

Core operations: `import_proofing_dictionary`, `proof_text`, `set_proofing_language`, `copy_format`, `apply_format`, `sample_slide_pixel`. MCP exposes the same names with strict schemas. Mutating tools require `expected_revision`; captures/lookups are read-only. MCP keeps existing output restrictions and creates no files for these operations.

```javascript
const dictionary = await client.importProofingDictionary({
  language: 'en-US', format: 'wordlist', content: 'hello\nworld\nreport\n'
});
const result = await client.proofText({ text: 'hello wurld', language: 'en-US', dictionary });
const style = await session.copyFormat('slide-1', { id: 'source', paragraph_index: 0, run_index: 0 });
await session.applyFormat('slide-1', { ids: ['target'], style }, { expectedRevision: session.revision });
await session.setProofingLanguage('slide-1', { id: 'target', language: 'en-US' }, { expectedRevision: session.revision });
```

All calls support the existing `signal` cancellation option. There is no paid endpoint, hidden remote content lookup, global dictionary installation, or translation-provider consent bypass. Full translation would require a separate explicitly consented provider integration and a real output contract; it is not simulated through report generation.

## Verification

```sh
node tools/cargo.mjs test -p aislide-core --test proofing --locked --offline
node tools/cargo.mjs build -p aislide-cli --locked --offline
node --test tools/proofing.test.mjs
npx playwright test tests/e2e/proofing.spec.ts
node tools/cargo.mjs test --workspace --locked --offline
npm run build
```

Use an unused `AISLIDE_TEST_PORT` for browser tests when other Studio servers are running. Tests generate synthetic documents in memory; no existing presentation is opened. Focused checks cover dictionary bounds/languages/scalar offsets, strict MCP schemas, nonmutating capture, cancellation, mixed runs, fields, hyperlinks, native reopening, atomic rejection, and Undo. Desktop/mobile UI tests exercise the real local core transport.