---
name: aislide-authoring
description: 'AISlide MCP / SDK で編集可能な PowerPoint / PPTX を作成・編集・検証・書き出しするときに使う。画像、ノート、アクセシビリティ、型付きバッチ、検証・busy・revision エラーの回復を担当する。日本語技術資料の構成は tech-deck-ja に任せ、このスキルが実行を引き継ぐ。'
---

# AISlide Authoring

## Scope

Use AISlide MCP/SDK and its shared core, never another PPTX engine. For Japanese
technical storytelling use `tech-deck-ja` once; simple edits need no story pass.
Honor the requested language and separate facts, assumptions and synthetic data.
Skill installation does not connect MCP.

## Short Workflow

1. Confirm audience, purpose, required pages, evidence, original images and notes.
	Never invent data or replace a required source image with a redraw.
2. Resume with `list_decks` / `get_deck_summary`. Discover advanced tools and fetch
	`get_tool_schema({name:"apply_operations"})` before advanced elements, rich text,
	composition, parts or graphs. Avoid full-deck reads for IDs.
3. Choose `compile_report` for its fixed layouts, guided authoring for evidence-led
	outlines, or `create_presentation` for content-oriented/freeform work. Set
	`setup:{design_preset,font_family,theme?}` before inserting diagrams: preset,
	explicit theme, then font override, at revision zero. Confirm installed Japanese
	glyph coverage; naming a font does not install/embed it. On older servers,
	set the theme before content.
4. Establish reusable title/header/footer/body regions and styles. On generated
	empty slides, `compose_slide` accepts 1-3 blocks: cards, callout, text, steps,
	comparison, part or graph. Reuse `style`; default card padding is 24px. Existing
	native/nonempty slides reject. Use explicit editing to preserve templates.
5. For an icon-led briefing style read `references/technical-panels.md`
	(panels, icon cards/rows, shifts, steps, agenda, screenshot callouts, Lucide icons) only on request.
	Prefer managed parts/graphs. A composition graph block takes coordinate-free
	`input` and inserts managed metadata in one operation. `layout_graph` returns
	a bounded grid GraphSpec, not a hierarchical layout: 1-48 nodes, 64 edges,
	1-8 columns (default 3), no groups or node resizing.
6. Register approved local images with `register_asset` or atomic `register_assets`
	(1-32). Use returned `asset_id` and dimensions; never regenerate/output base64
	through the model. Optional `prepare_assets` batches explicit resize/format
	changes and preserves originals. Use `fit:"contain"` or intentional `fit:"cover"`.
	Omitting `fit` uses legacy stretch. Retain attribution.
7. Batch final content and metadata into 1-128 `apply_operations`. After target
	insertion, include `update_notes`, `set_table_headers`, `set_accessibility`
	(metadata uses `element_id`). Supply the planned revision/hash; a changed batch
	is one Undo and invalid input rejects atomically. Do not refresh guards merely
	to force a stale plan through.
8. `set_text_padding` takes text/shape IDs and slide-pixel edges, leaving positive
	content space without shrinking frames/fonts and detaching inheritance when
	changed; `null` resets defaults. `set_rich_text` takes only paragraphs; core
	derives plain text and empty input clears it. Use dedicated operations for
	dynamic fields. Do not weaken canonical plain/rich validation.
9. Serialize core-backed AISlide calls, including reads/previews. Await each batch;
	do not assume automatic queues/retries or parallel edit capacity per handle.
10. Review all required pages in groups of at most 8, then recheck affected pages.
	 For customer/PDF delivery use `set_references`; check `publication.excluded`.
	 Read [distribution references](references/distribution-references.md);
	 for Japanese use `title:"参考資料"`.
	 Compact preview defaults to the first 8; inspect `page_scope`. Verify images,
	 notes, headers, alt text, Japanese glyphs/fallbacks, inner padding, overflow and
	 graph collisions. Ordinary composition text rejects overflow; managed parts
	 retain their bounded fitting rules. Structure is not Office visual parity.
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

## Further Detail

This workflow is portable after copying. When a source checkout is available,
consult `<repo>/docs/api.md` and `<repo>/docs/authoring/README.md` as needed;
never assume a fixed relative path from a user-level installation to the repo.