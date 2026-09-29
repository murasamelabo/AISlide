# Distribution References

For customer or slide-only PDF delivery, call `set_references` before finalizing.
This is an explicit publication step, not automatic extraction from speaker notes
or ingested sources. Discover its schema with `get_tool_schema` when using the
compact MCP tool profile.

```json
{
  "deck_id": "<current handle>",
  "expected_revision": 0,
  "expected_hash": "<current hash>",
  "options": {
    "placement": "auto",
    "title": "References",
    "font_size": 16,
    "entries": [
      {
        "id": "azure-docs",
        "name": "Microsoft Learn: Azure",
        "url": "https://learn.microsoft.com/azure/",
        "slide_ids": ["slide-1"],
        "publish": true
      }
    ]
  }
}
```

The SDK equivalent is `session.setReferences(options, { expectedRevision,
expectedHash })`; `session.referencePublication` exposes the latest accepted
operation's `{ supplied, published, excluded }` counts as a detached snapshot.
MCP and core responses include those counts as `publication`, also for no-ops
and all-unapproved lists. Other accepted mutations clear the SDK snapshot.
The CLI request protocol accepts `op: "set_references"` with
the document and the same revision/hash/options. All document behavior is in
`aislide-core`; the Studio document session shares this implementation, but no
new Studio reference-editing panel is provided.

For Japanese decks, set `title:"参考資料"` explicitly; the default is `References`.

## Publication And Layout

- Only `publish: true` entries are retained. Omitted or false approval is ignored,
  including its URL; nothing is copied from private source metadata or notes.
  Check `publication.excluded` for approval omissions; counts never expose URLs.
- HTTP(S) domain URLs only; local paths, IP addresses, localhost, embedded
  credentials, other schemes and whitespace are rejected. No URL is fetched.
  Approval remains the author's responsibility: a domain URL is not proof that
  its contents, query parameters or access tokens are suitable for distribution.
- `auto` and `footnotes` prefer full name/URL footnotes in a clear bottom area.
  Long, numerous or obstructed footnotes fall back to numbered appendix pages.
  `appendix` always uses those pages with markers on cited slides.
  Explicitly select `placement: "appendix_only"` for a marker-free alternative:
  cited slides receive no footnote or marker, even with full-slide dense content.
  The appendix retains full approved names and URLs and adds separate `Slides:`
  captions with the associated current 1-based deck page numbers. Caption text
  never reads private slide titles, notes or source metadata.
  Font size stays at the requested 16-32px.
  Low-contrast Office theme hyperlink colors also trigger appendix placement;
  the selected appendix theme must provide readable links on white.
- Body content is never moved or reduced. If even a numbered footer cannot fit,
  markers search the nearest free full-width horizontal band upward from the
  bottom, only in the bottom quarter (`y >= 0.75 * height`). Candidates retain
  at least 8px clearance from transformed element bounds, 32px side margins and
  a 24px bottom margin. No free band rejects atomically: the error identifies
  the slide, bottommost candidate frame (`x`, `y`, `width`, `height`) and only
  obstacle IDs actually intersecting that frame. It may report no intersecting
  IDs when clearance or the bottom-quarter constraint alone prevents placement.
  Reserve space with approval or explicitly retry `set_references` with
  `appendix_only`; the fallback is never chosen silently. An individual caption,
  name and URL that cannot fit one appendix page also rejects without truncation.
  Presentations with a design need an existing empty layout for appendix pages;
  the operation does not change a branded layout or create a replacement master.
- Entries bind to stable slide IDs, not page numbers. Only `set_references`
  chooses placement. Ordinary edits preserve generated frames, footnote/appendix
  choices, z-order and the user's appendix page order. Deletion removes only
  references no longer used and empty managed pages, in the same Undo.
  Citation numbers such as `[2]` retain their approved-list positions when an earlier source slide is
  deleted; gaps are intentional so existing in-body numbers do not silently shift.
  New slides receive references only when explicitly included in `slide_ids`.
- `appendix_only` page captions update after reorder, deletion and appendix
  movement, including removal of an empty appendix page. Their frames and the
  separate name/URL elements remain fixed; deleted associations do not reflow
  the remaining entries. Each referenced slide reserves the measured widest
  page-number slot across the core's maximum capacity of 256 pages, including
  separators and wrapping space. Capacity and frame-fit failures are explicit,
  not hidden overflow or font shrinking. Caption ownership hashes are captured
  after text updates, so subsequent edits and Undo remain verifiable.
  Existing persisted `auto`, `footnotes` and `appendix` states remain compatible
  and are not silently converted to the new mode.
- Replace the complete approved list to change citations; an empty list clears
  managed output. Repeating the request on an unchanged deck is a no-op; calling
  it after layout edits explicitly replans placement. Undo restores both the
  citation state and the generated elements. Direct edits to managed elements
  are rejected instead of being overwritten; use `set_references` to change them.

## Reserve Marker Space

For a 1280x720 technical briefing, reserve `x=32..1248, y=616..672` for a
single-line marker at the default 16px size. Existing source/page labels can
remain at `y=680..704`; the 8px gap is intentional. Longer titles or many
reference numbers need a taller band, measured without shrinking the body.
These are authoring coordinates, not a fixed requirement for imported slides:
the upward search can use a sufficiently clear band within the bottom quarter. Footer
occupancy therefore sends URLs to the appendix, not an automatic failure.

Imported decks without reserved space can use `appendix_only` without changing
the original slides. Marker modes reject when the bottom quarter has no suitable
band; they no longer search the upper body. Preview every affected slide for
reading order and whitespace; success alone is not sufficient. Diagnostic IDs
describe the bottommost failed candidate, not all obstacles or a ranked repair
plan. Existing-footer reuse is not implemented; obtain approval before changing
imported layouts rather than silently replacing their labels.

Footnote names/numbers and URL rows are separate text boxes. Only the URL row
is hyperlinked in both PPTX and native PDF; page captions are plain text in a
separate managed element. Appendix text uses the selected
empty layout's theme `dk1`, falling back to black only when necessary for
contrast on its white background; no theme or branded master is rewritten.

## Checks And Output

`preflight_presentation` reports `SOURCE_URL_NOT_VISIBLE` when explicit HTTP(S)
URLs in notes have no readable slide text or associated managed appendix entry.
Messages do not echo note URLs. A hyperlink alone, hidden text or an off-slide
frame does not satisfy the check. This is a text/frame heuristic, not an OCR,
occlusion, contrast, factuality or comprehensive citation audit; arbitrary prose,
unmanaged appendix associations and master-only citations require human review.

Japanese/CJK prose and full-width punctuation terminate bare note URLs; parsed
URLs are normalized before comparison (including hostname case and an omitted
root slash). Register the same URL as the notes after normalization: a non-root
path such as `https://learn.microsoft.com/azure/` is distinct from
`https://learn.microsoft.com/azure`. No redirect or equivalence check is fetched.
URLs enclosed in quotes, angle brackets, Japanese quotation marks
or parentheses are parsed intact, preserving Unicode domains and paths. Enclose
or percent-encode paths containing literal CJK text to distinguish them from
surrounding prose. Raw note URLs remain absent from
warnings because they may contain private hosts, paths or query tokens.

`REFERENCE_COLLISION` identifies the slide and overlapping element IDs when
normal editing introduces content over fixed references. The edit is accepted
without silently moving references or adding pages. Move the listed body
element or explicitly call `set_references` to choose new placement, then
preview and recheck the affected pages.

Visible references are ordinary editable OOXML text with hyperlink relationships,
so they are present when PowerPoint converts the slides to PDF. AISlide's native
PDF output also writes URI annotations for visible text/shape hyperlinks. Full
URL text remains available even in printed output. PDF viewers and conversion
software may differ; inspect both the text and actual link targets.

PPTX preserves notes and existing source/provenance information. This feature is
not a privacy scrub of an existing presentation. Review notes and metadata before
distribution, and use the existing clean-copy workflow for explicit removal.

Limits: 64 supplied references, 128 slide IDs per reference, 2048 characters per
URL, 200 per name; normal document/output limits still apply. Preflight selects
at most eight pages per call. Delivery PDF/preview selection may be partial, so
include reference appendix pages explicitly when selecting pages.
Caption page numbers refer to the current complete deck order, including the
positions occupied by appendix pages. They are not imported/custom footer
labels, which remain untouched. A subset PDF retains full-deck numbers; it is
not renumbered to subset-local pages. Use a complete delivery deck for direct
printed page correspondence. External reordering outside AISlide is not a live
caption field: reopen and explicitly refresh references before distributing it.
Native PPTX reopen and AISlide transactions preserve the managed association;
native PDF emits readable captions, full URL text and URI links. These checks
do not establish PowerPoint/Office visual parity.

Regression commands (run from the issue worktree):

```text
node tools/cargo.mjs test -p aislide-core --test references
node tools/cargo.mjs build -p aislide-cli
node --test tools/references.test.mjs
```

To retain the synthetic MCP fixture for Windows PowerPoint verification, set
`AISLIDE_REFERENCE_ARTIFACTS` to a new local directory before the Node test.
Then run `tools/verify-references-powerpoint.ps1 -Path <fixture.pptx>
-OutputDirectory <new-directory>` with PowerPoint closed. The script opens only
the generated fixture read-only, checks text/link presence, exports separate
PDF/PNG files and verifies the source SHA256. Inspect the captures separately.
Set `AISLIDE_POWERPOINT_REFERENCE_PDF` to that generated PDF and run:

```text
node tools/cargo.mjs test -p aislide-core --test references powerpoint_pdf_preserves_reference_text_and_link_targets -- --ignored --exact
```