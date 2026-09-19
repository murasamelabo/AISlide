# G28: Opt-In Document Fonts

AISlide supports a bounded, opt-in font workflow. This is **not Office font compatibility certification** and does not establish that a user owns a font license.

## Studio

1. Open **Fonts** and choose a local `.ttf` or `.otf` file. Only the explicitly selected file is read. No font is downloaded or installed.
2. Review family, style, OS/2 embedding permissions, file hash and available copyright/license notices. Notice URLs are displayed as text, never fetched.
3. Confirm that the font owner's license permits document embedding and editing, then choose **Embed full font**.
4. Use **Apply to selected text** to assign that family to the selected text box or shape. Attaching a font does not change any text automatically. Other existing font-formatting operations remain available.
5. Save to a new PPTX using the normal save workflow. Unused staged families are omitted from PPTX output. Undo/Redo includes font attachment and assignment as separate operations.

Reopening a PPTX retains supported font data but resets loading consent. Inspect and acknowledge the font again to enable its use. Browser faces are loaded from memory, tracked individually, and removed on Undo, document replacement or component disposal; pending loads cannot add a face after disposal. No other faces are removed. This is document-local use inside AISlide, not an OS font installation or an authorization system.

## Bounds And Rights

- At most **8 faces, 12 MiB per file and 24 MiB combined decoded font bytes**. The default large profile still requires the entire document, including base64 fonts and immutable native origin, to fit 32 MiB; archive output is limited to 16 MiB. Legacy/standard limits remain selectable and smaller. A valid 12 MiB face is not a guarantee that its reopened document fits. See [Phase 5 capacity and actual CJK evidence](../testing/phase5-recovery-capacity.md).
- Only standalone static outline SFNT TTF/OTF. WOFF/WOFF2, TTC/OTC collections, variable fonts, CFF2-only, SVG/color fonts and metadata-only fixtures are not loadable through this workflow.
- Installable (`fsType` usage bits zero) and Editable (`0x0008`) can be embedded after acknowledgement. Restricted, Preview/Print, bitmap-only, absent/malformed OS/2, unknown bits or ambiguous permission combinations fail closed for new embedding/loading. The conservative policy also rejects ambiguous legacy flags instead of assuming the least restrictive interpretation.
- Preview/Print fonts are inspectable but not loaded into an editable document. There is no preview/print-only document mode or override switch.
- `0x0100` means no subsetting. AISlide embeds the entire selected SFNT file unchanged, including permissions, names and license metadata. “Full font” means **no additional subsetting by AISlide**; it cannot prove an upstream file was never subsetted. Synthetic bold/italic files are not generated.
- Metadata inspection is bounded to 128 non-overlapping, ordered tables, 64 KiB/256 name records, and 8192 encoded bytes per name. Generic font parsers are not a native browser font sandbox or a comprehensive font sanitizer. Keep the browser/runtime updated.
- Family/style is derived from file bytes. A different file cannot silently replace an occupied family/style. Identical bytes deduplicate. Binary parts use SHA-256-based identities, not filesystem paths.

## Native Format

The mapping is independently implemented from public specifications, without a PPTX engine:

- PresentationML `p:embeddedFontLst` contains one `p:embeddedFont` per used typeface, with `p:font typeface="..."` and ordered `p:regular`, `p:bold`, `p:italic`, `p:boldItalic` entries.
- Each style references its presentation-root font relationship using **`r:id`**, not `r:embed`. Relationship type is `http://schemas.openxmlformats.org/officeDocument/2006/relationships/font`.
- New `.fntdata` parts have `application/x-fontdata` content type and an **uncompressed EOT v1 (`0x00010000`) header**. Header fields and UTF-16 name strings are little-endian; the exact SFNT payload remains big-endian. Flags are zero: no compression, subsetting, XOR or web-object policy. This is not Word's GUID-based obfuscated font format. Raw TTF is not merely renamed `.fntdata`.
- New font additions set `embedTrueTypeFonts="1"` and `saveSubsetFonts="0"`. Unrelated native edits do not rewrite those attributes or original font bytes.
- Imported uncompressed EOT v1 is inspectable/loadable only within the same bounds and rights policy, with matching EOT/SFNT `fsType`. Other encodings (including compressed/XOR EOT, EOT v2, raw SFNT parts and Word obfuscation), restricted fonts and unknown parts remain opaque and unchanged. No decompression, deobfuscation, installation or external-relationship fetch is attempted.
- Replacing/removing original native fonts and adding another style to an already embedded native family are deliberately rejected. Add all required style files before the first PPTX export. New distinct families can be added through the font-only save hook. Unsupported imported font inventories remain available through `inspect_pptx_fonts` even when not loadable in Studio.

Font use is projected from explicit family references and the existing theme font slots. This is not a glyph-by-glyph Office font-use oracle. Unsupported/unused staged resources are not promised portable merely because they are present in the editing model.

## Core, SDK And MCP

Core operations: `inspect_font`, `inspect_pptx_fonts`, `list_fonts`, `embed_font`, `set_font_usage`.

SDK: `client.inspectFont(base64)`, `client.inspectPptxFonts(base64)`, `session.listFonts()`, `session.embedFont({ base64, license_acknowledged: true }, { expectedRevision })`, `session.setFontUsage(sha256, acknowledged, { expectedRevision })`. Assignment is separate via existing `formatText`/`replaceFont` operations.

MCP exposes the same five snake-case tools with strict schemas. Mutations use `deck_id`, `expected_revision`, existing SDK serialization and Undo. Callers must obtain actual user consent before asserting acknowledgement; the boolean is an attestation, not license verification. Inspection returns metadata and hashes without font file paths or raw binary data. Explicit PPTX inspection accepts supplied package bytes and never bypasses protected-input guards.

Document fonts live in optional `Deck.embedded_fonts`. Font-free documents omit the field. Necessary existing Deck constructors initialize an empty vector; shape/text native mapping is unchanged. Original package parts remain authoritative on native save.

## Rendering And Dependencies

Current lockfile: `cosmic-text` 0.19.0, `fontdb` 0.23.0, `ttf-parser` 0.25.1 and `resvg` 0.48.1. `fontdue` is not a dependency. No new dependency was needed for G28.

Layout and static rendering clone the installed-font database only when consented document faces exist, replace matching-family database entries in the local clone, and load the selected bytes in memory. The native family name is added as a local alias when typographic naming differs. The temporary font system is dropped after the operation; embedded bytes are not inserted into the process-wide installed-font cache. Existing glyph fallback and missing-glyph reports remain active. Static export shapes and outlines text before resvg/SVG-to-PDF conversion, so the integration precedes outlining; it does not add an independent usvg global font registry. PDF remains outlined text, not selectable/tagged text or embedded-font PDF.

## Phase 5 CJK Evidence

The opt-in fixture helper uses official pinned OFL Noto Sans JP and isolated FontTools 4.59.2 full static instancing at weight 400. All 17,936 glyphs remain; no subsetting is performed. The 5,766,884-byte static TTF passed actual SDK/MCP embedding, Japanese measurement, native reopen/edit/Undo, and Studio desktop/mobile loading and assignment. Reopened document size is 12,153,815 bytes, demonstrating why standard's 8 MiB is insufficient. No font/model binary is committed or installed globally. `tools/cjk-font-fixture.mjs --accept-ofl` records exact source, license and output hashes in the ignored fixture directory. FontTools is test preparation only, not a runtime dependency. See [full reproduction, timing and remaining gates](../testing/phase5-recovery-capacity.md#4-actual-cjk-qualification).

PowerPoint recognition and visual parity, arbitrary CJK files and representative CFF OTF remain unverified. Variable fonts and TTC remain rejected by the product; the external fixture-instancing step is not native variable-font support.

## Historical G28 Evidence And Remaining Gate

- Ten passing core font tests cover inspection, consent/rights, deduplication, stale revisions, Undo, name allocation bounds, malformed ranges, four native style slots, EOT headers, exact imported SFNT bytes, no-op byte equality, font retention during text edits, opaque/external/missing font reference retention, protected-container rejection, resource limits, document-local loading, family aliases and missing glyphs. Behavioral RED failures were recorded before each implementation slice.
- Real local OFL Noto Sans from the already-installed cosmic-text dependency passes SDK/MCP inspection, attachment, explicit assignment, export/reopen, fresh consent, measurement and PDF export. Source font files are not copied into repository source or published.
- Three Edge browser cases cover desktop/mobile consent, file selection, assignment, malformed input, Undo/Redo and cleanup on document replacement. Screenshots were inspected at 1440 and 390 pixels.
- The installed Open XML SDK 3.5.1 validated the final temporary Noto-backed PPTX, SHA-256 `879c91e147030424eae4150640ea47c53029333ce20b44328e67603e32d8ba01`; source font SHA-256 `2ec33f84606cbaa0a1a944488e14f97faf2f6a25ecdd8354f5358f06da13c7d9`. This validates package/XML structure, **not the EOT payload's acceptance by PowerPoint**. No Office process, user font directory, protected presentation or global font installation was touched. Full native recognition, Office visual comparison and representative OTF/CJK qualification remain the parent's explicit follow-up gate.
- Two final SDK/MCP tests, three browser tests, frontend build, TypeScript and source encoding checks passed. Lint passed with no new font warnings; existing warnings in DocumentSetupPanel/TextControls remain. The 55 neighboring layout/static/native tests also passed during this task.
- Required workspace testing initially stopped at the concurrently edited capacity test `large_inverse_receipts_do_not_copy_unchanged_origin` (`provenance resources`). The final full-workspace attempt was blocked linking the concurrently held `review_features` executable (`Permission denied`). Neither was bypassed and no foreign process was stopped. Keep whole-repository qualification separate from this bounded feature evidence.

Reproduce locally, without downloading dependencies: `node tools/cargo.mjs test -p aislide-core --test fonts --locked --offline`, `node tools/cargo.mjs build -p aislide-cli --locked --offline`, `node --test tools/fonts.test.mjs`, and `node node_modules/@playwright/test/cli.js test tests/e2e/fonts.spec.ts`. The Noto-dependent cases skip explicitly if the local licensed fixture is absent. Browser tests require local Edge and a free `AISLIDE_TEST_PORT`.

## Public References

- [Microsoft: OS/2 fsType and font restrictions](https://learn.microsoft.com/en-us/typography/opentype/spec/os2#fstype)
- [Microsoft: SFNT structure and required tables](https://learn.microsoft.com/en-us/typography/opentype/spec/otff)
- [Microsoft: PowerPoint Font Part implementation](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/ea097c57-5794-4624-b08e-017b47051b1d)
- [Microsoft: unique and used embedded font families](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/fb2ecab1-17a1-4552-bac3-8df949321ba8)
- [Microsoft: EmbeddedFont schema and r:id example](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.presentation.embeddedfont)
- [Microsoft/Monotype EOT format submission: v1 layout and processing flags](https://www.w3.org/submissions/EOT/)