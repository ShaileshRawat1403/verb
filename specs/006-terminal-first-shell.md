---
id: 006
title: Terminal-first Shell
stage: review
branch: design/bench-consolidation
created: 2026-11-11
work-item: 001-bench-consolidation
---

# 006 · Terminal-first Shell

## Problem

Planning, review and terminal views compete for the same workspace. Owners need a terminal-first layout with persistent side panels and a way back to the previous navigation.

## Who it is for

Verb Desktop owners working with CLI agents.

## Acceptance criteria

- [x] Open into three regions on desktop and one surface at a time at 390 pixels. (R4)
- [x] Expand a work item, see spec stages and open a linked spec or its associated terminal. (R4, R6)
- [x] Collapse and restore both panels, remember the right panel width and expose Plan, Review and Preview. (R4)
- [x] Reach the previous views from Settings and switch layouts while retaining the live terminal and draft. (R6, R10)

## Out of scope

Terminal launch and context strip changes, Plan and Review workflows, Preview migration, and the new Board.

## Traceability

- 1 · 9e0d3bebb72320785cf5caac827f7145264847b944efdab69115b8fbc53ed849 · R4
- 2 · e8b03df9393214b0abcfd558b02c96bd31fa4c6fe94557d308c7f515b79db18f · R4, R6
- 3 · 0ae95a2350dce562da99a5dd2539cf9e4c3f8549d83d17ecaa36058d6b3840da · R4
- 4 · 3c23dbde199eb2beeb71c0b3ef1fced5c32fe40824c96357f6d75e60ee14b283 · R6, R10

## Verification

The [Shell browser journeys](../desktop/web/e2e/bench-shell.spec.js) exercised the real Verb host. [Screenshots](../work/001-bench-consolidation/screens/shell/) cover the default Shell, navigation, Plan, Review, Preview, terminal, Board, Needs you and Settings at 1440 and 390 pixels in light and dark. Collapsed desktop configurations were captured in both themes.

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-11 · Shailesh Rawat via Codex · created spec "Terminal-first Shell"
- 2026-11-11T11:35:25Z · Shailesh Rawat via Verb web · linked work item 001-bench-consolidation
- 2026-11-11 · Shailesh Rawat via Codex · stage spec → plan
- 2026-11-11 · Shailesh Rawat via Codex · plan approved in the Bench design; reuse the terminal grid, work item reader and spec-session association
- 2026-11-11 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-11 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-11 · Shailesh Rawat via Codex · verified the three-region shell, work tree, existing Board route, keyboard tabs and desktop width memory in Chromium
- 2026-11-11 · Shailesh Rawat via Codex · verified both mobile overlays and the same live terminal, output and Studio draft through both layout switches at 1440 and 390, in light and dark
- 2026-11-11 · Shailesh Rawat via Codex · formatting, Clippy, 472 Rust tests (5 ignored), 96 web unit tests and 11 browser journeys passed
- 2026-11-11 · Shailesh Rawat via Codex · stage verify → review
