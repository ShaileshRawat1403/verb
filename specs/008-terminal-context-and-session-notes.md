---
id: 008
title: Terminals, context delivery and session notes
stage: review
branch: design/bench-consolidation
created: 2026-11-11
work-item: 001-bench-consolidation
---

# 008 · Terminals, context delivery and session notes

## Problem

Owners need to see which spec and role a terminal is running, what context was delivered, and what an ended session reported without changing the spec revision.

## Acceptance criteria

- [x] Launch and display spec and role bindings while preserving old sessions and terminal continuity. (R6, R10)
- [x] Review exact context before resending and record each delivery privately. (R9, R13)
- [x] Use the existing reviewed handoff and retain its revision when a session note is saved. (R12, R14)
- [x] Save bounded, redacted, attributed session notes and include them in subsequent per-spec context. (R14)
- [x] Fill and refit the centre terminal, with the same spec launch form available from Plan and the empty state. (R4, R6)

## Storage and limits

Assignment and delivery metadata are optional on session records. Resend shows the exact text and pastes it into a running agent after confirmation, without submitting it. The delivered context lives in the private project store with an 8192-byte maximum per delivery. After 64 deliveries, a session refuses another resend and retains its existing records.

Verb saves notes under `specs/sessions/<spec-id>.md` with the project lock held. The 4096-byte entry cap and 128 KiB file cap refuse new writes without deleting existing notes. The note uses the Git comparison when available. Reported outcome, edited file lists, and reported checks are unverified.

## Verification

[Browser journeys](../desktop/web/e2e/bench-shell.spec.js) verify four spec terminals, desktop split and PTY column changes, mobile full-height layout and tab labels, context preview and resend, and saving a note and receiving it on the next launch. [Before and after screenshots](../work/001-bench-consolidation/screens/terminals/) include the running terminal, launch, selected-spec empty state, assignment, context, handoff and note views at 1440 and 390 pixels in light and dark, plus desktop split.

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-11 · Shailesh Rawat via Codex · created spec and linked Bench work item
- 2026-11-11 · Shailesh Rawat via Codex · stage spec → plan
- 2026-11-11 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-11 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-11 · Shailesh Rawat via Codex · verified old session records, owner-only assignment audit, exact recorded launch and confirmed resend text, redaction, note caps and ordering, unchanged reviewed handoff revision, and subsequent launch receipt
- 2026-11-11 · Shailesh Rawat via Codex · verified xterm 5.5.0 rewraps completed and newly printed output at 390 pixels and clips the line containing the cursor at resize
- 2026-11-11 · Shailesh Rawat via Codex · formatting, Clippy, 476 Rust tests (5 ignored), 96 web unit tests and 17 Chromium journeys passed
- 2026-11-11 · Shailesh Rawat via Codex · stage verify → review
