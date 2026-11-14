---
id: 007
title: Polish the terminal-first Shell
stage: review
branch: design/bench-consolidation
created: 2026-11-11
work-item: 001-bench-consolidation
---

# 007 · Polish the terminal-first Shell

## Problem

The new Shell repeats terminal controls and displays implementation details in the terminal footer. The Plan panel needs to show the state of each work item document and open it directly.

## Acceptance criteria

- [x] Use one compact toolbar at desktop and phone widths, with secondary commands available from the palette or Settings. (R4)
- [x] Use one terminal tab row with Search and End, and show connection text only for interruptions. (R6)
- [x] Display clean work item names and open BRD and PRD from their document states in Plan. (R4, R6)
- [x] Republish the Bench BRD and PRD with all required sections and no validation warnings. (R1, R3, R11)

## Traceability

- 1 · bacba350dd4383bcea4ace08557ad7b34171072d31275a7de162b51b33b921dd · R4
- 2 · 6ab3e758b3976b9765136f5d967b16fd72a62def7e7ebb5b798b628109da8dc4 · R6
- 3 · 2ad62ebf4e973342188ac0aa4a2ff268154679760b30fd9f8347ca4e2e076fd7 · R4, R6
- 4 · e13d1abdd040bdb00495883878a71d97a249243a65b4f07ea92018204cb9a007 · R1, R3, R11

## Verification

[Before and after screenshots](../work/001-bench-consolidation/screens/shell-polish/) include the top bar and running terminal at 1440 and 390 pixels, in light and dark. The after set also includes the work tree, Plan, Review, Preview, Board, Needs you, Settings and collapsed desktop states.

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-11 · Shailesh Rawat via Codex · created spec and approved Shell follow-up scope
- 2026-11-11 · Shailesh Rawat via Codex · stage spec → plan
- 2026-11-11 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-11T14:46:13Z · Shailesh Rawat via Verb web · linked work item 001-bench-consolidation
- 2026-11-11 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-11 · Shailesh Rawat via Codex · verified toolbar controls, one-row mobile layout, terminal menu and connection text, layout continuity, and Plan document previews and states in Chromium
- 2026-11-11 · Shailesh Rawat via Codex · both document publishes and the work item readback returned zero warnings
- 2026-11-11 · Shailesh Rawat via Codex · formatting, Clippy, 472 Rust tests (5 ignored), 96 web unit tests and 13 Chromium checks passed
- 2026-11-11 · Shailesh Rawat via Codex · stage verify → review
