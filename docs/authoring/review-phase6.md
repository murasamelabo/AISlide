# Phase 6 Review Contracts

Scope: G40/G41/G43, 2026-09-19. Core, SDK, MCP and Studio are connected, including PDF consumption of explicit table headers and the repeated native-body-save repair. This is a bounded implementation, not complete Office comment interoperability, PII detection, or WCAG certification. Final product verification, Office checks, publication and regular Windows installation are tracked separately in the [completion plan](../planning/editing-completion.md).

## Modern Comments

Legacy `Comment` and its four operations remain separate and unchanged. `SlideReview.modern_threads?: ModernThread[] | null` is optional. Protocol transactions preserve an omitted/null field from the current canonical model, including unsaved replies/status changes; an explicit empty array removes modeled threads. The review transaction wrapper handles whole-deck, slide-list, slide and review replacement without intercepting Undo inverse operations. Typed operations also use the current model. Direct low-level Rust `document::transact` callers must use the review-aware wrapper when emulating older clients; the low-level engine itself was not changed.

- Thread/reply fields: GUID `id`, immutable `author`, immutable XML-dateTime `created`, `status: active | resolved | closed`, typed rich-paragraph `body`. Replies are a flat `replies` list, never recursive. Omitted XML status reads as active.
- Authors contain GUID `id`, `name`, `user_id`, `provider_id`, optional `initials`. New typed operations allocate distinct deterministic version-8/variant-2 GUIDs from the current canonical operation input; names are never merged. Offline authors use `provider_id: None` and `user_id` equal to the supplied display name. These are not authenticated identities or notification targets.
- `modern_comment` takes `document`, `expected_revision`, `slide_id`, and `operation`. Operation variants are `create {draft, anchor}`, `reply {thread_id, draft}`, `set_status {comment_id, status}`, `update_body {comment_id, body}`, and `remove {comment_id}`. A draft contains `author_name`, optional `initials`, explicit `created`, and `body`; it cannot supply native identity or raw XML.
- SDK: `session.modernComment(slideId, operation, options)`. MCP uses the same `modern_comment` typed union. Studio exposes a Modern/Legacy selector, thread/reply status controls, rich-run body editing and removal. Mutations use the existing revision/hash transaction, cancellation and Undo paths.
- Anchor type includes unknown, slide, shape and reserved text_range, plus preserved for origin-only opaque anchors. Simple imported slide/shape monikers are projected with slide ID/creation ID and shape ID/creation GUID. Repeated, unknown or text-range monikers remain `preserved`. **Only explicit unknown is currently authored.** New slide/shape/text-range anchors and typed text-range projection remain unfinished. Modern-comment slide copy rejects, including when a client omitted the modern field. Existing anchors are immutable, never silently changed to unknown.
- Status edits patch only the status attribute. Supported body edits replace paragraphs, preserving body frame/list properties and existing end-paragraph properties. Namespace-aware isolation of retained `endParaRPr` supports repeated edit/save/reopen without duplicate namespace declarations. Unknown rich markup or unrepresentable font variants reject body edits. Unknown comment extensions remain byte-preserved through unrelated edits and Undo.
- Native thread/reply array reordering is rejected rather than accepted and silently ignored by the writer. Existing relative order is protected; no comment-ordering UI or arbitrary native reordering support is promised.
- Readers validate exact namespace/root/MIME, GUID uniqueness, author references, created timestamps, flat replies, child ordering, and the exact slide commentRel link. Writers reject reassigned existing metadata, shared comment parts, duplicate deck/package GUIDs, and external/missing comment-part relationships. Review-aware protocol transactions also reject forged author/created/anchor/parent metadata in authored documents without an origin. Signed/labelled/protected comment mutations and cleanup reject.

Public schema: [MS-PPTX 2018/8 main](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-pptx/af0dc8d7-ee58-435b-80fb-72f2b351b689).

Native contracts:

| Item | Value |
| --- | --- |
| Namespace | `http://schemas.microsoft.com/office/powerpoint/2018/8/main` |
| Comment part | `cmLst/cm`, `application/vnd.ms-powerpoint.comments+xml` |
| Authors part | `authorLst/author`, `application/vnd.ms-powerpoint.authors+xml` |
| Relationship base | `http://schemas.microsoft.com/office/2018/10/relationships` |
| Ownership | internal slide `/comments`; internal presentation `/authors` |
| Slide extension | `p:extLst/p:ext uri="{6950BFC3-D8DA-4A85-94F7-54DA5524770B}"/m:commentRel r:id` |
| Thread child order | anchor, optional pos, optional replyLst, optional txBody, optional extLst |

Clean-copy comments selection removes exact legacy/modern comment and author MIME parts, their relationships/content-type entries, and only the exact modern commentRel extension. Other slide extensions and unselected content remain. Cleanup does not revoke source/history copies and never unprotects a package.

## Inspection Candidates

`inspect_document` / `session.inspectDocument()` retains the existing seven removable categories. Their `paths` arrays are now empty to avoid leaking content-derived names. Separate `candidates` entries contain only `rule`, `scope`, `surface`, `count`, numeric `locations`, and `masked_value: [REDACTED]`. No snippets, matched values, source paths or author names are returned.

- Rules: `email_candidate`, `phone_candidate`, `personal_property_candidate`. These are conservative candidates requiring human review, not identities or certainty claims. Phone detection requires common formatting; GUID/date negative fixtures do not match.
- Scopes: `current_deck` and `embedded_origin`, independently scanned. Modeled text/table cells, notes/rich notes, comments/replies/authors, source text/cells, and bounded package core/custom properties are covered. Origin scanning reads inert slide/notes/comment XML and embedded source metadata. No OCR, binary attachment execution or external fetch occurs.
- Limits: 8 MiB scanned text budget and 4096 retained candidate locations; `candidate_scan_truncated` reports exhaustion. Numeric positions are scope/surface-local and end with a Unicode-scalar offset. They are not replacement authority or stale-safe redaction ranges.
- Candidate rule names are not `InspectionCategory` variants and are rejected by `export_clean_copy`. Studio renders them in a separate nonselectable table. Existing explicitly confirmed category cleanup remains available.
- Inspection failure returns a fixed error without candidate data. History/recovery/original packages can retain prior content. No guided automatic PII replacement was added; use separately confirmed existing editing workflows and rescan. No claim of complete detection or removal is made.

## Accessibility

`SlideReview.table_headers` maps table element IDs to `unknown | none | first_row | first_column | both`. Missing declarations mean unknown. Table first-row styling is never inferred to be semantic headers. `set_table_headers` and SDK `session.setTableHeaders(slideId, elementId, policy, options)` update this metadata; Studio exposes the same policy control. It persists in the table's cNvPr extension `urn:aislide:table-headers:1`.

Stable issue codes include `table_headers_unknown`, `table_headers_none_review`, `table_declared_header_empty`, `table_merged_semantics_unverified`, `image_alt_missing`, `chart_alt_missing`, and `group_alt_missing`. Repair targets identify `table_headers` or `alternative_text`; existing issue navigation selects the object for repair, and the checker can be rerun. Decoration meaning remains a manual-review hint. Contrast resolves the slide's effective master theme; complex/inherited backgrounds remain explicitly unverified.

PDF export consumes `review.table_headers` and uses correctly parented table/row/cell structure nodes and MCIDs. Missing/unknown/none produce TD only; first_row produces TH with Scope Column, first_column with Scope Row, and both uses Scope Both at the corner. Merge spans are retained without inferring complex header associations. The table-parent repair and header-policy checks passed 11 focused PDF tests on 2026-09-19. Screen-reader behavior, arbitrary merged associations, WCAG/PDF-UA certification and Office visual parity remain unverified.

## Verification Scope

- Initial focused Rust: 32 review_features + 11 review_api tests passed (43 total in the scoped log). After the repeated-body/ordering and rich-notes repairs, the comments/single-file checks passed 69 tests. These are separate scoped runs, not additive full-product totals.
- Focused Node: 8 SDK/MCP tests passed, including exact Undo, omission versus explicit deletion, authored metadata forgery, body editing, native header reopen, stale revision/cancellation, strict schemas, no-value inspection, and exact-one-BOM checks.
- Studio production build and focused ReviewPanel Oxlint passed. The earlier ESLint invocation failed because this repository uses Oxlint; that invocation is retained in the gate log, not counted as a code failure. Owned unique-port Playwright: 3 passed (modern native open/reply/resolve/Undo, nondeletable candidates, semantic header repair/rerun and 390px framing; existing legacy review/clean-copy; clean-copy active-session isolation).
- OpenXML SDK 3.5.1, Office2021 mode: final fresh writer with semantic headers, edited writer including rich-body mutation, and independently authored closed-thread/resolved-reply/same-name-distinct-author/ignorable-extension fixture each validated with zero errors. Evidence: `.artifacts/phase6-review/schema-results.json`. This is schema evidence, not an Office opening/editing test.
- Attribution logs: `.artifacts/phase6-review/{rust-final,node-final,build-final,browser-final}.log`; screenshot `.artifacts/phase6-review-candidates.png`. Only synthetic data was used.
- Final full-product core/Node/UI/native/Office gates, commit/push and desktop installation remain pending in the completion plan. The PDF header consumption described above is implemented, not a remaining integration task. The bounded static security rereview found no remaining High/Medium findings; this is not a full security audit.

Review behavior remains in the shared core, with typed SDK/MCP entry points and Studio controls. PDF semantics consume the same review metadata; display styling alone does not establish semantic headers. Focused fixtures and final product gates remain distinct evidence.