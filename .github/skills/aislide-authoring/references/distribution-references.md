# Distribution References

Read this before customer or slide-only PDF delivery, regardless of visual style.
Discover `set_references` with `get_tool_schema` in the compact MCP profile.

## Publication

- Supply only explicitly approved `publish:true` URLs with stable `slide_ids`;
  never auto-publish URLs from notes or imported source metadata.
- Inspect `publication.excluded` for omitted approvals. `supplied`, `published`
  and `excluded` are counts only; responses do not expose excluded URLs.
- Use `title:"参考資料"` for Japanese decks; the default is `References`.
- Register the same URL as the notes after normalization. Hostname case and an
  omitted root slash normalize, but `/azure/` and `/azure` remain different paths.
  No URL is fetched to test redirects or equivalence.
- Enclose Unicode URLs in parentheses or quotes, or percent-encode CJK paths to
  distinguish them from following prose. Warnings never echo raw note URLs.
- Existing notes and source metadata are preserved, not sanitized. Review them
  before publication and use the explicit clean-copy workflow when needed.

## Layout And Review

- `auto` and `footnotes` prefer a clear bottom area; long or obstructed references
  fall back to an appendix. `appendix` always creates reference pages with full
  names and URLs, plus numbered markers on the cited slides.
- Explicitly select `placement:"appendix_only"` for dense imported slides:
  no marker or footnote is added to cited slides. Separate appendix captions
  show current 1-based deck page numbers beside full approved names and URLs.
  Only URL rows are links. Private slide titles and notes are never caption text.
- On 1280x720 slides reserve x32..1248/y616..672 for 16px markers, body bottom
  at or above y608, and existing source/page labels at y680. Longer titles or
  more reference numbers need a taller band. Body content is never moved/shrunk.
- Markers search only the bottom quarter (`y >= 0.75 * height`) with at least
  8px clearance. No free band fails atomically, reporting the bottommost
  candidate frame coordinates and only obstacle IDs intersecting that frame.
  Clearance-only failures may have no intersecting IDs. Preview every affected
  slide; success alone does not guarantee a suitable reading order. Explicitly
  choose `appendix_only` or obtain approval to reserve space; never silently
  change an imported layout or drop references. Existing-footer reuse remains
  unsupported.
- Ordinary edits keep reference placement and appendix order. `appendix_only`
  captions update on reorder, deletion and appendix movement without reflow or
  changes to name/URL elements. Frames reserve measured page-number widths up
  to the core's maximum 256 pages; insufficient space rejects instead of
  shrinking or hiding overflow. Stable slide IDs stay internal; printed captions
  provide the visible association. Undo and native PPTX reopen preserve it.
  Existing saved modes remain compatible. Resolve
  `REFERENCE_COLLISION` by repairing body content or explicitly calling
  `set_references` to replan. Managed reference elements reject direct edits.
- Include appendix pages in export selections. Review at most 8 pages per call,
  inspect `page_scope`, and check overflow, links and notes-only URL warnings.
  A subset PDF retains full-deck page numbers, not subset-local positions;
  imported footer labels stay untouched. Use the complete delivery deck for
  direct printed correspondence and explicitly refresh after external reorders.
  Inspect both PDF URL text and link targets; structural checks are not Office
  visual parity. Never overwrite the source presentation.