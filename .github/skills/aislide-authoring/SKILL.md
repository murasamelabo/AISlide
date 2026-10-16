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

1. Confirm audience, purpose, required pages, evidence, source images and notes.
	Never invent data or redraw required source images.
2. Use `list_decks` / `get_deck_summary` for IDs. Discover advanced tools; fetch
	`get_tool_schema({name:"apply_operations"})` before advanced elements, rich text,
	composition, parts or graphs.
3. Choose `compile_report`, guided authoring or `create_presentation`. Set
	`setup:{design_preset,font_family,theme?}` before content. Confirm installed
	glyph coverage; font names do not install/embed.
4. Reuse title/header/footer/body regions and styles. On generated empty slides,
	`compose_slide` accepts 1-3 blocks: cards, callout, text, steps, comparison, part
	or graph. Reuse `style`; default card padding is 24px. Native/nonempty slides
	reject; edit them explicitly to preserve templates.
	If `layout_patterns` exists and the user has not opted out, resolve one per
	slide for the deck, use its frames and follow `fallback` when `fits` is false.
	On a request to use/skip patterns, call `set_layout_patterns` (on/off/status).
	Optional `component_styles` / `component_profile:"technical_briefing"` shares
	roles across recipes, patterns and manual elements; preserve imported templates.
5. For a requested icon-led briefing style, read `references/technical-panels.md`.
	Prefer managed parts/graphs. A composition graph block takes coordinate-free
	`input` and inserts managed metadata in one operation. `layout_graph` returns
	a bounded grid GraphSpec, not a hierarchical layout: 1-48 nodes, 64 edges,
	1-8 columns (default 3), no groups or node resizing.
6. Register approved local images with `register_asset` or atomic `register_assets`
	(1-32). Use returned `asset_id` and dimensions; never regenerate/output base64
	through the model. Optional `prepare_assets` batches explicit resize/format
	changes and preserves originals. Use `fit:"contain"` or intentional `fit:"cover"`.
	Omitting `fit` uses legacy stretch. Retain attribution.
7. Batch 1-128 `apply_operations`; after targets exist include `update_notes`,
	`set_table_headers`, `set_accessibility`. One Undo; atomic rejection.
	Use planned revision/hash; never refresh to force a stale plan.
8. `set_text_padding` must leave positive content space; `null` resets defaults.
	`set_rich_text` takes paragraphs; core derives plain text; empty input clears.
	Use dedicated dynamic-field operations; keep plain/rich validation.
9. Serialize core-backed AISlide calls, including reads/previews. Await each batch;
	no automatic queues, retries or parallel edits per handle.
10. Review all required pages in groups of at most 8, then recheck affected pages.
	 For customer/PDF delivery use `set_references`; check `publication.excluded`.
	 Read [distribution references](references/distribution-references.md);
	 for Japanese use `title:"参考資料"`.
	 Compact preview defaults to the first 8; inspect `page_scope`. Verify images,
	 notes, headers, alt text, Japanese glyphs/fallbacks, inner padding, overflow and
	 graph collisions. Composition text rejects overflow; managed parts
	 keep bounded fitting. Structure is not Office visual parity.
11. Export under a new approved filename. Verify actual path, hash, page count and
	 review scope. Report unmet requirements, substitutions and unverified items.
	 Never overwrite originals, publish, deploy or install without authorization.

## Limits And Recovery

- Asset roots come only from startup `--asset-dir`; no request widens permission
  or fetches external relationships. Registry: 32 handles / 64MiB raw. PNG/JPEG:
  1MiB; SVG: 256KiB; archives: 16MiB; fonts: 12MiB; evidence: 2MiB. `usage` is
	registry-only; `raster_cost` is not document capacity approval. Closing handles
	does not remove document images. Registration/preparation is atomic.
- Report bodies: at most 4 items. Process: 2-4 labels, each 80 Unicode scalars;
  other bodies: 240. Notes: 8000. Fix the reported field path; do not silently
  reduce agreed pages/content. Cards: 1-6 items, at most 3 columns.
- Fix validation/capacity inputs without bypassing guards or flattening managed
  objects. On busy, stop parallel calls and await the existing operation, no polls.
  After timeout/cancellation inspect summaries and output manifests; do not assume
  nothing happened. Save work before reconnecting: handles are process-local.
- `_meta.aislide_timing` is not model latency or pure CPU time; do not claim speedups.

## Select And Review The Expression

Use the [selection discriminators](references/technical-panels.md#selection-discriminators).
Record question, relation, primary visual, selected/rejected reasons, emphasis and
style roles. `recommend_recipes` accepts goal/dimensions/position need; inspect
rejected/no-fit, then preview. Check relations, source readability, anchors and
role consistency, not just overflow. Never infer facts or edit managed children.

## Further Detail

With a source checkout, consult `<repo>/docs/api.md` and `<repo>/docs/authoring/README.md`.
For user-level installs, do not assume a relative path to the repo.