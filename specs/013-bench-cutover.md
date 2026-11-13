---
id: 013
title: Bench cutover and Context
stage: review
branch: design/bench-consolidation
created: 2026-11-12
work-item: 001-bench-consolidation
---

# 013 · Bench cutover and Context

## Acceptance criteria

- [x] Board leads with actionable attention and shows one row per work item. (R4, R10)
- [x] Context exposes the project brief, shared notes, managed preview and explicit sync. (R6, R7)
- [x] Settings exposes host, connections, phone pairing and the task queue. (R10)
- [x] The Bench shell is the sole desktop navigation and replacement destinations preserve existing workflows. (R10)
- [x] The documentation index separates current documents from historical material. (R10)

## Behavior

Board puts observed attention cards before one row per work item. Each row opens the work item and shows its size, stage counts and last recorded change. The unlinked lane starts collapsed. Stage moves live in Plan. The audit records warnings as an array, and the toast shows only the move summary. Needs you explains what clears each entry. Existing planning reports retain manual handling. Preview keeps the open file tabs and omits the separate close strip.

Context contains the project brief, shared notes, and the managed block with byte count and per-file stale state. Refresh reads the current state. Sync uses the existing publish path and displays its Git exclusion notice. Settings contains Host, Connections, phone pairing, the full Tasks queue, and session history.

## Removed desktop views

| Previous view | Destination |
| --- | --- |
| Shared notes | Context |
| Host | Settings → Host |
| Connections | Settings → Connections |
| Tasks | Settings → Tasks |

The previous route names resolve to those destinations. The terminal-first shell is the desktop navigation. Standalone Specs, Studio, Project files and Overview remain accessible from Settings and the palette. Session history remains reachable from Settings, and the existing agent chat opens from Terminal actions. No dialog was removed.

| Source measure | Before | After |
| --- | ---: | ---: |
| Top-level views | 12 | 9 |
| Dialogs | 28 | 28 |
| JavaScript lines | 11,821 | 11,830 |
| CSS lines | 9,812 | 9,744 |

The counts use unbundled files in `desktop/web/src`. The source diff deletes 189 JavaScript lines and adds 198. It deletes 92 CSS lines and adds 24 for the Board fixes and relocated selectors. CSS consolidation remains in slice 7b.

## Verification

Rust formatting and Clippy passed. The all-targets Rust suite passed 478 tests with five ignored. The pinned-TLS phone control and revocation test passed. JavaScript passed 101 tests. Across local runs, 153 browser journeys passed. The Linux-only stalled-cleanup journey was skipped on macOS. Browser journeys created, renewed and revoked real terminal-scoped phone offers at both widths. A physical handset was not used.

[104 cutover screenshots](../work/001-bench-consolidation/screens/cutover/) cover the changed surfaces at 1440 and 390 in light and dark. The [documentation index](../docs/README.md) links every current document. Forty files moved to history, and the previous index was preserved there.

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-12 · Shailesh Rawat via Codex · created spec and linked Bench work item
- 2026-11-12 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-12 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-12 · Shailesh Rawat via Codex · stage verify → review
