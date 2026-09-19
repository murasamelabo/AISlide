# Native ChartEx: G14 Qualification

AISlide implements funnel, waterfall, histogram, box-and-whisker, treemap and
sunburst as real chartEx parts, not pictures or aliases for `c:barChart`.
There are 24 accepted chart kinds: 18 classic and these six chartEx families.
This is bounded native chart support, not full G14/PowerPoint parity.
The latest histogram count, width and single-negative-sample fixtures passed
actual PowerPoint opening, native ChartType 118, workbook edit/restore and
nonblank captures using `val`-attribute bin encoding. That Office-compatible
encoding has **two known Open XML SDK 3.5.1 schema errors per histogram chartEx
resource**. It is an explicit compatibility exception, not a schema-clean result.
The 24-kind synthetic editing deck also passed Office opening, all 24 workbook
edit/restore checks and captures. The final Sunburst boundary change then passed
fresh and native-edited product-output checks, including zero schema errors,
Office workbook editing and visible sector boundaries. See the
[verification ledger](../planning/editing-expansion.md) for hashes and separate
run scopes. Neither run establishes general pixel/typography parity.

The existing `histogram(samples, bin_edges)` helper remains a separate derived
standard-column workflow. Native `kind: "histogram"` preserves raw samples;
computed bin counts are never substituted into its embedded workbook.

## Statistical And Hierarchical Data

- Histogram: `categories: []`, one series with `values: []`, and required
  `options.histogram` containing `samples`, `binning: {rule: "count", count}`
  or `{rule: "width", width}`, and `interval_closed: "left" | "right"`.
  Optional numeric `underflow`/`overflow` must increase when both are present.
  Samples are 1..4096 finite values in +/-1e15; counts are 1..128; positive
  widths must produce at most 128 regular bins. Sample order, repeats and
  fractional values are retained exactly. See the Office/schema exception below.
- Box-and-whisker: `categories` are 1..32 unique, nonempty group labels;
  one series has `values: []`. Required `options.box_whisker.samples` has one
  raw array per group, at least four values each and at most 4096 total.
  `quartile_method` is inclusive/exclusive. `mean_line`, `mean_marker`,
  `nonoutliers` and `outliers` are explicit booleans. The workbook stores
  repeated group-label/value rows, not fabricated quartiles or counts.
- Treemap/sunburst: 1..32 positive leaf values in one series; required
  `options.hierarchy.paths` contains unique paths of equal depth 2..4.
  Every label is nonempty and at most 80 characters. Each category equals
  the final label of its path. Equal leaf labels in different paths are
  allowed. Treemap alone accepts `parent_labels: none | banner | overlapping`.
  Workbook path columns run root-to-leaf, then the numeric leaf-value column.
- All four reject incompatible typed options, custom axes, native data-label
  options, per-series chart kinds, trendlines, errors and bubble sizes.
  Statistical and hierarchical numbers use the shared finite +/-1e15 bound.
- Studio provides raw sample inputs, bin rule/count/width/thresholds, interval
  closure, group labels, quartile/visibility toggles, hierarchy depth/path/value
  editing and treemap parent-label control. Invalid applies retain the draft
  and document revision; a corrected apply clears the error. Compatible
  treemap/sunburst switching is offered; unrelated family conversions are disabled.
  Dedicated previews are approximate. Legend placement and treemap parent-label
  layout are preserved natively but are not reproduced by these new previews.
  Native hierarchy fixtures use one series color. Sunburst adds theme-light
  boundaries so parent/child sectors remain distinguishable without altering values.
- Static PNG/JPEG/PDF export supports represented basic charts in all six families;
  PNG/PDF tests pass for all 24 kinds. Studio and static layouts
  remain approximations, not Office renderings. Unsupported additional axes,
  trendlines, error bars or other unrenderable options fail explicitly.
  The default 24-chart editing demo edits the first raw sample for statistical
  families and the first leaf value for hierarchies; it does not invent values
  in the empty statistical `series.values` array.

## Contract

- Funnel and waterfall use 1 series and 1..32 categories with matching values.
  Existing standard charts retain the 6-series/32-category caps.
- Numbers must be finite and within +/-1e15; existing label/color limits apply.
- `funnel`: every value must be strictly positive. Values and category order
  are never sorted, normalized, binned, or replaced during export.
- `waterfall`: signed values and zero are supported. The required
  `options.waterfall_totals` is a sorted, unique list of zero-based category
  indices. A marked value is an explicit total; other values are changes.
  `[]` explicitly means all changes. Missing/null markers are rejected.
  Totals are authored values, not automatically calculated or validated as
  accounting identities. No implicit first/last total is fabricated.
- Optional `options.legend`: bottom (default), top, left, right, top_right,
  hidden. `top_right` maps to right/max alignment.
- Only default native axes are supported. Custom axes, data labels,
  trendlines, error bars, bubble sizes, per-series kinds, and secondary axes
  are rejected for these families. Waterfall has distinct category/value
  axis identities with implicit series association; funnel has no axes.
- Statistical and hierarchy options follow the dedicated contracts above.

Example chart data, usable in a scene chart element (add its ID and bounds):

```json
{
  "kind": "waterfall",
  "categories": ["Start", "Change", "End"],
  "series": [{"name": "Synthetic example", "values": [120, -80, 40], "color": "087F73"}],
  "options": {"waterfall_totals": [0, 2]}
}
```

## APIs And Preservation

Core `object_catalog`/`authoring_capabilities` advertise 24 accepted kinds,
with per-family limits and `office_validated: false`. `create_object` accepts
`preset: "funnel"` or `"waterfall"` and returns clearly named synthetic data.
The waterfall factory supplies explicit first/last total indices.
Scene documents, SDK `createObject`/`addObject`, MCP
`add_object`/`apply_transaction`, and Studio share core validation.
The current report chart contract includes `options`; focused tests verify
that typed statistical/hierarchy options survive compilation. SDK `ChartKind`
and `ChartOptions` carry the same raw-data contracts. Waterfall still rejects
missing/null total markers.

Native reading uses chartEx caches, never an original scene snapshot.
One data block with a unique numeric ID contains a category string dimension
(except raw histogram) and a numeric dimension (`size` for hierarchies,
`val` for the others). `cx:dataId` resolves that block. Dimensions
have schema-defined roles, not invented ID attributes. Formula references
point to internal Sheet1 cells; `cx:pt` caches and workbook cells contain the
same raw values. Formulas are stored only, never executed or fetched.

No-op native saves preserve the entire original package byte-for-byte.
Editing represented data/options creates new chart/workbook resources and
retains old resources. Chart XML and both native style resources must match
the represented subset and the embedded workbook must still be synchronized.
Unexpected chart relationships and style-part relationships block replacement.
Unknown extensions/formatting
and independently changed workbooks are retained and block replacement.
Malformed caches, dimensions, axis identities, unsupported layouts or semantic
options remain opaque native content. Chart/chartEx conversion after native
import is rejected; create a new chart object instead. Funnel/waterfall
conversion within chartEx requires valid explicit options.

The frame directly contains `a:graphic/a:graphicData/cx:chart`. The previous
inner `mc:AlternateContent` table fallback has been removed; older Office is
not supported by this representation. No raster preview, fabricated chart,
external relationship, `fallbackImg`, or source snapshot substitutes for the
native chart. The documented 2014 chartEx namespace targets Office 2016+;
no invented `c16r3` tags are required. Every extended chart has internal
`chartStyle` and `chartColorStyle` relationships to independently generated
2012 chart-style parts, including their content-type declarations.

Studio has distinct Recharts funnel and floating-range waterfall previews,
with an explicit Office-qualification warning. The native total controls
remap indices when a category is removed. Static export now supports their
represented basic forms and the other four chartEx families. Neither Studio
nor static preview proves Office layout or editability. See the
[static output contract](../api.md#static-export-and-recovery) for independent
page/byte limits and effect-rasterization warnings.

## Current Histogram Office/Schema Exception (2026-09-18)

The production writer emits `cx:binCount` or `cx:binSize` with a `val` attribute.
Changing only the previous text value to this attribute restored PowerPoint
opening; the schema-valid text form had failed with `0x80070570`.
The reader accepts either one unambiguous text value or one `val` attribute,
but rejects mixed nonempty text/attribute values, nested elements and extra
metadata rather than silently discarding them. Raw samples, cache/workbook
synchronization and native-preservation guards remain in force.

Public MS-ODRAWXML CT_Binning (2.24.3.7) and Open XML SDK 3.5.1 expect simple
text values for these children. The production attribute form therefore
produces two known schema errors for each histogram chartEx resource:
an undeclared `val` attribute and an invalid empty simple-text value.
Native edits can retain old resources, so count errors per resource, not per
visible chart or slide. Do not suppress these findings, call all schema checks
passed, or extend this narrow exception to unrelated validation failures.

Histogram-specific evidence: `%TEMP%/AISlide-histogram-final-leLuRt`.
The explicitly run ignored regression passed count, width and single-negative
sample cases: actual Office Open, ChartType 118, embedded-workbook edit and
restore, and nonblank captures. The schema exception remains even when those
application checks pass.

Old text-form sources may still be read and saved byte-identically with no
edits. No-op save and Undo do not repair them or automatically rewrite binning.
Unknown XML/resources still block unsupported replacement. Earlier four-family
schema successes below describe the historical text form, not current histogram
output.

Focused static gates: 31 `export_static` tests and 4 `static_api`
tests passed, including all-24 PNG/PDF rendering, the histogram last-interval
overflow label, and pie/doughnut legends. Final integrated Node, browser,
native and follow-up results are recorded in the [editing verification](../planning/editing-expansion.md).

Sunburst native replacement accepts either the exact current generated XML or
the exact previous no-boundary encoding, followed by the same complete workbook
check. A custom line, unknown property or independently edited workbook remains
protected. No-op export and Undo preserve the old package bytes; no unrequested
upgrade is made merely by opening a presentation.

## Historical Four-Family Qualification (Text Bins, 2026-09-18)

Evidence root: `%TEMP%/AISlide-G14-four-CmKagx`.
This run predates the production `val` encoding. Its histogram failure and
schema successes are retained as historical evidence, not current status.
The eight files are `native-<kind>.pptx` and `edited-<kind>.pptx`, each with
one slide, one chart and one separate title. Fixtures use independently
specified fractional/negative/repeated raw samples and depth-two paths;
edits change raw samples, bin/visibility options and depth-three paths/values.

| Kind | Core native create/read/edit/Undo | PowerPoint native / edited | Numeric cell |
| --- | --- | --- | --- |
| histogram | Passed | Failed / failed at open, 0x80070570 | B2 |
| box_whisker | Passed | Passed / passed | B2 |
| treemap | Passed | Passed / passed | C2 / D2 |
| sunburst | Passed | Passed / passed | C2 / D2 |

The native box fixture hit one RPC error on its initial attempt and passed
on one unchanged retry. All actual Office checks used the existing disposable
copy helper, with no user presentation touched. `ChartValueCell` defaults to
B2 and accepts only a bounded single A1-style address in A:E; it rejects
ranges/expressions and requires an actual numeric value before editing.
The existing optional first-numeric-cell mode remains available.

For these historical text-bin files, Open XML SDK 3.5.1 Office2016 validation passed all eight whole packages,
12 chartEx roots, 24 chart-style/color-style roots and all 12 embedded XLSX
packages, including old resources retained by native edits. Schema validity
did not imply histogram Office compatibility. The six successful Office
fixtures each have one native chart, one workbook edit/restore, notes and
1280x720 captures; source and disposable-copy hashes are compared separately.

SHA-256, native then edited:

- histogram: `e7e8b566e44e602397545fa8e58e09a988924e0f1516223bffc2e611b29620c6`
  / `b4569d73a502fa2037f89037feb5e3f09ea220e482e7a9266244b5a402c6ccdc`.
- box_whisker: `2785c200ab245e9001f6d74463831988b166dfc59f4bc6ca8f9c313218def76f`
  / `c4fcc7fc4511e605de2225eb64dd8d412fae120a8e39af3c2b064f99b7a1b001`.
- treemap: `3d0a0333ecec11b899789e204b2103af892f6ea7839220b25e362c4d84390103`
  / `5b54b5ad295bdfe6b4e98d7aeff7b53dd3480b925444847f11c824e551c42fd6`.
- sunburst: `18c5d4857bf1641fe63c14628e4fbcd56391778e5656bf1cc0db9ed09c2a5045`
  / `8b4a9837d788217864cac416199dcc3182a4c7206be8522cf7d5dc798c107065`.

Focused tests now compare the full typed native element against independent
input, raw workbook cells against original arrays, and caches against cells.
They cover edited options/paths, exact no-op/Undo, unknown chart/style XML,
independent workbook edits, sparse/duplicate/oversized/nonnumeric caches,
unsupported formulas, invalid samples/bins/thresholds, true duplicate paths,
depth limits and accepted depth 2/3/4. Four real-core browser scenarios prove
invalid-input recovery, exact data after save/reopen and Undo, family-specific
primitives, nonblank pixels and bounded layout at widths 1440 and 390.

Histogram diagnostic copies changing only category cache, numeric axis,
automatic thresholds, numeric role, explicit axis references or legend each
still failed opening. None of those guesses changed the production codec.
The later isolated text-to-`val` change and its explicit schema exception are
recorded above; these earlier unsuccessful probes are not the current blocker.

## Verification Scope

Focused tests:

```text
node tools/cargo.mjs test -p aislide-core --test charts --test chart_format --locked --offline
node tools/cargo.mjs build -p aislide-cli --locked --offline
node --test --test-name-pattern="MCP expanded authoring" tools/mcp-poc.test.mjs
node node_modules/@playwright/test/cli.js test tests/e2e/document-render.spec.ts tests/e2e/object-tools-panels.spec.ts --grep "chartEx|chart transaction"
```

Use a new empty temporary directory as `AISLIDE_CHARTEX_SCHEMA_DIR`, then run
the ignored `extended_offline_schema_fixture` test with `--ignored --exact`.
It exclusively creates original and edited funnel/waterfall PPTX fixtures;
it never overwrites any file. Run `tools/validate-openxml.ps1 -Path <fixture>`
for each. Also validate with `OpenXmlValidator(FileFormatVersions.Office2016)`
against the whole package, every `ExtendedChartPart`, and its embedded
`SpreadsheetDocument`, so a compatibility fallback cannot hide invalid
modern content. Compare file hashes before and after read-only validation.

Historical funnel/waterfall verification: native and UI RED/GREEN, deterministic XML/ZIP, cache-cell
agreement, unknown-content preservation, invalid input rejection, guarded
native edit/reopen/Undo, and Open XML SDK 3.5.1 schema validation of four
packages (six chartEx parts and workbooks including retained old resources).
The subsequent actual PowerPoint qualification is recorded below. Structural
validation alone did not detect the original PowerPoint-open failure.

## Historical Funnel/Waterfall PowerPoint Open Fix (2026-09-18)

The supplied 9,777-byte waterfall failed `Presentations.Open` with
`0x80070570` despite Office 2016 schema validation. Its unchanged SHA-256 is
`20946ef35ce2f5c540d882685d15f24448d9ed9417bea16787fca8d9117484bf`.

Two independent application constraints were isolated with disposable copies:

- Removing either the chart-style or color-style relationship from a new
  PowerPoint-created calibration waterfall reproduced the same open failure.
  AISlide previously emitted neither resource. The writer now supplies both
  using original, schema-based theme/font defaults, not copied Office styles.
- Waterfall series containing `cx:axisId` failed even inside the calibrated
  package. Removing only those children restored opening. The writer retains
  distinct category/value axes, but uses their implicit series association.
  The reader validates both implicit roles and any imported explicit IDs.

Direct chart references and owner-level compatibility wrappers both failed
before these resource/payload fixes. A direct reference now opens successfully;
the wrapper-placement hypothesis alone was not the root cause. The current
run qualified the representation only for these two families; it did not
qualify statistical or hierarchical layouts added afterward.

The original and native-edited funnel and waterfall fixtures each passed the
existing disposable-copy PowerPoint verifier: one actual chart, zero pictures
or tables, an embedded-workbook B2 edit/restore, notes, and a 1280 x 720 slide
capture. The four captures were inspected: funnel remains centered, waterfall
retains the floating decrease and explicit final total, and edited values
change the geometry. All four source hashes stayed unchanged. The synthetic
edited waterfall intentionally does not assert an accounting identity.

Verified fixture SHA-256 values (original / native-edited):

- Funnel: `5a83734220ab173452d5ed9768d798f5e9bad8d0b243848637c02c7f899b2658`
  / `f51524b4d43ff66c01c862f0767d7d0d2f453429dfc277561e6055d3f51923b9`.
- Waterfall: `5aa8f0f3e68c285eb94fd998d654165474898638e3fdbd1fa337638e6191d972`
  / `36647669209bb813983ffb8040cc6030522e674bbeb05d2c911352baa34d63c2`.

No-op saves and Undo preserve bytes, including older unreadable output; they
are not repair operations. Old chartEx packages without the required styles,
or with the previous explicit-axis encoding, can remain readable by AISlide
but block data/style replacement. Recreate authorized source data into a new
document to obtain the corrected representation. Unknown XML, modified style
resources and independent workbook edits are never silently discarded.

The calibration file was created only to observe public-format behavior; it
was not counted as AISlide functionality. All Office opens operated on newly
owned temporary presentations. Macro execution was disabled during automation
and the prior setting restored; no user presentation or protection policy was
changed. The classic-18 center-label fix was not modified.

Historical gates for this two-family fix: `node tools/cargo.mjs test --workspace --locked --offline` passed
415 tests with zero failures and three opt-in skips; `npm run build` and the
CLI rebuild passed. The chart-format suite passed 26 tests (two fixture-writing
tests ignored by default), including eight focused chartEx tests. Production
build chunk-size warnings remain unrelated to this fix.

The unmodified default `tools/editing-demo.mjs` completed all 28 SDK/core
supported subsets in a fresh temporary directory, including the then-advertised 20
chart kinds and native data edits/reopen/Undo/Redo. Both baseline and edited
20-chart decks passed PowerPoint opening, all 40 workbook edit/restores, all
40 slide captures and nonblank chart-region pixel checks. Office 2016 schema
validation passed for 60 chart parts/workbooks (including retained resources)
and 12 chart-style/color-style parts. The four focused fixtures independently
passed whole-package, six chartEx-part/workbook, and style validation.

Local evidence root: `%TEMP%/AISlide-chartEx-fix-8SKqed`.
The final chart deck is `demo/editing-charts.pptx` (166,472 bytes), SHA-256
`472c9346755f7368a8f80ba02b0b388100f89dc44cfc34b40930f46aec236839`.
Its baseline is `demo/editing-charts-baseline.pptx`, SHA-256
`8917ded5e2e5d91fa15d0c2fd2012f02371b47a3bf2b5a9bf9b3031634738919`.
`demo-office-*.log`, `demo-schema-proof.log`, `demo-pixel-proof.log`, and
`schema-pixel-proof.log` bind source hashes to disposable capture copies.
The final 12-slide feature and 3-slide portrait decks also passed schema and
PowerPoint inspection; embedded-font recognition remains a separate boundary.
These results are fixture-specific, not general Office visual parity.

## Official References

- [MS-ODRAWXML 2.1.5: ChartEx content type and relationship](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/5d0d453e-adac-43be-a797-59b9916593dd)
- [MS-ODRAWXML 2.24.4.19: native series layouts](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/2ea3f228-fe39-4f55-b8ec-cee89596e926)
- [MS-ODRAWXML 2.24.3.15: data blocks/dimensions](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/b895f5cf-d6a5-4fc1-b566-7b594b2a4e60)
- [MS-ODRAWXML 2.24.3.85: subtotal indices](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/4fc38c3f-c55e-4453-8a06-21f92056554e)
- [MS-ODRAWXML 5.22: complete chartEx schema](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/e2723b0a-9120-42a5-bd11-c252ccb13c1e)
- [Office 2016 ChartSpace reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.office2016.drawing.chartdrawing.chartspace)
- [Open XML SDK 3.5.1 schema metadata](https://github.com/dotnet/Open-XML-SDK/blob/v3.5.1/data/schemas/schemas_microsoft_com_office_drawing_2014_chartex.json)

The implementation remains independent Rust XML/ZIP code. Microsoft schema
metadata and SDK are used for reference/validation only, not as a PPTX engine.

## Historical Classic 18 PowerPoint Qualification (2026-09-18)

This result concerns the original 18 classic chart kinds, not funnel/waterfall.
All 18 default factories opened in PowerPoint. With the editing-demo options,
six kinds failed during `Presentations.Open`: doughnut, area, radar,
radar_filled, column3d and bar3d. The other 12 opened, including combo with a
secondary axis, trendline and error bars, bubble, and pie3d.

The isolated cause was `c:dLblPos val="ctr"` under `c:dLbls`. Removing only
that node made all six controls open. A per-point position control also failed
for column3d. No extra 3D axes, disabled chart families, alternate chart engine,
changed data, or protection changes were needed.

The shared writer now omits that position node for these six families and for
area groups within combos. Office uses automatic native label placement;
in particular, 3D labels may appear at bar ends, not at their centers. Explicit
`center` remains AISlide authoring intent in a namespaced `c:extLst` extension,
not a native position-parity promise. Unspecified positions remain unspecified.
Other explicit positions are rejected for these families. Shared capability
limitations disclose the mapping. Labels and workbooks remain native/editable.

Native edits accept either the complete current encoding or the exact previous
AISlide center-label encoding, with strict synchronized-workbook comparison.
Unknown XML/extension attributes and independent workbook changes still block
replacement; no-op output and Undo preserve original bytes. No-op saves do not
repair old unreadable output. Create a fresh deck from the synthetic scene for
qualification, rather than retaining unrelated old resources through deletion.

The regenerated classic-18 fixture matched the reported failing fixture's
series and options after 18 native data edits, with exact Undo/Redo. It passed
Open XML SDK 3.5.1 validation, including all 36 embedded XLSX resources (18
baseline plus 18 edited), and the disposable-copy PowerPoint helper verified
18 slides, 18 native charts, 18 workbook edit/restores, 18 notes and captures
of every slide. SHA-256:
`e285b881a4316a54bdf5a8ed3135acb1e1debabafe9ace8b26c8f889651f29a9`.
The source hash stayed unchanged. This is fixture-specific opening/rendering
and editability evidence, not general Office visual parity.

Regression coverage is in `crates/aislide-core/tests/chart_format.rs`:
`fixed_center_labels*` exercises all six families, an area/line combo,
implicit versus explicit intent, legacy edits, unknown-content rejection,
workbook/cache synchronization and exact Undo.

At this historical run, `tools/editing-demo.mjs <new-temp-directory>` generated
20 kinds; the current catalog/demo has 24. For classic-18 qualification, select its 18 classic slides
in memory into a fresh document before export; do not remove slides from an
imported package and assume unused chart resources have been removed.
The separately regenerated 12-slide feature and 3-slide portrait fixtures
also passed schema and disposable-copy Office inspection. Their checks do
not qualify chartEx or embedded font recognition.