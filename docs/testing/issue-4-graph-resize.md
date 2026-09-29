# Issue #4: Edge Graph Resize Notifications

Validated on Windows/Microsoft Edge on 2026-09-29 in isolated worktrees.
Main's working directory, running services and build output were not used.

## Evidence And Fix

The original four-level group drag/resize/ungroup/PPTX test failed three of
three local runs on the PR #2 frontend. The error was
`ResizeObserver loop completed with undelivered notifications.` It occurred in
the Canvas tab, not the native Preview. A separate Preview width-change test
passed, falsifying the first hypothesis about its observer.

Temporary test-only observer tracing showed a subnet measurement of 976x240.
Its callback returned with the same DOM dimensions, but the error event saw
976x232: a pending controlled React update committed during the browser's
measurement cycle. Delaying only measurements, or queueing updates without a
synchronous frame commit, did not eliminate the error; both probes were removed.

The accepted fix batches non-selection React Flow changes into an animation
frame and commits them with `flushSync` before that frame's layout measurement.
Selection stays immediate. Installing a canonical graph clears stale queued
changes; unmount cancels the pending frame. No observer override, ignored
browser error, dependency patch or test retry is shipped.

## Comparison

- PR #2 pre-fix interaction: 3/3 failed locally with the notification error.
- Fixed interaction with temporary diagnostics: 3/3 passed, no retries.
- Exact baseline `9ed13f8`, isolated checkout/core/dependencies: 3/3 passed with
  the same Edge, viewport and test configuration. This does not prove immunity.
- PR #2's later hosted run [36518243916](https://github.com/murasamelabo/AISlide/actions/runs/36518243916)
  also succeeded without this fix, confirming the observed failure is
  timing-dependent. It is not claimed to be a consistently failing main bug.

## Final Verification

```text
node node_modules/@playwright/test/cli.js test tests/e2e/graphs.spec.ts --retries=0 --reporter=line
```

Final result: **19 passed, 1 skipped**. The strict ResizeObserver error assertion
remains. The test additionally verifies resized geometry while the pointer is
still down, then history and native roundtrip. A Fit view hit-test now waits for
the same correct hit target instead of asserting once before rendering finishes.

One intermediate six-case run had a pre-graph navigation failure:
`net::ERR_NO_BUFFER_SPACE` loading Vite's `@react-refresh`, proven by its trace.
It was not a ResizeObserver recurrence and was not hidden by retries. The final
complete graph suite passed. React Flow transient native-handle warnings were
printed by another existing test; they were not suppressed.

The final production build passed. Studio `oxlint` passed with three existing
Fast Refresh export warnings outside the modified component. The full Studio
E2E suite was not rerun locally; the graph suite is the scoped browser gate.
Reviewer agents without filesystem access could not perform independent source
audits; a supplied-snippet review found no additional issue.

## Local Artifacts

- Worktree: `F:/rep/AISlide-issue-1`, branch `issue/3-4-reference-layout-edge`.
- Final graph artifacts: `.artifacts/issue-4-final-graphs`.
- Baseline checkout: `F:/rep/AISlide-issue-4-baseline`, detached `9ed13f8`.
- Baseline artifacts: that checkout's `.artifacts/playwright`.

These local generated artifacts are not committed or uploaded.