---
name: aislide-authoring
description: 'AISlide MCP / SDK で編集可能な PowerPoint / PPTX を作成・編集・検証・書き出しするときに使う。画像、ノート、アクセシビリティ、型付きバッチ、検証・busy・revision エラーの回復を担当する。既存台帳を優先し、台帳のない新規構成だけを slide-planning に任せる。'
---

# AISlide Authoring

## Scope

Use AISlide core via MCP/SDK only. Reuse supplied ledgers without replanning.
For new structure without a ledger, use `slide-planning` once; ask if absent.
Wording: [Japanese](../japanese-editing/SKILL.md), [English](../english-editing/SKILL.md).
Bounded edits skip planning. Honor scope/language; separate facts from assumptions.
Skills do not connect MCP. Label synthetic data.

## Short Workflow

1. Confirm audience, purpose, pages, evidence, images and notes.
	Never invent data or redraw source images.
2. IDs: `list_decks` / `get_deck_summary`. Expose any `loaded:false` tool in
	`discover_tools` via `get_tool_schema({name})` right before use (compact mode
	retains 4); fetch `apply_operations` before advanced elements, rich text,
	composition, parts or graphs.
3. Use `compile_report`, guided authoring or `create_presentation`. Set
	`setup:{design_preset,font_family,theme?}` before content. Check glyph coverage;
	names do not install/embed fonts.
4. Reuse deck regions/styles. On empty slides, `compose_slide` accepts 1-3 blocks
	(cards, callout, text, steps, comparison, part, graph; default card padding 24px).
	Native/nonempty slides reject; edit explicitly to preserve templates. Resolve one
	`layout_patterns` per slide unless opted out (`set_layout_patterns`); follow
	`fallback` when `fits` is false. Optional `component_styles` /
	`component_profile:"technical_briefing"` shares roles.
5. For icon-led briefings, read `references/technical-panels.md`. Prefer managed
	parts/graphs and native charts/tables; never draw data charts from shapes.
	Composition graph `input` is coordinate-free. `layout_graph` is a grid:
	1-48 nodes, 64 edges, 1-8 columns (default 3), no groups or node resizing.
6. Register approved local images with `register_asset` / `register_assets` (1-32).
	Use `asset_id` and dimensions; never output base64. `prepare_assets` batches
	edits, keeping originals. Use `fit:"contain"` or `fit:"cover"`; omitting `fit`
	uses legacy stretch. Retain attribution.
7. Batch 1-128 `apply_operations` (`update_chart` / `remove_elements` adjust in
	place); after targets exist include `update_notes`, `set_table_headers`,
	`set_accessibility`. One Undo; atomic rejection. Use planned revision/hash.
8. `set_text_padding` leaves positive space; `null` resets defaults. `set_rich_text`
	takes paragraphs; empty input clears. Use dynamic-field ops; keep validation.
9. Serialize core-backed AISlide calls, including reads/previews. Await each batch;
	no queues, retries or parallel edits per handle.
10. Review all required pages in groups of <=8; recheck affected pages. Use
	 `set_references` (`title:"参考資料"` in Japanese; see
	 [distribution references](references/distribution-references.md)) and check
	 `publication.excluded`. Inspect `page_scope` and `canvas_scale`; read real font
	 px from `get_deck_summary` / `preflight_presentation`, not downscaled previews.
	 Do not replace fixed-font parts (`SMALL_TEXT`) or chart previews (`CHART_PREVIEW`)
	 with shapes. Verify images, notes, headers, alt text, glyphs, padding, overflow
	 and collisions. Structure is not Office visual parity.
11. Interactive exports: [mandatory playback checks](../../../docs/authoring/powerpoint-interactive.md#mandatory-creation-and-delivery-checklist).
	 Test exported HTML after clicks; rebuild/re-export fixes. Export to a new
	 approved filename; verify path, hash, `producer.git_commit`, page count and
	 scope. Report native chart/table/part counts, hand-drawn data slides with
	 reasons, unmet requirements, substitutions and unverified items. Never overwrite,
	 publish, deploy or install without authorization.

## Limits And Recovery

- Asset roots use `--asset-dir`; never widen permissions or fetch external links.
  Registry: 32 handles / 64MiB raw (PNG/JPEG 1MiB, SVG 256KiB, archives 16MiB,
  fonts 12MiB, evidence 2MiB). `usage` is registry-only; `raster_cost` is not
  document capacity. Closing handles retains document images.
- Report bodies: <=4 items. Process: 2-4 labels, 80 Unicode scalars each; others:
  240. Notes: 8000. Cards: 1-6 items, <=3 columns. Fix field paths; never drop
  agreed content.
- Fix validation/capacity inputs without bypassing guards or flattening managed
  objects. On busy, await the active call without polling. After timeout/cancel,
  inspect summaries/manifests. Save before reconnecting.
- `_meta.aislide_timing` is not model latency or pure CPU time.

## Select And Review The Expression

Follow [visual defaults](references/visual-selection.md).
Explicit user exclusions override this default.
Record per-slide question, relation, primary visual, selected/rejected reasons,
emphasis and role consistency. Use [selection discriminators](references/technical-panels.md#selection-discriminators)
and `recommend_recipes`; never infer facts or edit managed children.

## Further Detail

Source docs: `<repo>/docs/api.md`, `<repo>/docs/authoring/README.md`.