---
id: 012
title: Board, Needs you and Preview cleanup
stage: review
branch: design/bench-consolidation
created: 2026-11-12
work-item: 001-bench-consolidation
---

# 012 · Board, Needs you and Preview cleanup

## Acceptance criteria

- [x] Board shows work item stage counts, observed attention first, and a collapsed unlinked lane. (R4, R10)
- [x] Needs you opens observed waits, failed checks, recorded gate warnings, and stale proof. (R4, R5)
- [x] Archive and unarchive preserve files. Stage moves warn and record. (R4, R5)
- [x] Preview uses one picker and three actions while preserving drafts. (R6)

## Behavior

Board groups specs by work item, orders items with observed attention first and uses the latest spec audit or evidence entry for recency. The unlinked lane starts collapsed. Archiving is an owner action stored in the private project store and leaves the work item files in place. Stage moves use the existing warning and audit path.

Needs you shows four observed signals: a session waiting for input or approval, a current failed check, a recorded stage-gate warning, and stale recorded proof. Planning reports retain their own section and handling controls. They do not count as Board attention.

A reviewed handoff cannot be correlated to a unique launched session from the existing audit. The handoff line lacks a reviewed marker and a session ID. Its adjacent start line carries an eight-character prefix and also appears on legacy handoffs. The fifth signal is in the [backlog](../work/001-bench-consolidation/specs-plan.md).

Preview uses one search picker and keeps the existing file tabs and drafts. Preview displays documents, images, and saved app previews. Edit and Source reuse the existing editor and file limits.

## Verification

Rust formatting and Clippy passed. The all-targets Rust suite passed 478 tests with five ignored. JavaScript passed 100 tests. Four Studio regression journeys passed.

The capability probe test uses separate immutable fixture executables and distinguishes spawn, process deadline, stdout deadline, and invalid results. A deliberately slow fixture reaches the one-second deadline. The earlier failure's cause remains unverified. The production deadline and fallback retain their prior behavior. Measuring cold-start time on the phone node is in the backlog.

The 12 focused browser journeys passed on the final build. The earlier complete run passed 32 journeys and exposed two desktop stale-proof failures. The spec-list refresh was returning fresh evidence while the Board was selecting an older workspace summary. The final run uses the freshly fetched spec summaries.

[Browser journeys](../desktop/web/e2e/bench-shell.spec.js) exercise stage warnings, archive and unarchive, opening and clearing observed signals, and Preview continuity. [Screenshots](../work/001-bench-consolidation/screens/board/) cover the changed surfaces at 1440 and 390 pixels in light and dark.

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-12 · Shailesh Rawat via Codex · created spec and linked Bench work item
- 2026-11-12 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-12 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-12 · Shailesh Rawat via Codex · stage verify → review
