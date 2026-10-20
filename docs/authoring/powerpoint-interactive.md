# PowerPoint interactive content add-in (Windows development)

## What runs, and what does not

AISlide can export its five typed interactive components as **Office content
add-ins on the PowerPoint slide**. Its own HTML/JavaScript runtime provides
selection, grouping, dashboard scales, comparison dimensions and ordered stage
navigation. It does not reuse Microsoft's Copilot catalog ID, execute imported
HTML, or claim a supported public Copilot authoring API. The separate opt-in
experiment below can also target the catalog reference observed in the supplied
PowerPoint file; it is not the default own-add-in path.

## Experimental Copilot catalog target

Choose **PowerPoint interactive export → Playback target → Copilot catalog
(experimental)** and explicitly acknowledge the unpublished contract. This uses
the observed `id="fa000000210"`, `version="1.0.0.0"`, `store="en-US"`,
`storeType="sdxcatalog"` reference. Its `contentBlockHtml` property is a
**JSON-encoded string containing a complete HTML document**, not raw HTML and
not the AISlide add-in's JSON settings object. Changing the ID alone is not
sufficient.

AISlide embeds its own trusted runtime, CSS and validated compiled views into
that HTML. Data is escaped for script contexts; the renderer uses safe text/rect
DOM operations and makes no network requests. Imported HTML, scripts and the
sample deck's HTML source are not copied or executed. Static PNG/native group
fallbacks remain. The output is AISlide-authored content, not content generated
or approved by Microsoft Copilot.

Playback uses the entire WebView viewport, not a vertical editor sidebar above
a miniature preview. Plain views keep compact controls below the drawing;
showcase views put them inside the header and make the content directly clickable.
The drawing is uniformly fitted against both the available width and height
and recomputed on host resize. Duplicate preview headings are screen-reader
only; the slide's own heading/source remain visible. **Readable view text**
opens a scrollable text alternative without hiding the controls.

The Copilot target embeds a copy of this runtime/CSS in each PPTX. Updating
AISlide or refreshing a server does **not** repair an already exported file:
re-export the normal editable source under a new filename. Do not overwrite
or edit the supplied presentation to apply the fix. The own-add-in path uses
the same layout, but loads its runtime assets from its HTTPS server.

This target does not use an AISlide HTTPS server, network-share catalog or own
add-in manifest. It still depends on PowerPoint's Microsoft interactive-slide
runtime, rollout, version and organizational policies. It does not bypass any
license or host restriction. Microsoft documents the end-user
[interactive slide feature](https://support.microsoft.com/en-us/powerpoint/copilot/create-interactive-slides-with-copilot-in-powerpoint),
but no supported third-party authoring contract for this catalog/property has
been established here. The host may reject the file or change this behavior.
**Referencing the catalog, or seeing its static snapshot, does not verify
Copilot-hosted HTML playback.** The exporter returns `experimental:true`,
`public_authoring_api_verified:false`, `powerpoint_playback_verified:false`.

SDK: `session.exportCopilotInteractive({acknowledged_experimental:true})`.
Core/MCP operation: `export_copilot_interactive`. Core takes `document` and the
acknowledgement; MCP takes `deck_id`, a new `filename`, and the acknowledgement,
and exclusively publishes the PPTX plus its final integrity manifest. The
acknowledgement is required, not silently defaulted; existing exports still use
the AISlide-owned add-in.

Create a separate five-page synthetic experiment without any trust changes:

```powershell
node .\tools\office-interactive-demo.mjs --experimental-copilot `
  --out .\.artifacts\copilot-interactive
```

Open `interactive-copilot-experimental.pptx` in an approved PowerPoint
environment and verify real selection/filter changes in slide show; keep
`interactive-source.pptx` for edits. A browser execution of the embedded HTML
only verifies the generated document, not the unpublished Microsoft host
contract. If activation fails, retain the static source and report the host
error; there is no automatic target switch. This is an experimental compatibility
test, not a production compatibility guarantee.

## Rich interactive sample

For a Japanese, fully synthetic product-demo deck with seven interactive pages:

```powershell
node .\tools\rich-interactive-demo.mjs --out .\.artifacts\aurora-interactive-showcase
if ($LASTEXITCODE -ne 0) { throw 'Rich sample generation failed.' }
node .\tools\verify-rich-interactive.mjs --directory .\.artifacts\aurora-interactive-showcase
if ($LASTEXITCODE -ne 0) { throw 'Rich sample interaction checks failed.' }
```

Build/select the matching CLI first as described in the mandatory checklist.
The sample contains an orientation atlas, executive drill-down, linear/log
dashboard, six-stage explorer, four-axis qualitative comparison, grouped
priority board and three assumed scenarios: 79 compiled states in total.
Numbers are labeled fictional settings, not observations, forecasts or benefits.
The scenarios are discrete authored alternatives, not a numerical simulator.

The SDK-built bundle includes `aurora-interactive.pptx` (experimental Copilot
catalog playback), `aurora-editable.pptx` (native editable static source),
seven self-contained HTML files, an offline `index.html`, an
`aurora-offline-html.zip` (extract all files before opening its index), and an
authoring recipe with notes/operations. Every rich page uses
`data.spec.appearance:"showcase"` with a native frame of at least 1200×720;
omission still uses the existing plain appearance. Rich cards, bars, stages
and rows can be clicked directly. The drawing occupies the full playback canvas;
the duplicate bottom item-button strip is hidden. Only compact group, scale,
dimension and stage-navigation/reset controls are embedded in the header band.
Text alternatives open from the small **Text** disclosure inside that band.
When the canvas is small, auxiliary controls collapse into a 24px **⋯**
disclosure in the reserved header corner. The cards/bars/stages remain directly
clickable without opening it. This prevents fixed-size controls from covering
the content when the drawing is scaled down. The disclosure also provides a
larger-target item list for narrow-screen touch/keyboard access; it is not an
always-visible strip below the canvas.
The same Rust placement geometry produces both the native drawing and bounded
item-selection hotspots. No URL, code or arbitrary HTML is accepted in hotspots.
The default state remains the static PPTX view; selecting in HTML never saves.

Compiled rich views include a validated `controls_frame` as well as hotspots,
so the control location follows the drawing's scale and letterboxing without
turning the canvas into a small preview. Omitted frames retain the plain runtime.

The verifier checks actual exported HTML, all authored states, visible text
and metric changes, clipping, five viewport sizes (including a true 440px-wide
host), constrained body/main styles,
keyboard selection, reset and accessibility. It writes default/selected
screenshots and a separate verification report. These are browser checks, not
certification of live PowerPoint/Copilot activation. Do not overwrite existing
sample outputs; select a new directory for each iteration.

## JavaScript expression experiments

The earlier rich deck switches among core-generated states. A separate
eight-page lab demonstrates what the JavaScript runtime can calculate and draw
**during interaction**, without network access or external libraries:

| Experiment | Direct interaction | Expression / boundary |
| --- | --- | --- |
| Live calculation | Change three sliders | Calculate revenue, cost and balance from a displayed simple formula; not a demand forecast |
| Relationship graph | Select a node and toggle its hypothetical failure | SVG connections and affected-node explanation; no real monitoring or system operation |
| Priority board | Drag a tile, use arrow keys or position sliders | Pointer events and keyboard alternatives; coordinates are subjective placements, not measured benefits |
| Time playback | Scrub, play and pause | `requestAnimationFrame` progress, not a recorded video or real process-duration measurement |
| Before/after reveal | Move the visual boundary | CSS clipping and direct manipulation of a fictional UI illustration; hidden content is still in the file |
| Branching exercise | Choose, continue and go back | Local decision path and reasons; no external collection or genuine examination score |
| Table exploration | Search, filter, sort and select | Client-side data transforms with an explicit empty state and fictional denominators |
| Canvas drawing | Change point count, select and animate | Deterministic synthetic points with textual/selection alternatives; no WebGL or live sensor data |

Build and select the matching CLI, then use a new output directory:

```powershell
node .\tools\javascript-lab-demo.mjs --out .\.artifacts\javascript-expression-lab
if ($LASTEXITCODE -ne 0) { throw 'JavaScript lab generation failed.' }
node .\tools\verify-javascript-lab.mjs --directory .\.artifacts\javascript-expression-lab
if ($LASTEXITCODE -ne 0) { throw 'JavaScript lab verification failed.' }
```

The output contains `javascript-lab-interactive.pptx`, an experimental Copilot
catalog playback package, and `javascript-lab-explanatory.pptx`, an editable
native explanation of each experiment. **The static fallback is not the live
HTML screen or a saved result of user input.** It describes the operation and
its limitations. The self-contained HTML files are the actual bytes embedded
in the playback PPTX. Extract all files from `javascript-lab-offline.zip` and
open `index.html` to try them without installing a server or enabling Office
trust. Local browser support is not evidence of Copilot-host activation.

This is a closed catalog of trusted built-in examples, not an arbitrary
JavaScript import feature. The SDK/MCP export's optional `javascript_labs`
selection identifies the managed slide/element and one of the eight enum
kinds; no request contains executable script or an external module URL.
Only explicitly selected components are replaced in the playback copy;
the source, normal exports and precomputed rich components remain unchanged.
No external communication, local-file reads, clipboard access, credentials,
Office writes or persistent storage are required by these experiments.

### Canvas sizing and no-scroll delivery

The lab runtime receives the actual managed component's native
`canvas:{width,height}` from the Rust exporter. A missing canvas in older
payloads uses the legacy 1280×720 baseline; current exports always supply it.
Width must be 960–4096 and height 480–4096 native pixels. It lays out the content
at these logical dimensions, then uniformly contains and centers the complete
canvas in the WebView. Resizing a WebView does not turn a slide into a long
responsive web page. Canvas backing stores are refreshed when this display
scale changes.

Do not merely hide a scrollbar, clip the lower controls, or individually shrink
all fonts to conceal overflow. The runtime budgets header, body and footer:
long descriptions, full sources and calculation formulas are retained on
**解説** pages; the data table uses **前の行 / 次の行** pagination. On short
native canvases, **操作を表示 / 図を表示** switches between the visual and control
panels while preserving input. Reset returns to the visual and initial state.
Long metadata has its full text in the explanation, not silently truncated.
The default 16:9 sample keeps visual content and controls side by side.

The verifier fails on **internal vertical scrolling**, not just document-level
horizontal overflow. It checks native 1280×720 (16:9), 1200×900 (4:3),
960×540 and 960×480 layouts; all relevant visual/control/explanation states;
actual selected details; table paging and access to every record. It also
checks resizing across WebView sizes and letterbox bounds. Using
`scrollIntoView()` to reach a missing control is not evidence that a slide fits.
Generated files must be re-exported after rebuilding the runtime; existing
PPTX files contain the older HTML and do not update automatically.

These tests establish fit and operation for the built-in examples and tested
dimensions, not universal support for arbitrary HTML. If authored content
cannot fit, split it into more pages or report a capacity failure; do not
deliver clipped or scrolling content as a successful slide.

Animations start only on request and can be paused; hidden-page/disposal
cleanup and reduced-motion preferences matter. Dragging must have keyboard
or slider alternatives. Canvas must have textual alternatives; a visible
pixel alone is not accessible data. Each experiment carries its synthetic-data
and platform caveats rather than presenting a capability test as factual proof.

Browser verification records actual calculations, pointer/keyboard events,
pause/reset behavior, filtering and empty states, direct selections, resize,
accessibility checks and network requests. Tests do not certify WCAG, Office
visual parity, production deployment, arbitrary PowerPoint APIs or support
for remote services, imported scripts, video/camera or file-system access.
Verify the intended PowerPoint host separately before distributing as a
host-tested interactive presentation.
The Windows verification workflow generates and exercises this closed lab
catalog after building its CLI, without installing/trusting an Office add-in.

## AISlide-owned add-in requirements

A PPTX is not a standalone HTML browser. Active playback requires all three:

1. A supported PowerPoint host with Office Add-ins enabled by policy.
2. The **AISlide Interactive** manifest installed from an explicitly trusted
   Windows network-share catalog.
3. The manifest's HTTPS runtime reachable with a certificate trusted by that
   machine, plus the Microsoft-hosted Office.js library.

The current `FileSystem` catalog export is for **Windows development/testing
only**, not Marketplace, Microsoft 365 production deployment, Mac or web
deployment. The exporter never creates a network share, installs an add-in,
changes a trusted catalog, trusts a certificate, or uploads a presentation.

## Build and run

Use the development branch's CLI and set `AISLIDE_CORE_BINARY` to its absolute
path. A normal build is `node tools/cargo.mjs build -p aislide-cli`; follow the
repository's build setup for its Rust toolchain.

Run the application runtime with an **already approved** certificate/key:

```powershell
$env:AISLIDE_CORE_BINARY = 'C:\path\to\aislide.exe'
node .\tools\office-content-server.mjs `
  --https-cert C:\approved\localhost.crt `
  --https-key C:\approved\localhost.key
```

This loopback-only development server uses port 4198 and serves only its
allowlisted application assets. Its URL is
`https://localhost:4198/office.html`. The certificate must cover `localhost`
and be trusted by the Office/WebView machine. The server does not generate or
install certificates. Closing it makes active playback unavailable.

For a recipient on another machine, `localhost` refers to **their** computer.
Do not distribute a localhost manifest as though it points to the author's
server. A different trusted HTTPS deployment and a suitable production add-in
deployment method require separate configuration. Never host confidential
components on an untrusted runtime origin: runtime code can read its settings.

## Export and explicitly install

In Studio, insert a managed **Interactive part**, save its normal editable
PPTX, then select **PowerPoint interactive export** on the editing ribbon.
Supply the HTTPS runtime URL and the real trusted catalog UNC path, such as
`\\your-server\AISlideAddins`. Download both the playback PPTX and add-in XML.
The dialog does not install them.

For an automated synthetic demonstration:

```powershell
node .\tools\office-interactive-demo.mjs `
  --runtime-url https://localhost:4198/office.html `
  --catalog '\\your-server\AISlideAddins'
```

The default output is `.artifacts\office-interactive`. It contains:

- `interactive-source.pptx`: editable static source with typed metadata.
- `interactive-powerpoint.pptx`: five content add-ins, with synthetic data.
- `aislide.addin.xml`: the external Office manifest.
- Five compiled-view JSON fixtures for browser testing.
- `interactive-powerpoint.manifest.json`: local output hashes and limitations.

Existing files are never overwritten. Use `--out` with a new directory for
another run. Multi-file publication is not crash-atomic; the integrity manifest
is published last. Neither generated manifest grants trust.

Place the add-in XML in the actual shared catalog folder. In PowerPoint:

1. **File → Options → Trust Center → Trust Center Settings → Trusted Add-in
   Catalogs**: add that UNC catalog and select **Show in Menu**.
2. Restart Office as required by its catalog configuration.
3. **Home → Add-ins → Advanced → Shared Folder**: select **AISlide Interactive**
   and explicitly install it. An empty inserted add-in reports that it has no
   component data; use the generated playback PPTX for content.
4. Open the generated playback PPTX and test the controls in the intended
   editing/slideshow mode. Host version, policy and view mode can affect
   activation; a saved snapshot is not proof of active playback.

These are explicit user/admin steps. AISlide does not bypass Office trust or
silently register the catalog. Refer to Microsoft's current
[network-share sideloading instructions](https://learn.microsoft.com/office/dev/add-ins/testing/create-a-network-shared-folder-catalog-for-task-pane-and-content-add-ins).

## Shared-core contract and persistence

- `compile_interactive` / `client.compileInteractive({id,spec,theme?})` compiles
  all permitted state combinations through the same Rust layout and data
  validation used by Studio. Maximum 2 MiB per component and 16 components per
  playback export; overlarge or overflowing views reject.
- `powerpoint_addin_manifest` /
  `client.powerpointAddinManifest(runtimeUrl)` creates the own-ID `ContentApp`
  manifest with `Restricted` permission, Presentation host and snapshots.
- `export_powerpoint_interactive` /
  `session.exportPowerpointInteractive({runtime_url,catalog:{kind:"file_system",
  location:"\\\\server\\share"}})` produces a **new** PPTX and manifest.
  The MCP tool takes `deck_id`, `filename` and `options`; it requires an
  operator-approved output directory and exclusively publishes the PPTX,
  `.addin.xml` and final `.manifest.json`.
- Export returns `powerpoint_playback_verified:false`,
  `requires_addin_installation:true`, `requires_https_runtime:true`, snapshot
  `warnings`, and explicit `limitations`. These are not success-shaped claims
  of Office validation. Font fallback warnings must be reviewed.

The PPTX contains a standard web-extension reference to AISlide's own manifest,
a JSON **data** property, and an embedded PNG snapshot. `mc:AlternateContent`
also retains the native editable group fallback. PowerPoint can recognize the
content add-in even when it cannot activate its runtime. The static PNG and
typed settings are stored in the PPTX and must be treated as disclosed content.

The HTML runtime uses only compiled rectangle/text primitives and safe DOM
text operations. It does not evaluate source strings, load item URLs, read
other slide content, or write Office settings. Each load resets to the saved
`initial_state`; selection/filter/scale changes are transient.

Keep editing the normal static source. Playback exports reopen in AISlide as
an **inert native fallback, read-only**, preserving the active content without
executing it. Edits are rejected rather than silently leaving runtime data and
fallbacks inconsistent. Re-export from the editable source to update playback.
The ordinary **Save PPTX**, PDF and image paths remain static.

## Validation boundaries

Browser playback tests use real core-compiled payloads. The Office bridge tests
use an Office.js host stub and are **not actual PowerPoint click tests**.
PowerPoint COM shape recognition verifies that the desktop parser sees
`msoContentApp` (27), not that the installed/trusted runtime ran.
The tests do not certify Office visual parity or factual accuracy.

## Mandatory creation and delivery checklist

Apply this checklist to **every interactive delivery**, not only a reported
bug. The earlier miniature-preview defect was in the shared runtime, not just
one slide: source code and regression tests were corrected as well as the
sample PPTX. A successful export or static preflight alone did not catch it.

1. **Choose and record the target.** Keep the normal editable source. Distinguish
   AISlide-owned add-in, experimental Copilot catalog, and ordinary static PPTX.
   Do not silently switch targets or describe experimental output as a public
   Microsoft authoring API.
2. **Rebuild before export.** After changing shared HTML/CSS/JavaScript, build
   the CLI from the same checkout and point `AISLIDE_CORE_BINARY` at that
   executable. Copilot HTML embeds the runtime at Rust compile time: testing
   updated source CSS with a stale executable is not sufficient. For the own
   add-in, restart its runtime server to load changed assets.
3. **Re-export to a new directory/filename.** Existing Copilot PPTX files retain
   their old HTML. Reopening them, refreshing Studio, or restarting a server
   does not apply a new embedded runtime. Keep earlier files unchanged and give
   the recipient the exact newly verified file, not the previous download.
4. **Test the delivered runtime and data, by target.** For Copilot, test the
   actual exported HTML: extract only AISlide-generated `contentBlockHtml`
   from the new PPTX for local browser checks. For the own add-in, test its
   served HTTPS runtime using the new PPTX's exported `aislideInteractive`
   settings; it does not embed `contentBlockHtml`. Record the served asset
   revision/hash as well as the PPTX hash. Do not execute arbitrary imported
   HTML or substitute a source-code demo/PNG snapshot for delivery bytes.
5. **Exercise all pages and controls.** Check initial state and at least one
   changed selection, group, dashboard scale, comparison dimension, and
   timeline previous/next where applicable. Confirm the visible detail/chart
   actually changes, not only the selected button or status text. Check Reset,
   reopen/default state and keyboard focus order.
6. **Measure layout after interaction and resize.** Budget content at the
   actual native canvas dimensions, not a desktop browser's assumed height.
   Reject internal vertical scrolling as well as document overflow; never use
   `scrollIntoView()` as proof that an off-canvas control fits. Use the matrix below for
   each applicable template, including constrained host styles. Retain a
   full-frame screenshot before/after selection and after resize. Do not crop
   away controls or use only a contact sheet to judge text readability.
7. **Separate host verification from browser verification.** In an approved
   PowerPoint environment, check real clicks in the intended slide-show mode.
   If unavailable, report **PowerPoint interaction unverified** explicitly;
   content-app recognition, Office.js stubs and static snapshots are weaker
   evidence and must not be reported as successful host interaction. Do not
   change Office/OS trust or install add-ins without authorization.
8. **Record delivery evidence.** Include the new path, SHA-256, page/template
   coverage, tested sizes, screenshots, test results and any unverified host
   conditions. Stop delivery as a verified fix if layout/interaction assertions
   fail; fix the shared runtime and repeat from step 2 rather than manually
   patching a single exported slide.

### Required viewport and host matrix

| Case | Size / constraint | Required result |
| --- | --- | --- |
| Full slide | 1324×746 | Drawing width greater than 1000 CSS pixels for the synthetic landscape fixture; no miniature sidebar preview |
| Embedded landscape | 864×408 | Full drawing bounds and usable controls stay inside the viewport |
| Short landscape | 640×300 | Height as well as width limits the drawing; no bottom clipping |
| Narrow portrait | 390×900 | Controls wrap/scroll within their area; no document-level overflow |
| Constrained host | Centered body/main with 440px max-width, 540px height and 140px top margin | Playback root still fills the WebView viewport, rather than inheriting the narrow column |

For every size, require positive drawing dimensions, no drawing edge outside
the viewport, visible selected detail and reachable controls. The landscape
fixture's >1000px threshold guards this particular regression, not a universal
minimum for portrait/custom canvases. Use uniform width-and-height containment;
do not distort the slide or shrink text merely to satisfy a screenshot check.
Plain views place compact controls below the canvas. Showcase views use
in-canvas item targets and header controls, never a duplicate bottom button
strip. At small scales, auxiliary controls collapse into the header disclosure
instead of overlapping cards or bars. DOM focus order must match the visual
order. **Readable view text** must remain accessible.

### Reproducible development check

From the checkout root, build the CLI and explicitly select it. The commands
below assume the default Cargo output directory; if `CARGO_TARGET_DIR` is
customized, select that build's actual executable instead. Use fresh output
directories; the generators never overwrite existing files.

```powershell
node .\tools\cargo.mjs build -p aislide-cli --locked
if ($LASTEXITCODE -ne 0) { throw 'CLI build failed; do not export with an older executable.' }
$env:AISLIDE_CORE_BINARY = (Resolve-Path .\target\debug\aislide.exe).Path
node .\tools\office-interactive-demo.mjs `
  --runtime-url https://localhost:4198/office.html `
  --catalog '\\localhost\AISlideAddins' `
  --out .\.artifacts\office-interactive
if ($LASTEXITCODE -ne 0) { throw 'Own-add-in fixture generation failed.' }
node .\tools\office-interactive-demo.mjs --experimental-copilot `
  --out .\.artifacts\copilot-interactive-review
if ($LASTEXITCODE -ne 0) { throw 'Copilot fixture generation failed.' }
$env:AISLIDE_COPILOT_TEST_DIR = (Resolve-Path .\.artifacts\copilot-interactive-review).Path
npx --no-install playwright test --config playwright.office.config.ts
if ($LASTEXITCODE -ne 0) { throw 'Playback checks failed; do not deliver as a verified fix.' }
```

Stop after any failed command. The catalog/runtime values above are synthetic
export coordinates only; these commands neither install an add-in, create a
share, start an HTTPS service nor grant trust. The own-add-in JSON fixtures
must exist in `.artifacts\office-interactive` for the test runner. If that
directory already contains an earlier run, use its verified matching fixtures
or deliberately archive it before generating; do not delete or overwrite it
blindly. `AISLIDE_COPILOT_TEST_DIR` must select the **newly generated** bundle.

Windows verification CI generates both fixture sets from its just-built CLI
and runs the same browser suite. Documentation tests check that this skill,
checklist and CI entry stay linked. These are CI/procedure checks, **not a
runtime export-blocking gate**: Studio/MCP export alone does not execute
Playwright or certify Office playback. Synthetic regression coverage also
does not replace reviewing the recipient's actual authored pages.

A browser-only development check is available without certificate changes:

```powershell
node .\tools\office-content-server.mjs --http-preview `
  --preview .\.artifacts\office-interactive\dashboard.json
```

Open `http://localhost:4198/preview.html`. HTTP mode is explicitly for browser
fixtures; it cannot be used as the HTTPS manifest source. Run the runtime
suite with `npx playwright test --config playwright.office.config.ts` after
generating both the own-add-in fixtures and experimental Copilot bundle in
their example output directories. Stop the preview server before starting
that runner.

To verify a newly re-exported Copilot bundle without replacing the earlier
artifact, set `AISLIDE_COPILOT_TEST_DIR` to its absolute directory before
running the same tests. The exported HTML is tested with centered 440px host
styling, full-size, short and narrow viewports, state changes, keyboard order,
and visible drawing/controls bounds. These tests guard the miniature-preview
regression; they still do not certify the unpublished Copilot host contract.

## Public specifications

- [MS-OWEXML content extension example](https://learn.microsoft.com/openspecs/office_standards/ms-owexml/6ddbf09f-37b4-4874-be2b-b6929d04810e)
- [MS-OWEXML web-extension structure and snapshot](https://learn.microsoft.com/openspecs/office_standards/ms-owexml/d59d5543-252a-47dd-a566-1503dbf6a233)
- [Office state and settings persistence](https://learn.microsoft.com/office/dev/add-ins/develop/persisting-add-in-state-and-settings)
- [AllowSnapshot behavior and disclosure](https://learn.microsoft.com/javascript/api/manifest/allowsnapshot)
- [Office add-in manifests and HTTPS](https://learn.microsoft.com/office/dev/add-ins/develop/add-in-manifests)

AISlide's OOXML mapping is independently implemented from these public contracts.
