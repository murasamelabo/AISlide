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
expectedHash })`. The CLI request protocol accepts `op: "set_references"` with
the document and the same revision/hash/options. All document behavior is in
`aislide-core`; the Studio document session shares this implementation, but no
new Studio reference-editing panel is provided.

## Publication And Layout

- Only `publish: true` entries are retained. Omitted or false approval is ignored,
  including its URL; nothing is copied from private source metadata or notes.
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
  the operation rejects atomically; reserve footer space first. An individual
  URL too tall for an appendix page also rejects rather than truncating it.
  Presentations with a design need an existing empty layout for appendix pages;
  the operation does not change a branded layout or create a replacement master.
- Entries bind to stable slide IDs, not page numbers. References regenerate in
  the same transaction after edits, including slide reordering and deletion.
  Numbers retain their approved-list positions when an earlier source slide is
  deleted; gaps are intentional so existing in-body numbers do not silently shift.
  New slides receive references only when explicitly included in `slide_ids`.
- Replace the complete approved list to change citations; an empty list clears
  managed output. Repeating the same request is a no-op. Undo restores both the
  citation state and the generated elements. Direct edits to managed elements
  are rejected instead of being overwritten; use `set_references` to change them.

## Checks And Output

`preflight_presentation` reports `SOURCE_URL_NOT_VISIBLE` when explicit HTTP(S)
URLs in notes have no readable slide text or associated managed appendix entry.
Messages do not echo note URLs. A hyperlink alone, hidden text or an off-slide
frame does not satisfy the check. This is a text/frame heuristic, not an OCR,
occlusion, contrast, factuality or comprehensive citation audit; arbitrary prose,
unmanaged appendix associations and master-only citations require human review.

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