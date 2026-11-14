---
id: 011
title: Preview tab and Review polish
stage: review
branch: design/bench-consolidation
created: 2026-11-12
work-item: 001-bench-consolidation
---

# 011 · Preview tab and Review polish

## Acceptance criteria

- [x] Browse and preview documents, images, and saved applications with the existing Studio controls. Hide Preview when no previewable project files exist. (R4, R6)
- [x] Preserve the same editor draft and terminal across tab and terminal switches. (R6, R10)
- [x] Show observed Review verdicts, scoped changes and a compact handoff. (R5, R12)

## Behavior

Preview mounts the existing Studio canvas and file browser in Bench. The same editor retains unsaved text across tab and terminal switches. The app preview continues to use the saved-file sandbox. Leaving Preview stops the app or discards a pending preview response, while the editor draft stays available. Preview is hidden when the file browser has no visible entries.

Review uses the latest Verb-observed check for each criterion and the ledger's accepted proof. It shows no verdict before an observed run. A stale or inconclusive observed check blocks a ready verdict. The selected comparison in Changes uses the existing last-known-good and implementation diff endpoints. The implementation comparison shows the latest commit when there is no separate comparison branch, as identified in the diff view.

The reviewed handoff displays a short revision while submitting the full revision. The dialog looks up the current live spec agent and puts the running-session note above the buttons.

## Verification

[Bench browser journeys](../desktop/web/e2e/bench-shell.spec.js) verify document, image and saved app previews, file search, empty-list visibility, late app response cancellation, and continuity of the editor draft and terminal. They also exercise an observed failure verdict, criterion numbers, handoff source and footer, both diff comparisons and the Git menu. [Screenshots](../work/001-bench-consolidation/screens/preview/) show the changed surfaces at 1440 and 390 pixels in light and dark.

Rust formatting and Clippy passed. The all-targets Rust suite passed 478 tests with 5 ignored. JavaScript passed 98 tests. Bench and project-switcher passed 30 browser journeys, the targeted Studio regression run passed four, and all five Preview journeys passed against the final build.

### Prior Rust failure

`isolation::external::tests::capability_probe_is_bounded_and_requires_a_successful_versioned_reply` failed at the successful-fixture assertion with `called Option::unwrap() on a None value`. The fixture supplies valid capability JSON, so the panic establishes that the probe returned no result. The original run had no spawn, exit, timeout, or stdout diagnostics. A temporary instrumented full unit-suite rerun passed and recorded exit status 0 and 69 bytes decoded for that fixture. The original trigger remains unverified. [Probe and test](../desktop/src/isolation/external.rs).

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-12 · Shailesh Rawat via Codex · created spec and linked Bench work item
- 2026-11-12 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-12 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-12 · Shailesh Rawat via Codex · stage verify → review
