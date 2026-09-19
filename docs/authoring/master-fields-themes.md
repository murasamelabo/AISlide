# Master Fields And Themes

## Supported Contract

`Master.theme` is optional. Without it, the master uses `Design.theme`. Each
slide resolves its theme through its layout's master. Default-theme application
preserves explicit master overrides and does not rebind their slide content.
Studio, static rendering, text measurement, authored PPTX generation and native
theme edits resolve the effective owner theme. Fonts remain local; this feature
does not download, install or grant embedding rights for fonts.

Native XML remains authoritative. Theme edits change only requested modeled
attributes, retaining unknown color transforms, effects and script-font entries.
When another part references a theme, editing one owner clones the original
theme and its relationships before updating that owner's relationship. No-op
native export and Undo preserve the original package bytes. Native import uses
the first master's theme as the default and stores distinct themes as overrides;
equal themes do not require additional serialized fields.

## Fields

`set_design_field` creates or updates a `slide_number`, `date` or `footer`
placeholder for one master, optionally limited to one owned layout. The core
creates matching native master/layout/slide placeholders. Slide numbers use
`a:fld type="slidenum"`; dates use `a:fld type="datetime1"`. Footer content is
ordinary placeholder text, not an invented field type. New instances have
deterministic UUIDs; subsequent cache edits retain field identities.

The caller supplies a valid `YYYY-MM-DD` reference date. `datetime1` retains the
existing numeric month/day/year convention. `datetime2..7` use the reserved
long-date, day-month-year, month-day-year, day-short-month-year, month-year and
short-month-year forms. `datetime8..13` require explicit `HH:mm:ss`; no current
time is inferred. Only `en-US` is evaluated. Other locales, omitted times and
unknown kinds retain caches and produce warnings. The mapping follows the
[DrawingML field definitions](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.field).
`refresh_fields` updates supported slide and rich notes caches in one undoable
transaction while leaving shared template caches unchanged.
Canvas and static rendering calculate slide numbers from the current page order
without mutating the deck. Dates remain the explicitly supplied/refreshed cache.

Design updates preserve existing rich slide text. Explicit footer/date default
changes propagate to matching still-linked content; independently changed content
is not restored from a template. Mixed or unknown dynamic-field bodies reject
replacement. Signed/protected design edits and field refresh fail closed.

Native field inheritance compares semantic styles while ignoring only the UUID
and cached text of supported field instances. Actual instance paragraphs and IDs
are retained, including raw paragraph attributes and field extensions. Layout
geometry changes propagate after reopening; local transforms and local modeled
formatting remain detached. This is not permission to replace unknown text styles.

## Rich Notes And Auxiliary Masters

`Slide.notes` remains the mandatory plain projection. Optional `notes_paragraphs`
contains typed `RichParagraph[]`; joined run texts must exactly equal `notes`.
`update_rich_notes` updates both atomically, supports 8,000 Unicode scalars,
256 paragraphs and 4,096 total runs, and preserves the ordinary 4,000-scalar text
limit. An empty list clears supported notes. Formatting-only changes save too.
Native recognition includes language-only runs, indentation and point-based
spacing, including rich formatting whose plain projection is empty. Repeated
edit/save/reopen retains namespace-correct end-paragraph properties.
Native edits target one body placeholder by its native identity. Unchanged
paragraphs, other notes shapes, relationships, notesStyle and unknown parts are
retained. Only verified cache changes modify existing fields. Changed unmodeled
paragraphs, hyperlinks, multiple bodies and shared notes parts fail closed.

`Deck.auxiliary_design` is separate from slide masters/layouts:
`{width,height,notes_master?,handout_master?}`. Each master is
`{name,background,theme,elements}`. Dimensions describe the notes-page coordinate
space; native EMU dimensions remain authoritative and are not rounded on save.
New masters use actual notesMaster/handoutMaster parts, theme relationships,
presentation lists and content types. Native name, supported text/geometry,
background and theme edits retain unrelated XML, including notesStyle and
extensions. Shared themes are copied before changing one owner.

Studio's design tree exposes **Notes master** and **Handout master** with the
existing text, geometry, theme, placeholder and field controls. **Preview page**
projects page numbers without changing a shared cache. The Notes tab exposes a
notes-page preview and Review provides run emphasis, font size, paragraph lists
and alignment. Notes preview is approximate; handout print imposition is not
implemented. Native auxiliary master removal and notes-page resizing are rejected.
Omitting new optional fields from older clients preserves native content rather
than deleting masters or flattening unchanged notes.

## Interfaces

- Core: `set_master_theme`, `set_design_field`, `design_capabilities`.
- Core/MCP: `update_rich_notes` with `document`/`expected_revision`/`slide_id`/
    `paragraphs`, and `update_auxiliary_design` with `document`/`expected_revision`/
    `design`. MCP substitutes `deck_id` for the document.
- SDK: `session.setMasterTheme(masterId, themeOrNull, options)`,
  `session.setDesignField(field, options)`, `client.designCapabilities()`.
- SDK: `session.updateRichNotes(slideId, paragraphs, options)`,
  `session.updateAuxiliaryDesign(design, options)`, and
  `session.refreshFields(date, {referenceTime?,locale?,expectedRevision?,signal?})`.
  `session.fieldWarnings` describes the last successful refresh. Core and MCP
  refresh responses also include `warnings`.
- MCP: the same snake-case tools; mutations require `expected_revision`.
- Studio: **Edit masters and layouts**, then **Master theme** or **Dynamic fields**.
- Review: **Dynamic fields** has selected-text, current-master and current-layout scopes.

The typed field input is `{ master_id, layout_id?, kind, reference_date, text? }`.
Passing `null` to `setMasterTheme` restores default inheritance. Mutation helpers
reuse the existing revision, cancellation, native preservation and Undo checks.

## Limits And Verification

Table fields, automatic locale inference, every date language/calendar,
handout pagination, arbitrary native paragraph replacement and universal template
import are not implemented. This does not
claim all PowerPoint field types, automatic Office recalculation or Office visual
parity. The first native slide master's effective theme becomes the imported
default, so an independent unused authored default is not retained.

Focused regressions live in `tests/native_design.rs`, `tests/review_features.rs`,
`tools/client.test.mjs`, `tools/mcp.test.mjs` and `tests/e2e/design.spec.ts`.
They cover native create/edit/reopen, distinct fonts, owner isolation, shared-theme
copy-on-write, unknown XML retention, unique field instances, explicit dates,
page-order projection, stale revisions, cancellation, Undo and desktop/mobile UI.
On 2026-09-19, the combined comments/single-file regression run passed 69 tests
after the repeated-body-save and rich-notes recognition repairs. This is a scoped
run, not the final workspace count. Office field/notes parity and the final
native/install gates remain unverified; see the
[completion plan](../planning/editing-completion.md).

The existing POTX/THMX factory tests remain the bounded template baseline.
On 2026-09-18, cached external representatives `WithMaster.pptx` and
`templatePPTWithOnlyOneText.pptx` did not match the pinned corpus hashes; the
latter had a CFB signature. They were not used, overwritten, decrypted or
redownloaded. External representative-template qualification remains pending;
synthetic native tests do not establish arbitrary-template compatibility.