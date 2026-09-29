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
- On 1280x720 slides reserve x32..1248/y616..672 for 16px markers, body bottom
  at or above y608, and existing source/page labels at y680. Longer titles or
  more reference numbers need a taller band. Body content is never moved/shrunk.
- The current upward search can put markers between body sections on imported
  slides, or reject dense slides without a free full-width band. Preview every
  affected slide; success alone does not guarantee a suitable reading order.
  Error obstacle IDs are not ranked by repair priority. There is currently no
  marker-free or existing-footer reuse mode. Do not silently drop references or
  change an imported layout; obtain approval to reserve space first.
- Ordinary edits keep reference placement and appendix order. Resolve
  `REFERENCE_COLLISION` by repairing body content or explicitly calling
  `set_references` to replan. Managed reference elements reject direct edits.
- Include appendix pages in export selections. Review at most 8 pages per call,
  inspect `page_scope`, and check overflow, links and notes-only URL warnings.
  Inspect both PDF URL text and link targets; structural checks are not Office
  visual parity. Never overwrite the source presentation.