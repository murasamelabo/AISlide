---
name: aislide-authoring
description: 'Create, edit, review and export editable PowerPoint presentations with AISlide MCP or its SDK. Use for AISlide reports, source-image placement, notes, accessibility metadata, typed batch edits and recovery from validation, busy or revision errors.'
---

# AISlide Authoring

## Short Workflow

1. Establish audience, purpose, outline, sources, required images and notes. Separate sourced facts, assumptions and missing information. Do not replace a required source image with a reconstruction without disclosing that substitution.
2. Use `list_decks` and `get_deck_summary` to resume existing work. Use `discover_tools` and `get_tool_schema` only for needed advanced operations. The initial batch schema covers common shapes and metadata; fetch `get_tool_schema({name:"apply_operations"})` before composing tables, charts, groups or managed parts/graphs. Do not retrieve full documents or entire catalogs just to find a handle or revision.
3. Choose the creation path: `compile_report` for fixed layouts; guided creation for evidence-linked outlines; `create_presentation` and `apply_operations` for freeform content; managed `add_part`/`add_graph` for diagrams that must remain regenerable. Establish the theme before managed diagrams.
4. Register approved local images once with `register_asset`. PNG/JPEG responses include verified pixel `width`/`height`. Pass `asset_id` to `add_picture` or graph icons. Use `fit:"contain"` to preserve the whole cropped image inside a frame, or `fit:"cover"` to fill the frame with additional cropping. Omission retains legacy stretching; inspect `IMAGE_ASPECT_DISTORTED`. Never echo or regenerate base64 through the model. Preserve image attribution and usage conditions; requests cannot widen startup-approved roots or fetch imported relationships.
5. Assemble final content and metadata into `apply_operations` batches of 1-128 operations across slides. Use `update_notes`, `set_table_headers` and `set_accessibility` after their targets exist, including after insertion in the same batch. Use `element_id` for metadata targets. Supply the revision/hash of the state used to plan the edit. One changed batch creates one Undo; an invalid operation rejects the entire batch. Plain notes do not silently replace incompatible rich notes.
6. Serialize core-backed AISlide calls, including reads and previews. Independent external research may run in parallel. Do not use parallel AISlide calls to accelerate editing. There is no automatic core queue or retry. An SDK executor can await a bounded planned batch without returning to the model for every element.
7. Review all required pages initially, in selected groups of at most eight; after corrections, review affected pages. Compact previews cover the first eight by default and disclose coverage in `page_scope`. Check expected picture IDs and image counts, not just successful tool responses. Run the required layout/accessibility checks; structural validation is not Office parity or factual verification.
8. Export a new file or delivery, then verify the actual path, hash, page count, required images/notes and check scope. Report unmet requirements and substitutions explicitly. Never overwrite the source or imply partial-page checks covered the whole presentation.

## Constraints And Recovery

- `compile_report` body has at most four entries. `process` requires 2-4 step labels of at most 80 Unicode scalars each; other body entries allow 240. Fix the named `report.sections[index].body[index]`, not the whole deck. Preserve the requested slide count and content unless a change is agreed.
- Notes allow 8000 Unicode scalars. Table policies are `unknown`, `none`, `first_row`, `first_column`, or `both`. Accessibility uses a typed `metadata` object or explicit `null`, never an invented JSON Patch location.
- A graph title is hidden when either `spec.show_title` or `layout.show_title` is false; layout defaults cannot re-enable it. Font fallback warnings aggregate the requested/used families per element. Delivery layout checks exclude unused design definitions; explicit `measure_layout` still inspects them.
- Validation: fix the exact input path and limit. Capacity: reduce the relevant bounded batch or asset while retaining requirements. Busy: stop concurrent dispatch and wait for the already-running call to finish, without polling.
- Timeout/cancellation: inspect `list_decks`/`get_deck_summary` and any published manifest before retrying; do not assume nothing happened. Conflict: inspect the changed state and replan; never replace the expected revision/hash merely to force an old edit through.
- Keep guarded writes and stop the executor on an unexpected failure. Do not silently fall back from managed diagrams to unmanaged groups, remove safety checks, or skip required validation to save calls.
- Record MCP `_meta.aislide_timing` when available. `handler_elapsed_ms` and `core_roundtrip_ms` exclude model/service wait; `core_calls` counts attempted core requests. Pure model inference time cannot be inferred from these fields or from all non-tool time. Compare model turns, tool failures, image use and output quality separately.

## Further Detail

- [Typed APIs, asset bounds and timing semantics](../../../docs/api.md)
- [Creation paths, batches, visual review and delivery](../../../docs/authoring/README.md)

Load the relevant section only when needed. Do not install another presentation engine or change global agent configuration for this workflow.