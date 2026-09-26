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

## Choose An Authoring Path

| Need | Recommended path | Boundary |
| --- | --- | --- |
| A 39-page technical outline with keyword headings | Guided `technical-explainer` with `authoring:{headline_style:"keyword",slide_limit:39}` | Supply the actual pages and evidence ledger; 39 is a ceiling, not generated filler |
| A decision meeting | Guided `consulting-decision` | Multi-page decks still require 3-6 issues, opening C02 and closing C03; keyword mode does not remove those requirements |
| Common charts, processes and described architecture nodes | Parts with typed `data`; native graphs with node `detail` | Inspect catalog capabilities and fit; descriptions remain editable text |
| Reusable parts and graphs across existing slides | One mixed `apply_operations` with `add_part` / `add_graph` | Retains editable objects and managed metadata; supply final layouts before insertion |
| Exact freeform layout or polygons | `create_presentation`, then complete `add_elements` / `apply_operations` at final frames | Supported native objects, not arbitrary PPTX replication; batch before selected-page preview/preflight |
| A structured report | `compile_report` | Deterministic fixed seven layouts: cover, metrics, table, columns, statement, chart, process; not a general PPTX replication engine |
| Reuse authored slides | `import_slides` with an existing authored source handle | Same canvas, current metadata and licensed fonts; native cross-package slide copying is unsupported |

Design presets are optional styles, not a prerequisite for these paths. The
seven presets are unchanged. Direct part insertion with `PartSpec.layout`
bypasses automatic preset-region fitting; guided creation still fits its
part into the guided body region. Use direct `addPart` or typed elements when
absolute slide placement is the requirement.

For reusable direct content, prefer managed variants in `apply_operations`;
`addPart` remains compatible. Use complete typed elements for an explicitly
unmanaged composition, not as a substitute for reusable part metadata.

## Choose A List Composition

Use the information relationship, not item count alone, to select a part.
For equal-status topics, the catalog recommends `list/rows` for 2-8 aligned
heading/detail rows, `list-horizontal/columns` for 2-4 open columns, and
`list-enumeration/grid` for 2-8 unordered peers. These three new presets use
native text and whitespace without automatic boxes, accent rails, numbers,
icon discs or per-item colors. Use a flow for stages and a comparison for
shared evaluation criteria instead of styling both as lists.

The 111-entry catalog retains the original 108 IDs and examples. Optional
`recommended`, `use_when` and `avoid_when` fields describe the new alternatives.
Studio presents them first on initial category selection; explicit choices,
saved drafts and existing part instances are not replaced. The MCP prompt,
workflow resource and all four core best-practice guides carry the same
selection policy. They guide the calling author, not an automatic deck-wide
redesign service. Keep meaningful repeated structures consistent and change
composition only when the information calls for it; do not randomize layouts.

Text limits and installed-font fitting still apply. Long labels, many items
or a small explicit region can be rejected at the existing font floor. Keep
source wording and citations, and review the actual output before claiming
visual quality. The new native layouts are independently implemented; no
reference-site slide images, branding or proprietary artwork are bundled.

## MCP Workflow

The default connection is lightweight: ten common tools are initially exposed. `discover_tools({query})` searches advanced operations without their full schemas; `get_tool_schema({name})` returns the needed schema and publishes that tool. The four most recently requested advanced tools remain in the additional list. Existing direct calls are still valid; `--tool-profile full` restores full discovery and legacy response defaults.

For ordinary freeform authoring, use `create_presentation`, `edit_slides`, and bounded `apply_operations` batches with final content/geometry. Compact mutation replies carry the next revision/hash. `list_decks` restores lost handles and last successful operations/exports; `get_deck_summary` returns small paginated slide or element indexes without core calls, image/source bytes, notes or rendering. This removes the need for a manual progress file or repeated full-document reads just to continue working. It does not persist across server restarts or determine whether content is complete.

With operator-approved `--asset-dir` roots, `register_asset({path:"image.png"})` reads the file once. Use its `asset_id` in binary tools, `apply_operations` with `add_picture`, or graph `icon` values. Prepared graph icons and catalog icon assets already return handles in compact mode. `list_assets` recovers them and `close_asset` releases unused registry bytes. No arbitrary paths, external relationship fetching or source overwrite is enabled. See [local asset bounds and contracts](../api.md#local-asset-handles).

The evidence-led guided path remains available through discovery:

1. `best_practice_profiles` lists the four profiles.
2. `best_practice_guide({profile_id})` returns combined English guidance, pattern capabilities, limits and the Rust-derived `input_schema`.
3. The assistant resolves the audience, purpose, evidence, logical ledger and body content. Missing quantities stay `xx` or are explicitly sourced assumptions.
4. `validate_guided_presentation({input})` checks input relationships and compiles a temporary native layout. It creates no persistent handle or file.
5. `create_guided_presentation({input})` repeats the same validation, then returns a new `deck_id`, actual `revision`, slide count and review report. No existing document is replaced.
6. Use `preview_presentation({deck_id})` for a compact 640px JPEG contact sheet. Explicitly select later pages and/or a larger size when needed; specify `format:"png"` for lossless images. `detail:"full"` restores core defaults. No file write is needed.
7. Run `preflight_presentation({deck_id,options:{page_indices:[0],min_font_size:24}})` for renderer warnings, possible collisions and readability heuristics. Review the image before treating geometry findings as defects.
8. For direct edits, use `apply_operations` with current revision/hash and 1-128 typed operations, then review selected pages. When before/after approval is needed, use `preview_slide_revision`, inspect its images, then explicitly call `apply_slide_revision` with the candidate ID and exact base revision/hash. One Undo reverses a changed batch; a preview candidate is not required for every frame edit.
9. `finalize_presentation` publishes a requested delivery bundle and a hash-bound manifest under the server's operator-approved output directory. Individual `export_pptx` / `export_static` saves remain available. Return the actual saved paths and validation scope.

The `author_presentation` MCP prompt and `aislide://authoring/workflow` resource describe this loop. Existing server configuration is sufficient; restart an already-running MCP connection after updating the source/CLI. Restarting releases process-local deck handles and candidates, so export or explicitly retain a recovery envelope first.

An already-running MCP server does not hot-load changed server code. Reconnect
after the operator has updated the server code and matching CLI actually used
by that connection. Editing this checkout or reconnecting does not
automatically update a normally installed MCP package. For source-based use,
the existing `npm run core:build` command is
`node tools/cargo.mjs build -p aislide-cli`, building the single core CLI;
the bridge's default executable is `target/debug/aislide[.exe]`. An operator
may instead select an already built binary with `AISLIDE_CORE_BINARY` at
host process startup: an absolute local path only, never a request field or
relative/UNC path. The official MCP SDK's child environment allowlist means
that override must be passed explicitly when spawning a client. This does
not update an installation, edit host configuration or stop Studio/other
user processes. Build and reconnect are operator actions, not authoring tool
calls; preserve live handles' work before reconnecting.

The SDK equivalents are `client.bestPracticeProfiles()`, `client.bestPracticeGuide(profileId)`, `client.validateGuidedPresentation(input)` and `client.createGuidedPresentation(id,input)`. Creation returns `{session,validation,profile_id,model_inference:false}` and rejects a late success after cancellation. Normal subsequent session edits remain revision-checked and undoable. The initial compiled document has no undo entry for its creation; read the returned revision instead of assuming zero.

## Positioned Parts And Typed Edits

Use `PartSpec.layout:{x,y,width,height,show_title?}` for a bounded part region.
The frame must fit within 4096 scene pixels, including its right/bottom
edges, and within the destination slide on insertion. `show_title:false`
removes the part title/subtitle band; omission defaults true. Explicit-region
fitting normalizes geometry and absolute table tracks, normally enforces a
12px text floor and can reject overflow or missing glyphs. Explicitly small
graph annotations have the exception described below. Enlarge the region or reduce
content when it cannot fit. Some chart axis labels occupy the removed 88px
band and make title removal reject. Chart-internal typography has not been
newly qualified by these checks. Omitted layout retains previous placement.

For graphs, `GraphSpec.show_title:false` allows content at y=0 instead of
reserving the 88px title band on the 1152x512 graph canvas. Node `detail`
accepts a nonblank description up to 240 characters. `detail_font_size` is
12-40, requires detail and cannot exceed heading `font_size`; its default is
`max(12,font_size*0.8)`. `text_align` is left/center/right, default left with
detail and center without. `heading_bold` defaults true. Heading and detail
render as separate editable native text with an 8px gap, subject to fitting.
Omitting the new fields preserves the earlier title/label presentation.

Node `label_fit:"shrink"` prefers a single-line heading by reducing its font
only as far as 12px; default `wrap` retains the original behavior. Hard
newlines are never removed. A node too narrow at that floor retains wrapping
and reports `GRAPH_NODE_LABEL_SHRINK_LIMIT`; ordinary additional wrapping is
reported as informational `GRAPH_NODE_LABEL_WRAPPED`. Widen the node instead
of assuming every label can become one line.

Graph edges also accept `stroke_width` (0.5-12, default 2), `label_color`
(default `@dk1`) and `label_font_size` (8-40, default 16). These sizes are in
graph pixels, before any explicit part-region fitting. Explicit font sizes
below 12 on edge labels, numbered badges and group headers opt those specific
annotations into an 8px fitting floor, including bounded `add_graph` layouts.
Omitted sizes or sizes of 12 and above retain the 12px fitting floor; node
headings/details and ordinary parts always retain it. Fitting still rejects
clipping or unavailable glyphs rather than shrinking below the applicable floor.
Use `label_placement:{position,side?,offset?,on_overlap?}` for an explicit label: `position`
is 0-1 of the complete route length from the semantic source to target;
`side` is `above` (default) or `below`, and `offset` is the 0-128px gap from
the label frame to the segment (default 8). Above is the upward-facing
segment normal, or rightward for a vertical segment. At a bend, the incoming
segment determines the normal. Omit placement for automatic avoidance.
Explicit annotations outside the graph content area reject rather than move
elsewhere. `on_overlap:"warn"` (default) preserves that placement and leaves
overlap review to diagnostics and preflight. `on_overlap:"error"` rejects collisions with
visible node content, group headers and border strokes, badges or other labels, with IDs and
repair suggestions. It never silently moves an explicitly placed label.
Automatic labels try bounded wrapping/font fitting close to their route and
inside a shared endpoint group. They can retain a bounded best-effort
candidate when collisions remain; preflight and preview are still required.
No automatic background is added to hide boundaries or connection lines.
Cylinder card text excludes the upper ellipse. Graph detail fitting reduces
single-CJK-glyph soft-wrap tails when possible, after final part placement,
without altering text or intentional newline-only lines. The 12px detail
floor and heading ceiling remain; unavoidable tails still need review.

Use `badge:{number,position?,size?,font_size?,fill?,color?}` for a native
editable numbered circle. Numbers are 1-99; position defaults to 0.5 along
the route, size to 24px (16-64), font size to 12px (8-32), fill to `@lt1` and
text to `@dk1`. Its outline uses the edge color. The circle must fit its text
and the graph content area. A badge does not require a relationship label.

`route:"manual"` requires 1-16 `waypoints:[[x,y],...]` between the endpoints,
in the 1152x512 graph coordinate space. Diagonal and backtracking segments
are supported; points must be finite, inside the content area, and cannot
collapse consecutive segments. Other route kinds cannot include waypoints.
`source_offset` / `target_offset` are signed fractions of node-side length
in -0.5..0.5: zero is the center, increasing rightward on top/bottom and
downward on left/right. Curved edges project these positions onto the shape
boundary. Nonzero offsets currently support rectangles, rounded rectangles,
ellipses and diamonds, not cylinders or clouds. Unsupported offsets reject.
Native connection IDs still reference the true source/target node; routing
never moves nodes or substitutes another endpoint to fit a line. A `move`
of both endpoints translates their waypoints; moving only one endpoint keeps
intermediate points fixed. Selecting the edge in `move` translates its
waypoints without moving nodes. A no-op `apply_graph` retains the current
native objects and history; `update_graph` remains an explicit regeneration.
Hiding a previously present title band through `PartSpec.layout` translates
manual waypoints by the same 88px as their nodes and groups.

Groups accept `padding` (0-64px, default 8) on left/right/bottom,
`header_height` (20-128px, default 40) as the reserved top band, and
`header_font_size` (8-32px, default 18), and `header_color` (RGB/theme reference,
default `@dk1`). Omitting the color retains the previous rendering. Headers must fit the band, with
`header_font_size * 1.25 <= header_height - 12`. Containment, grid layout,
Studio placement and resize constraints use these values consistently.

For a graph-only preview, request `create_graph` with
`include_diagnostics:true` to receive `{element,diagnostics}`; omission still
returns the bare element for existing callers. Studio displays these findings
without making warnings block insertion. Managed mutation replies expose
optional `graphDiagnostics` with the accepted revision/hash, up to 64 findings
and 32 KiB. A `partial` or `unavailable` status is not an all-clear. Inspect the
final graph-local bounds and entity IDs, not only a warning count. The SDK
getter is `session.graphDiagnostics`; diagnostics are ephemeral and separate
from document state, persisted metadata and Undo/Redo.

Preflight suppresses only verified managed graph badge/own-connector pairs
whose actual attributes and route still match their metadata. Hand-made or
changed badges, crossings with other edges and label collisions remain
reported. General chart non-Office notices are grouped once per page as info;
specific chart limitations remain individual warnings. This is reduced noise,
not Office visual-parity approval.
The minimum node y is `group.y + header_height`, not that value plus padding;
the minimum x is `group.x + padding`. Containment errors include node/group
IDs, these minimum coordinates, the allowed right/bottom boundaries and the
actual frame, so horizontal padding violations are also distinguishable.
All options are retained through SDK/MCP managed edits and Studio's Canvas,
Preview and JSON views. Optional styles/placements accept null to remove
them; `waypoints` is an array, not null. Native polyline/custom-site tests
verify structural round trips, not PowerPoint visual parity.

For mixed managed batches, use `add_part` / `update_part` with
`{slide_id,id,spec:PartSpec}` and `add_graph` with
`{slide_id,id,spec:GraphSpec,layout?:PartLayout|null}`. Batch `update_graph`
takes only `{slide_id,id,spec:GraphSpec}`: it preserves the existing PartLayout
and rejects a supplied `layout` field. Set final placement with `spec.layout`
for parts or top-level batch `layout` for graphs before insertion, allowing
the fitter to handle text rather than blindly scaling a finished group.
The individual legacy MCP part/graph tools remain compatible.

Matrix and contrast presets share `data.kind:"matrix"` and optional
`corner_label`, for example `"Criterion"`. It defaults to empty and accepts
at most 48 Unicode scalars, not null; XML character and layout/fit bounds
still apply. Source-provided Japanese labels also work with suitable fonts.
All three variants, `balanced`, `focus` and `labeled`, keep the label native
and editable (text for balanced/focus, top-left table cell for labeled).
Empty labels are omitted from serialization, preserving old blank output,
hashes and generated IDs.

`session.addElements` accepts complete typed roots, including rich text,
polygons, tables, charts and groups. `session.addPicture` accepts embedded
PNG/JPEG plus optional final frame/crop in one transaction. Use
`session.applyOperations` for mixed operations across existing slides and
`session.setFrames(slideId,[{id,frame}],options)` for 1-128 frame edits in one
request. Options accept `expectedRevision`, `expectedHash` and `signal`;
omitted guards use current session values. MCP `apply_operations` requires
both guards. These calls do not render a live preview or require a candidate;
validation and transport costs still apply. Review selected pages afterward.

Managed batch entries use the same core helper as individual part calls,
updating the object and `PartInstance` in one working batch without creating
a new document or fully verifying the document per item. Intermediate scene
checks and final transaction/native preservation checks remain. A changed
batch is one Undo; a no-op adds no history. Any failure, including the last
specification, leaves the original document and history unchanged. There is
no automatic fallback to unmanaged groups. `create_part` / `create_graph`
outputs passed directly to `add_elements` / `session.addElements` are an
EXPLICIT UNMANAGED choice only, not the recommended reuse workflow.

Each batch accepts 1-128 operations. The document-wide metadata total is
128 parts including graphs for every capacity profile; all other caps still
apply. For a planned 39-slide deck, first use existing `create_presentation`
and `edit_slides` (SDK `createPresentation` / `editSlides`), then insert
17 parts and 4 graphs in one 21-operation `apply_operations` call on those
existing slides. This does not create 39 slides implicitly. Use `large` or
`standard`, since `legacy` allows only 32 slides, and include any existing
metadata in the 128-entry total. Smaller intentional chunks may help review
and cancellation, but each changed chunk is a separate Undo. No measured
speedup is claimed for the new managed workflow.

`setFrames` changes geometry only: group children and view dimensions are
normalized, absolute table tracks scale proportionally, and fonts/strokes
stay unchanged. Nested coordinates are parent-local. Locked/hidden targets,
parents and affected group descendants reject. Editing managed children can
make metadata stale; use `updatePart` with the retained layout for semantic
regeneration of a current part. Stale parts must retain manual edits or be
replaced by a separately inserted part, not silently regenerated.
An earlier child edit followed by `update_part` / `update_graph` in the same
batch can also fail the render fingerprint check and roll back that batch.

`setTextStyle` overlays only supplied `RunStyle` properties without dropping
other run/paragraph formatting. `applyFormat` remains format painting.
`setConnector` instead replaces its full settings: require color, stroke width
and arrow, and resupply endpoints/routing to retain them; omission clears
start/end/routing and resets `flip_v` to false. Visual properties remain.
`setHyperlink` requires `link`; `null` clears it, while omission is an error.
The core validates links without fetching them. Imported rich-text frame
style changes and partial inherited transforms retain their native guards;
unsafe updates reject. Only a single `adj` for roundRect, chevron or triangle
is supported, not multiple named or arc adjustments.

Inserting a slide with an explicit layout fills every title placeholder with
the supplied title; an empty title clears their sample text. `rename` still
changes only the slide's displayed title, not existing text boxes.

### Visual Layout Safety

Run selected-page `preflight_presentation` and inspect `preview_presentation`
before delivery. Diagnostics do not edit the document. Findings have `info`,
`warning` or `error` severity; information is not an automatic visual approval.

- `CONTAINER_CORNER_OVERFLOW` warns when an element frame enters the curved
  corners of a likely rounded container. Inset or shorten accent bars; do not
  run a flush, full-height rectangular bar across a rounded card's corners.
- `CONTAINER_PADDING` warns when text has less than 8 slide pixels of clearance
  from a likely container's transformed rounded boundary. Enlarge or reflow
  the layout instead of shrinking all text; the curved corners also need clearance.
- `CONNECTOR_BADGE_OVERLAP` is informational only for a compact, opaque,
  numbered ellipse painted after a line that crosses near its center. A
  transparent badge, foreground line, ordinary label or grazing line retains
  `CONNECTOR_LABEL_INTERFERENCE`, as does a soft edge on the badge or its
  parent group. Reserve separate label, badge and node-text
  regions, then review the actual drawing.

Container ownership is inferred from the smallest earlier visible rounded
rectangle in the same drawing scope whose rectangular frame contains the
element frame. Rotation, flips, nested transforms and explicit corner adjustment
are considered. Frame corners are not painted glyph/pixel bounds, and arbitrary
shapes, partly external elements or intended overlays need human review.
Hidden or fully transparent, unoutlined containers do not create these warnings.
The existing 8-page, 1024-visible-object and 256-finding budgets remain.

For imported or unmanaged content, use guarded edits and before/after previews
before applying a repair. Do not silently infer semantic roles, change reading
order, move user content or globally suppress overlap findings. These checks
are heuristics, not Office visual parity or accessibility certification.

### Regeneration Feedback

`set_accessibility` uses at least 65 seconds, with larger documents receiving
the size-aware budget below. Ordinary small requests retain 20 seconds;
document-scale writes and read-only work receive finite additional time.
Progress is opt-in through an MCP progress token:
the server reports elapsed time every five seconds, never an invented completion
percentage or document content, and stops on completion or cancellation.
Use a client deadline longer than the core budget; progress does not override
a client's hard total timeout. This is not a measured speed improvement.

Create the target first, then call `set_accessibility` with its slide/element
IDs and current revision. Accessibility is not an `apply_operations` variant
or an inline managed-part field yet; each changed call has its own Undo.
After a timeout, inspect the current document/recovery state before deciding
whether to retry. Do not fall back to unmanaged groups to bypass a timeout.

Apply the theme before adding managed parts or graphs. A later `apply_theme`
can change child font/color representation and invalidate the stored render
fingerprint. This guard prevents silently overwriting manual edits. Automatic
regeneration of unchanged managed diagrams after theme changes is not implemented.

All three Venn variants accept `layout.show_title:false`. The three-set
`venn/focus` body starts at 80px rather than the usual 88px, so titleless
placement maps its full 432px body into the requested frame. Default/title-visible
generation is unchanged. Other preset clipping guards remain, including chart
labels that enter the title band. Review intended overlaps visually; do not
globally suppress preflight warnings because some overlaps are intentional.

The reported 52-slide PowerPoint comparison and accessibility results are user
feedback, not a new independently reproduced visual or accessibility qualification.

### Managed Batch MCP Acceptance

These three stages use actual registered MCP tools and synthetic content,
not a measured benchmark or factual architecture. JSON blocks are
`tools/call` params, ready to submit after substituting named placeholders.
Replace quoted `<..._INTEGER>` placeholders with **unquoted JSON integers**;
handles and hashes remain strings. Always obtain current guards from the
immediately preceding response or `get_document({deck_id})`, which returns
the document directly, including `revision`, `hash`, `deck` and `parts`.
Do not reuse guards across mutations. No shell script or new client helper
is required.

1. **Create the blank deck and destination slides.** Call
`create_presentation({title:"Synthetic managed example",capacity_profile:"large"})`,
retain its returned `<DECK_ID>`, then call `get_document({deck_id:"<DECK_ID>"})`
to obtain `<BLANK_SLIDE_ID>` from `deck.slides[0].id` and the current revision.
Submit this `edit_slides` call, then read `get_document` again for the new
revision/hash. There are now two existing destination slides.

```json
{
  "name": "edit_slides",
  "arguments": {
    "deck_id": "<DECK_ID>",
    "expected_revision": "<CURRENT_REVISION_INTEGER>",
    "operations": [
      {"op":"insert","id":"request-path","after":"<BLANK_SLIDE_ID>","title":"Synthetic request path"}
    ]
  }
}
```

2. **Insert both managed variants in one batch and inspect metadata.**
Submit the following using the guards just read. The optional standard
progress token is outside `arguments`; omit `_meta` to disable notifications.
The part and graph use explicit slide layouts, not a later frame-scale pass.

```json
{
  "name": "apply_operations",
  "_meta": {"progressToken":"synthetic-managed-example"},
  "arguments": {
    "deck_id": "<DECK_ID>",
    "expected_revision": "<CURRENT_REVISION_INTEGER>",
    "expected_hash": "<CURRENT_DOCUMENT_SHA256>",
    "operations": [
      {
        "op":"add_part", "slide_id":"<BLANK_SLIDE_ID>", "id":"comparison",
        "spec": {
          "version":1, "preset":"matrix/labeled", "title":"Synthetic comparison",
          "layout":{"x":64,"y":144,"width":1152,"height":512,"show_title":true},
          "data": {
            "kind":"matrix", "corner_label":"Criterion",
            "rows":["Input","Output"], "columns":["Option A","Option B"],
            "cells":[["Sample A","Sample B"],["Draft A","Draft B"]]
          }
        }
      },
      {
        "op":"add_graph", "slide_id":"request-path", "id":"flow",
        "layout":{"x":64,"y":144,"width":1152,"height":512,"show_title":false},
        "spec": {
          "version":1, "title":"Synthetic request path", "show_title":false,
          "nodes":[
            {"id":"caller","label":"Caller","x":80,"y":160,"width":320,"height":160,"font_size":24},
            {"id":"service","label":"Service","x":730,"y":160,"width":320,"height":160,"font_size":24}
          ],
          "edges":[{"id":"request","source":"caller","target":"service","source_port":"right","target_port":"left"}]
        }
      }
    ]
  }
}
```

Read `get_document({deck_id:"<DECK_ID>"})`: `parts` must contain `comparison`
and `flow`, with their slide IDs, retained `spec.layout`, render fingerprints
and `stale:false`. The graph entry uses preset `diagram/custom`. Call
`get_graph({deck_id:"<DECK_ID>",slide_id:"request-path",id:"flow"})` to inspect
its graph spec and stale status. There is no `get_parts` or `get_part` tool.
One `undo({deck_id:"<DECK_ID>"})` would remove both inserted objects and their
metadata, but retain the slides; if checking Undo, `redo` before exporting.
Review `preview_presentation` and `preflight_presentation` with
`options:{page_indices:[0,1]}` separately. A metadata check is not visual
approval.

3. **Save, reopen and semantically update the retained part.** Call
`export_pptx({deck_id:"<DECK_ID>",filename:"synthetic-managed-example.pptx"})`
with a new filename under the operator-approved output directory. It returns
`path`, `bytes` and `sha256`, not file bytes. The caller supplies that exported
file's base64 to `open_pptx({base64:"<EXPORTED_PPTX_BASE64>",capacity_profile:"large"})`;
no tool argument reads arbitrary paths. Retain the new `<REOPENED_DECK_ID>`.
Read `get_document` on that handle to verify both metadata entries and their
layouts survived, and read
`get_graph({deck_id:"<REOPENED_DECK_ID>",slide_id:"request-path",id:"flow"})`
to verify its spec and `stale:false`. Stop on unexpected stale metadata or
missing IDs; do not reconstruct unmanaged groups automatically. Then submit
this compatible individual `update_part` call with the reopened revision and
the retained part's slide ID. It takes a complete spec, not a patch, and
accepts a revision guard only; use batch `update_part` in `apply_operations`
when both revision and hash guards are needed.

```json
{
  "name":"update_part",
  "arguments": {
    "deck_id":"<REOPENED_DECK_ID>",
    "expected_revision":"<REOPENED_CURRENT_REVISION_INTEGER>",
    "slide_id":"<REOPENED_COMPARISON_SLIDE_ID>", "id":"comparison",
    "spec": {
      "version":1, "preset":"matrix/labeled", "title":"Synthetic comparison revised",
      "layout":{"x":64,"y":144,"width":1152,"height":512,"show_title":true},
      "data": {
        "kind":"matrix", "corner_label":"Criterion",
        "rows":["Input","Output"], "columns":["Option A","Option B"],
        "cells":[["Sample A","Sample B"],["Revised A","Draft B"]]
      }
    }
  }
}
```

Read `get_document` again: verify the changed cell, current metadata and
unchanged graph, then export to a different new filename to retain the edit.
Reopening restores supported metadata, not the earlier in-memory Undo stack;
this semantic update creates its own Undo entry. Protected/encrypted input,
stale source bindings and unsupported native changes retain their rejection
rules. These instructions do not claim that this sequence or PowerPoint
rendering has been executed as part of the documentation update.

SDK callers use the same operations array with existing
`session.applyOperations(operations,{expectedRevision,expectedHash,signal})`;
the four managed members are part of `AuthoringOperation`, not a new method.

### Time Budgets And Progress

The Node core bridge starts with 20 seconds. It counts at most 256 slides
and 8192 nested elements and uses U = max(ceil(slides/8), ceil(elements/256),
ceil(encoded bytes/1MiB)). For U > 1 the document budget is
min(180, 20 + 10*U) seconds. Requests greater than 4MiB and recovery checks
receive at least 120 seconds. PPTX open/import receives at least 60 seconds;
preview receives at least 60 + 5*(page count - 1), up to 95 seconds for eight
pages. Before/after revision previews and delivery preparation receive at
least 120 seconds. The base is the maximum of these applicable budgets. See the
[API timeout contract](../api.md#managed-timeouts-and-progress).
For N top-level managed add/update part/graph entries in `apply_operations`,
the budget is `min(300, max(base, 60 + N * 5))` seconds. Individual core
insert/update part/graph and `apply_graph` count as one. With the ordinary
base, 1 / 21 / 128 managed entries receive 65 / 165 / 300 seconds; non-managed
requests keep the base and existing AI operations keep 310 seconds. These
are limits, not latency guarantees, unlimited execution or retry loops.

Opt in through standard `_meta.progressToken`, received by the server as
`extra._meta`. Mutation tools (including normal frame/note edits), imports
and slow preview/inspection tools send an initial elapsed-time notification and
then one on a five-second timer when no notification is in flight. There is
no estimated percentage or `total`, and messages contain no private slide
or source content. Notifications stop on completion, failure or abort;
notification failure does not fail the mutation. SDK `AbortSignal` remains
supported. A client's overall timeout may need to cover the server budget
plus transport margin, with progress reset enabled where supported; some
clients ignore progress entirely. Progress does not extend the server limit.
Timeout errors include both the configured limit and measured bridge elapsed
time; process startup/termination and serialization can add to wall time.
Preview failures say that preview was interrupted without document changes.
After a client timeout inspect current guards before another mutation, not
an automatic retry or unmanaged fallback. The core remains request-based,
without a new persistent stateful core or differential protocol.

### SDK Example

Pass an already configured `AislideClient` to this complete helper. It creates
one synthetic technical page, applies two edits in one Undo, and returns the
session plus in-memory selected-page review results. It does not save files.
Reuse the inline authoring options with your real 39-page outline; the example
does not claim to contain 39 pages or factual system evidence.

```js
export async function createSyntheticTechnicalExample(client) {
  const graph = {
    version: 1, title: 'Synthetic request path', show_title: false,
    nodes: [
      { id: 'caller', label: 'Caller', detail: 'Sends a synthetic request.',
        x: 80, y: 120, width: 320, height: 180,
        font_size: 24, detail_font_size: 18,
        text_align: 'left', heading_bold: true },
      { id: 'service', label: 'Service', detail: 'Handles the synthetic request.',
        x: 730, y: 120, width: 320, height: 180,
        font_size: 24, detail_font_size: 18,
        text_align: 'left', heading_bold: true }
    ],
    edges: [{ id: 'request', source: 'caller', target: 'service',
      source_port: 'right', target_port: 'left', label: 'request' }]
  };
  const input = {
    version: 1, profile_id: 'technical-explainer', language: 'en',
    title: 'Synthetic technical example', audience: 'Engineers',
    purpose: 'Demonstrate editable authoring with synthetic content.',
    governing_message: 'The synthetic caller sends a request to a service.',
    authoring: { headline_style: 'keyword', slide_limit: 39 },
    evidence: [{ id: 'fixture', kind: 'assumption',
      reference: 'Synthetic example; not measured system behavior.',
      statement: 'Assume a caller sends a request to a service.' }],
    slides: [{
      id: 'request-path', section: 'Synthetic example', headline: 'Request path',
      sentence_form: 'explanation', pattern_id: 'native-part',
      question: 'What is the synthetic request path?',
      parent_message: 'governing', transition: 'Introduce the path',
      parallel_basis: 'Component role',
      part: { version: 1, preset: 'diagram/custom', title: graph.title,
        layout: { x: 64, y: 144, width: 1152, height: 498, show_title: false },
        data: { kind: 'diagram', graph } },
      support: [{ clause: 'Request path',
        body_paths: ['/data/graph/nodes', '/data/graph/edges'],
        evidence_ids: ['fixture'] }]
    }]
  };
  const validation = await client.validateGuidedPresentation(input);
  if (!validation.ready) throw new Error(validation.issues.join('\n'));
  const { session } = await client.createGuidedPresentation('synthetic-tech', input);
  const base = session.document;
  const slide = base.deck.slides[0];
  const headline = slide.elements.find(element =>
    element.type === 'text' && element.text === input.slides[0].headline);
  if (!headline) throw new Error('Expected the guided headline text');
  await session.applyOperations([
    { op: 'add_elements', slide_id: slide.id, elements: [
      { type: 'polygon', id: 'synthetic-marker', x: 24, y: 70,
        width: 20, height: 24, points: [[0, 0], [1, 0.5], [0, 1]],
        fill: '@accent1', stroke: '@accent1', stroke_width: 1 }
    ] },
    { op: 'set_text_style', slide_id: slide.id,
      ids: [headline.id], style: { color: '@accent1' } }
  ], { expectedRevision: base.revision, expectedHash: base.hash });
  const preview = await session.previewPresentation({ page_indices: [0] });
  const preflight = await session.preflightPresentation({
    page_indices: [0], min_font_size: 16
  });
  return { session, validation, preview, preflight };
}
```

Inspect the returned image and findings, then export through the session or
the existing delivery workflow. `session.undo()` reverses the complete final
batch, not creation. A passing guided validation or selected-page preflight
does not certify factual truth, all pages, or PowerPoint visual parity.

## Reuse Authored Slides

MCP `import_slides` takes the target `deck_id`, `expected_revision`,
`expected_hash`, an existing authored `source_deck_id`, 1-128 distinct
`source_slide_ids`, a `prefix` of 1-24 ASCII letters/digits/underscore/hyphen,
and optional `after` target slide ID (omitted/null appends). SDK equivalent:
`session.importSlides(sourceSession.document,{source_slide_ids,prefix,after},options)`.
Source and target canvas dimensions must match. Matching design is reused;
supported notes, part metadata, evidence bindings and referenced sources are
retained. Review their content before redistribution. All source embedded
fonts require embedding/editing license consent and compatible family/style
bytes; existing font and target capacity budgets apply.

Source `origin` must be absent (`None`); opening a native file does not satisfy
that rule. Arbitrary native cross-package copying is unsupported, and native
targets still enforce preservation. Stale parts/bindings, native part/slide
references, modern comment threads, conflicting source hashes and differing
source notes/handout masters reject. Target compiled-report metadata is
cleared. A successful import is one Undo and leaves the source unchanged.
This is slide reuse, not the separate master-import workflow below; MCP
accepts handles, not source paths or arbitrary source document JSON.

## Import Masters From PPTX Or POTX

Studio's **Import masters** ribbon command adds reusable masters and layouts
to the open presentation. It is separate from **Document setup > Templates**,
which creates a replacement document from POTX or THMX.

1. Choose a local PPTX or POTX in **Master source file**.
2. Select **Existing masters** to bring a master's supported layouts, theme
  and common artwork, or **Sample slides** to turn selected slides into
  reusable layouts. Select up to eight entries and supply a name.
3. Use **Preview masters** and choose each **Preview layout** for inspection.
4. Choose **Add masters**. Existing slides are unchanged. Use **Slide layout**
  to apply a new layout, or **Edit masters and layouts** to edit it. Undo
  reverses the complete import in one step; save a new PPTX or export POTX
  to retain the imported design.

Sample-slide text and graphics remain fixed layout content unless they are
already represented text placeholders. AISlide does not infer placeholder
roles or replace text automatically. Notes, comments, source bindings and
embedded fonts are not copied. Source fonts must already be available locally
or embedded separately with the required license acknowledgement. Selected
sample slides retain visible master/layout artwork and their own modeled
elements; they do not become additional ordinary presentation slides.

The source and destination must have the same scene dimensions. Imports stay
within the document's existing limits of eight masters and 32 layouts, plus
its selected archive/document capacity profile. Changing the file, mode,
selection, name or active document invalidates the preview. Cancel, stale
revision/hash, and rejected input leave the presentation and Undo unchanged.

This is an editable-subset import, not arbitrary Office template compatibility.
The core checks source shapes, background, text styles, theme and relationships
against what can be reproduced without loss. Unknown extensions, unsupported
theme effects or resource structures are rejected with a reason rather than
flattened or silently removed. Sample layouts also reject placeholder
stacking that could change against later artwork, and connectors to excluded
placeholders or other layers. Signed, labelled, encrypted, protected and
macro-bearing sources are rejected. No external relationship is fetched or
imported content executed. Original files are never overwritten; preview is
not Office visual-parity verification.

For MCP, call `inspect_master_source({kind:"potx",base64})`, select importable
IDs, then call `preview_master_import` with the current `deck_id`,
`expected_revision`, `expected_hash` and this input:

```json
{
  "kind": "potx",
  "base64": "<caller-supplied file bytes in base64>",
  "source_sha256": "<hash returned by inspection>",
  "mode": "masters",
  "ids": ["<selected source master ID>"],
  "prefix": "brand-2026",
  "name": "Reusable brand"
}
```

After reviewing the returned design and `preview_slides`, call `import_masters`
with the same base/input and `expected_candidate_hash` from the preview.
Use `mode:"slides"` with inspected slide IDs for sample layouts. The prefix is
1-32 ASCII letters, digits, underscores or hyphens and must not collide with
existing imported IDs. These operations return JSON, not image blocks or files;
the same design is rendered in Studio. SDK equivalents are
`client.inspectMasterSource`, `session.previewMasterImport`, and
`session.importMasters(input,candidateHash,options)`. No source library or
automatic local persistence is created by importing.

## Input Contract

`GuidedInput` contains `version:1`, `profile_id`, `title`, `audience`, `purpose`, `governing_message`, `language` (`ja` or `en`), optional six-digit `brand_color`, optional `authoring`, `evidence`, optional `issues`, and `slides`.

Omitting `authoring` retains the previous layout and typography. Supplying `{}` enables contextual defaults: projection for `event-talk`, reading for the other profiles. Supplying only `headline_style` and/or `slide_limit` changes validation without activating typography overrides. All sizes below are scene pixels, not PowerPoint points.

| Authoring setting | Values | Default when authoring is supplied |
| --- | --- | --- |
| `context` | `reading`, `projection` | Profile-dependent |
| `body_font_min` | 12-40 | Reading 16, projection 24 |
| `headline_font_size` | 28-64 | Reading 34, projection 40 |
| `density` | `comfortable`, `compact` | Comfortable; changes body margins and paragraph spacing |
| `spacing` | `standard`, `relaxed` | Standard; changes body gap and text line spacing |
| `font_family` | Explicit nonempty family, at most 100 characters | Existing Yu Gothic theme |
| `headline_style` | `sentence`, `keyword` | Sentence; keyword waives Japanese consulting headline length and adjacent sentence-form variation only |
| `slide_limit` | Integer 32-128 | 32; actual slide count must be 1 through the selected ceiling |

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

These are synthetic test values, not factual business data. Numeric quantities in part data need exact path/value declarations with source or assumption evidence. Geometry and structural/index fields are excluded: x/y/width/height, font/stroke size (including `detail_font_size`), version, edge from/to indices, timeline start/end indices, longitude/latitude. Their factual meaning, along with dates, amounts and counts embedded in prose, remains a review responsibility. Unknown evidence cannot justify a numeric chart value. Use `xx` in qualitative content instead of fabricating a chart bar.

Limits: guided authoring defaults to 1-32 slides; explicit `authoring.slide_limit` accepts 32-128. At most 64 evidence entries, 6 decision issues, 8 support clauses per page, 16 body paths and evidence references per clause, and 256 numeric declarations per page are allowed. Existing core part limits and the selected fixed capacity profile also apply; default large is 96 MiB wire / 32 MiB complete document. Standard and legacy remain smaller, so legacy cannot accommodate 39 slides even with the grammar opt-in. [Phase 5 capacity](../testing/phase5-recovery-capacity.md) alone does not expand the guided input grammar. Long citations or text that cannot fit are rejected rather than clipped or silently omitted. Source/reference strings are inert, not fetched.

## Decision Templates

A multi-page consulting deck starts with `C02` and ends with `C03`, with 3-6 shared decision issues. Each issue has `id`, `question`, `requested_decision`, `criterion`, `owner`, `due`, `evidence_ids` and `analysis_slide_ids`. Analysis references must resolve to actual `native-part` pages, not the summary or close.

C02 shows issue, requested decision, the cited evidence statement, and analysis page numbers. C03 shows the same issue IDs, decision and owner, timing, and criterion, plus a compact immediate schedule. Both use purpose-sized columns in editable native text/fill/rule groups; they are not rasterized or equal-width PowerPoint table cells. Their values can be edited as normal native elements, but the whole decision grid does not have part metadata for automatic regeneration.

In default sentence mode, Japanese consulting headlines target one 36-character line; 30-56 characters are accepted and longer ones reserve a two-line area. The compiler rejects more than two numeric runs, forbidden dashes/self-reference, and three adjacent identical sentence-form declarations. Keyword mode waives only the Japanese length and adjacent-form variation checks; valid sentence-form names, whole-headline support, evidence, numeric declarations and the other checks remain required. It cannot fully recognize Japanese grammatical completeness, whether a number merely counts items, or whether a judgment is warranted; these checks remain in the guide. Body resizing normalizes child geometry/fonts in the native group, with a 12-core-unit font floor corresponding to the reference's 18px at 1920x1080. Event profiles enlarge key labels where their frames allow; this is not automatic projection-legibility certification.

## What Ready Means

### Visual Review And Revisions

Preview and preflight accept 1-8 unique zero-based page indices. Compact MCP preview selects the first eight when omitted, defaults to a 640px JPEG contact sheet and a 384KiB image budget, and reports selected/unselected counts in `page_scope`. Explicit options override those defaults. Core/SDK and MCP `detail:"full"` select all pages only if there are at most eight. Maximum bounds remain 160-1600px, 2MiB combined encoded images and 4MiB response. Images are returned directly, not as disk paths; `include_images:false` hides them but still renders.

Core/SDK and full-detail preview default to `format:"png", overflow:"shrink"`. Encoded-byte overflow
alone retries at 75% and 56.25% of the requested edge, minimum 160px, at most
three distinct attempts in total. No pages are omitted. The response reports
`requested_max_dimension`, `actual_max_dimension`, `quality_reduced` and a
`PREVIEW_DOWNSCALED` warning when reduced. Use `overflow:"error"` for strict
resolution or `format:"jpeg"` (lossy quality 90) for photo-heavy pages. Other
errors never trigger silent retry. `include_images:false` still renders;
use `get_deck_summary` instead when only the current revision/hash is needed.

Preflight returns stable slide/element IDs, `scopes` (slide/master/layout), transformed bounds, severity, evidence category and repair suggestions. Renderer warnings include actual shape-padding/table clipping and font/glyph warnings. Geometry checks flag possible text-frame overlap, connector/label interference and off-slide objects; font floor and density are heuristics. Background containment and attached endpoint nodes are excluded. At most 1024 visible objects per selected page and 256 findings are allowed; exceedances fail explicitly. Orphan lines, semantic truth, full accessibility and Office parity remain separate checks.

`preview_slide_revision` requires `deck_id`, `expected_revision`, `expected_hash`, `slide_id` and 1-16 typed `edits`. Supported edits are `translate`, `align`, `set_text_frame`, `replace_text`, `update_part`, and `update_graph`. Translation/alignment target up to 32 top-level IDs; text edits may target nested IDs, with frame coordinates in the parent space. Frame changes retain font size. Locked/hidden targets, stale managed updates and unsafe native edits reject. Editing part children can mark their metadata stale, which is reported rather than silently regenerated.

The returned `candidate_id` refers to at most 128KiB of immutable edit instructions. Sixteen candidates are retained in memory, for ten minutes, until a document revision changes or its handle closes. There is no disk persistence. `apply_slide_revision({deck_id,candidate_id,expected_revision,expected_hash})` recomputes and verifies the candidate hash before one atomic commit. Wrong-document, stale, expired and already-consumed candidates fail. No-op application adds no Undo entry. Unrelated slides and the immutable source are retained. Preview does not mutate the document/history; cancelled SDK operations reject late results before committing.

`ready:true` means the typed input, references and measured native text layout are compilable. It does **not** mean semantic truth, causal entailment, complete argumentation, universal absence of visual collisions, or Office parity. Every response retains `semantic_truth_verified:false`, `office_visual_parity:false`, and explicit `review_required` items. `ready:false` includes unmet checks; creation rejects that input without returning a handle.

Actual charts retain their existing renderer's conventions. Common-axis small multiples, forecast line styles, direct end labels and quantitative area plots may require manual composition. An arbitrary part is not automatically transformed into a fully compliant consulting compound visual. Follow the capability metadata and inspect the actual PPTX.

The complete ledger and evidence text are preserved in slide notes, including assumptions and source references. Review them before redistribution. Subsequent manual edits can invalidate the original reasoning or source declarations; they are not continuously revalidated by the guided compiler. Existing native part fingerprints continue to protect manual changes from destructive regeneration.

### Remaining Limits

Typed batches reduce separate edit requests; they do not make the core a
stateful document service or add SVG caching. A general report-layout catalog,
preflight overlap aggregation, finding prioritization and preflight contrast
analysis are not implemented by this feedback work. Existing bounded findings
and separate accessibility checks remain as documented, not a combined
automatic quality verdict. These are future candidates, not delivered features.

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

PDF, images and visual preflight select 1-8 unique zero-based pages. Compact MCP delivery defaults to the first eight pages and a 640px contact sheet, with `page_scope` and the manifest disclosing coverage. Explicit options override these defaults. Core/SDK and MCP `detail:"full"` select all pages and reject more than eight when visual output/checks are requested. A full-PPTX-only delivery can use `preview:"none", preflight:false` on larger decks. The PPTX always includes every slide. The manifest and each file record their actual scope; selected visual checks do not certify other pages. PNGs retain the 160-1600px/2MiB preview limits; `min_font_size` defaults to 16. Decoded files plus manifest are limited to 32MiB (`max_output_bytes` may lower this), the core response obeys its capacity profile, and MCP response is at most 4MiB. `include_images:false` omits the response thumbnail without disabling saved previews.

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