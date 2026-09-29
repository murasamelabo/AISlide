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
  `appendix` always uses those pages. Font size stays at the requested 16-32px.
  Low-contrast Office theme hyperlink colors also trigger appendix placement;
  the selected appendix theme must provide readable links on white.
- Body content is never moved or reduced. If even a numbered footer cannot fit,
  markers search the nearest free full-width horizontal band upward from the
  bottom. Candidates sit 8px above existing transformed element bounds, with
  32px side margins and 24px top/bottom margins. No free band rejects atomically
  with the slide ID and blocking element IDs; reserve space and retry
  `set_references`. An individual
  URL too tall for an appendix page also rejects rather than truncating it.
  Presentations with a design need an existing empty layout for appendix pages;
  the operation does not change a branded layout or create a replacement master.
- Entries bind to stable slide IDs, not page numbers. Only `set_references`
  chooses placement. Ordinary edits preserve generated frames, footnote/appendix
  choices, z-order and the user's appendix page order. Deletion removes only
  references no longer used and empty managed pages, in the same Undo.
  Numbers retain their approved-list positions when an earlier source slide is
  deleted; gaps are intentional so existing in-body numbers do not silently shift.
  New slides receive references only when explicitly included in `slide_ids`.
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
the upward search can use any sufficiently clear horizontal band. Footer
occupancy therefore sends URLs to the appendix, not an automatic failure.

Footnote names/numbers and URL rows are separate text boxes. Only the URL row
is hyperlinked in both PPTX and native PDF. Appendix text uses the selected
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
root slash). URLs enclosed in quotes, angle brackets, Japanese quotation marks
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