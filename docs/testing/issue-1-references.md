# Issue 1 Reference Distribution Verification

## Scope

- Isolated branch: `issue/1-visible-references`, based on `b299d5e`.
- This record describes initial implementation and validation. The existing
   `main` worktree was not edited or switched. Commit and PR publication were
   separately authorized on 2026-09-29; merging into `main` remains for review.
- Public references are explicitly approved input, not extracted from private
  notes or sources. Native OOXML text/links are independently implemented by
  the existing core; no presentation engine was replaced.
- See [authoring contract and limits](../authoring/references.md).

## Test-First Evidence

1. `notes_only_reference_urls_warn_without_publishing_them` ran and failed on
   the missing `SOURCE_URL_NOT_VISIBLE` finding, then passed after adding the
   read-only preflight check.
2. Publication, appendix, idempotency and edit/Undo tests failed on the absent
   `set_references` protocol operation, then passed after the core implementation.
3. Native PDF failed on missing `Annots`; PPTX reopen failed on generated-element
   fingerprint normalization. Both passed after URI annotation output and
   exported-state normalization were implemented.
4. Boundary tests reproduced full-slide-text/background confusion, trailing-dot
   localhost acceptance, discarded appendix page styling, overflowing multiline
   titles, poor dark-slide contrast and inherited appendix decoration. Focused
   tests passed after the corresponding fixes. A final regression also proved
   that deletion does not renumber later sources and that Office theme hyperlink
   colors must meet the contrast threshold before footnote placement.
5. SDK/MCP tests failed on absent method/tool registration, then passed using
   the existing revision/hash guards and compact mutation summary.

## Executed Gates

| Gate | Observed result |
| --- | --- |
| `node tools/cargo.mjs test -p aislide-core --test references` | 16 behavior tests passed; one Office-dependent test run separately |
| `node tools/cargo.mjs test --workspace` | 793 passed, 0 failed, 8 explicit ignores across 51 result groups |
| `node --test tools/client.test.mjs tools/mcp.test.mjs tools/mcp-poc.test.mjs tools/references.test.mjs` | 123 passed, 0 failed |
| `npm run build` | TypeScript and Vite succeeded; existing large-chunk advisory remains |
| `npm run encoding:check` | 319 files, 0 mismatches |
| Office verification script parse and `-WhatIf` | Passed; no files written by WhatIf |
| Real PowerPoint open and PDF/PNG export | Two slides, two URL strings, four native hyperlinks; source SHA256 unchanged |
| Explicit PowerPoint PDF parsing test | Two pages, both expected URL strings and URI targets found |

The eight ignored tests include seven pre-existing opt-in checks and the new
Office-dependent parsing test, which was run explicitly and passed. The real
MCP reference test and that PDF parsing test passed again after the final core
changes. No coverage percentage is claimed; instrumented Rust coverage was not
measured.

## Artifacts

The final generated synthetic fixture is retained locally under
`.artifacts/issue-1-final/` (ignored build artifacts, not release attachments):

- `references.pptx`
- `references.pdf` from AISlide native export
- `references-overview.png`
- `references.manifest.json`
- `powerpoint/references-powerpoint.pdf`
- `powerpoint/slide-1.png` and `powerpoint/slide-2.png`

The final PPTX SHA256 verified by the PowerPoint script was
`27B812D7F6DFDC1CE7885745713E3CCC788B0F751B548B3BD06D4A8914B3934E`.
The two PowerPoint PNGs were inspected for the fixture's body/reference markers,
readable full URLs and non-overlap. This is evidence for that fixture only, not
general Office visual parity, PDF accessibility certification or source truth.

## Environment And Review

The first full Rust run exhausted the F drive while producing parallel Windows
PDB files and failed in linking, before the full test suite. Only this worktree's
generated `target` directory was cleaned (33.8 GiB). The successful rerun used
`CARGO_BUILD_JOBS=1`, `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0` and a worktree-local
`CARGO_TARGET_DIR`. Source files and the other worktree's build outputs were not
cleaned. Last measured free space after the successful runs was 23.49 GiB.

The issue worktree also contains an unrelated `world-land.geojson` difference
that this task did not edit or revert. Its trailing whitespace makes an unscoped
`git diff --check` fail; the Issue 1 diff check excluding that file passed. Do not
include that difference when staging this task. The main worktree was still on
`main` with no uncommitted changes at the last isolation check.

Rust, code, TypeScript, security and PowerShell reviewers were invoked. Their
sessions lacked filesystem tools, so their reviews used supplied code/design
evidence; they were not independent repository audits. Confirmed findings were
reproduced and fixed with focused tests. Remaining intentional limits include
frame/text-based URL detection, explicit targeting for newly inserted slides,
manual review of private query parameters, an existing empty appendix layout,
and explicit page selection when a partial PDF would omit references.

CodeScene tools were unavailable during PR preparation. That optional check was
skipped under the user's instruction to proceed autonomously; no Code Health
score or independent maintainability approval is claimed.

## Self-Check

The scorecard below describes the initial implementation. The subsequent PR
review identified the workflow gaps recorded in the follow-up section; passing
the original tests did not establish suitability for occupied briefing footers.

| Axis | Score | Evidence or limit |
| --- | --- | --- |
| Accuracy | 4/5 | Core, MCP and Office outputs verified; no broad parity claim |
| Completeness | 4/5 | Issue workflows covered; no dedicated Studio panel or measured coverage |
| Clarity | 4/5 | Public API and rejection conditions documented; managed-output rules require reading |
| Actionability | 4/5 | Worktree, commands and artifacts retained; integration requires user approval |
| Conciseness | 4/5 | Core behavior is centralized; verification remains split across Rust and Office |

Overall: 4.0/5. Integration review should prioritize the managed-reference
transaction contract, broader Office fixtures and optional coverage measurement.
These are follow-up limits, not claims of completed independent review.

## PR Review Follow-Up (2026-09-29)

The review against `1cd0537` reproduced failures with existing footer text at
y680, placement changes on unrelated edits, CJK prose in note URLs, missing
publication feedback, and missing authoring/CI guidance. Each changed behavior
was first exercised with a failing test. Corrections are in `5a8e876` and the
integration of main `9ed13f8` into the issue branch; the main worktree was not
updated or merged into.

1. Appendix markers now search upward through free horizontal bands, leaving
   existing source labels, page numbers and body elements untouched. A synthetic
   26-slide briefing with y680 footers passes in both auto and appendix modes.
   Fully occupied pages still reject with slide/blocking element IDs and explicit
   repair guidance. The reviewer's actual 26-slide deck was not available here.
2. Only `set_references` replans placement. Ordinary edits preserve generated
   frames, placement choices, z-order and moved appendix order. Overlaps are
   accepted and reported as `REFERENCE_COLLISION` with involved IDs and repair
   suggestions; no hidden conversion from footnotes to appendix occurs.
3. Bare URLs terminate before Japanese/CJK prose. Explicitly enclosed URLs retain
   Unicode domains/paths; parsed URLs are normalized before comparison. Bare
   Unicode URL/prose boundaries remain ambiguous: quote or percent-encode them.
   Raw URLs remain omitted from diagnostics to avoid leaking private note data.
4. Core/MCP return supplied/published/excluded counts; SDK exposes a detached
   `referencePublication` snapshot. Unapproved URLs remain excluded from storage.
5. The portable skill and MCP workflow now require the distribution-reference
   step and describe a reserved x32..1248/y616..672 marker band. The technical
   briefing guide also documents the y608 body bottom variant. The integrated
   skill is 5811 bytes, below its 6000-byte limit.
6. Rust tests already ran in `verify.yml` via `test --workspace --locked`.
   `test:mcp` now also runs the reference E2E and guidance/CI contract test.
   The previous CI's three failures were unrelated full-inventory assertions
   against compact discovery; only those fixtures now request `--tool-profile full`.
7. URL-only text rows carry hyperlinks; appendix labels/headings use the selected
   theme's dk1 with a contrast-preserving fallback. Appendix movement is retained.

### Integrated Verification

- `node tools/cargo.mjs test --workspace --locked`: 810 passed, 0 failed,
  8 ignored across 52 result groups, including 25 reference behavior tests.
- Node SDK/MCP/reference and extended editing tests (nine files, concurrency 1):
  147 passed, 0 failed, 5 environment-dependent skips (152 total).
- `npm run build`: succeeded; the existing large-chunk advisory remains.
- Encoding: 324 files checked, 0 mismatches. Staged integration diff check passed.
- Updated generated fixture: `.artifacts/pr2-review/`. PowerPoint opened two
  slides with existing footer labels intact, exported PDF/PNG, and verified
  two URL strings and exactly two URL-row hyperlinks. Source SHA256 unchanged:
  `ACA52887F3E82C4CF5FDC6F2BD58D890149F79B8EB8C7F96229CE17753A4D0DD`.
  The PowerPoint PDF parsing test passed; both exported PNGs were inspected.
- Auxiliary reviewers again lacked filesystem access. Their design-level
  feedback prompted a tested Unicode URL correction, not an independent audit.

Logs are local under `target/pr2-review-integrated-{rust,node,build}.log`.
The hosted CI result after pushing these changes must be checked separately;
the local evidence above does not assert a GitHub Actions success.