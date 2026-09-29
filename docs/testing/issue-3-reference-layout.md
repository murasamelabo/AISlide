# Issue #3: Imported-Deck Reference Layout

Validated on Windows on 2026-09-29, based on PR #2 commit `5e6c0c6`.
All fixtures were synthetic; the reviewer's actual 24-page presentation was not
available. No existing source presentation or main worktree was modified.

## Contract

- Explicit `placement:"appendix_only"` publishes approved references on appendix
  pages without adding markers or changing original slide elements. Captions
  identify current 1-based full-deck page numbers, not private titles or notes.
- Stable slide IDs determine correspondence. Reorder, deletion and appendix
  movement update only caption text and ownership hashes, not frames or fonts.
  Space is reserved for up to 256-page numbering; overflow rejects atomically.
- Normal markers stay in the bottom quarter with 8px clearance. Failure reports
  a candidate frame and only its intersecting obstacle IDs, with an explicit
  `appendix_only` alternative. No silent fallback changes the chosen mode.
- Native PPTX export/reopen, Undo and native PDF URL text/URI targets are tested.
  An imported dense PPTX test also compares original slide, relationship and
  notes XML exactly before and after adding the appendix.

## Verification

Commands run from the isolated issue worktree with the rebuilt CLI selected by
`AISLIDE_CORE_BINARY`. Rust used `CARGO_BUILD_JOBS=1`, debug info disabled for
dev/test, `CARGO_INCREMENTAL=0`, and a worktree-local `CARGO_TARGET_DIR`.

| Check | Result |
| --- | --- |
| `node tools/cargo.mjs test --workspace --locked` | 819 passed, 0 failed, 8 ignored |
| Final `node tools/cargo.mjs test -p aislide-core --test references --locked` | 35 passed, 0 failed, 1 Office-only ignored |
| `node --test --test-concurrency=1 tools/client.test.mjs tools/mcp.test.mjs tools/mcp-poc.test.mjs tools/references.test.mjs` | 127 passed, 0 failed |
| `npm run build` | Passed; existing large-chunk advisory |

The final reference test run adds the imported-dense-PPTX preservation test to
the earlier workspace run. The first version of that test incorrectly assumed
the appended package part would be named `slide2.xml`; imported packages can use
another name. It now checks the reopened appendix and separately preserves the
original XML comparisons.

## Limitations

- Caption numbers refer to the complete deck, including moved appendix pages;
  subset PDF exports are not renumbered. Export the full deck when using them.
- Empty-layout and readable-theme-link requirements still apply to appendices.
- This work does not scrub existing notes or source metadata.
- No new Office rendering comparison, coverage percentage or CodeScene score
  was measured. Source-review agents lacked file access; the security review
  used supplied implementation details, not an independent source audit.