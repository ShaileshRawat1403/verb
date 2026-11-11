---
id: 005
title: Open another authorized project workspace
stage: review
branch: spec/005-open-another-project-workspace
created: 2026-11-11
work-item: 001-bench-consolidation
---

# 005 · Open another authorized project workspace

## Problem

The web host owns one project's terminals. Replacing its page to switch projects would discard browser state, including unsaved drafts.

## Who it is for

Verb project owners working with more than one running Verb workspace.

## Acceptance criteria

Requirement IDs refer to the [Bench consolidation PRD](../work/001-bench-consolidation/prd.md).

- [x] An owner can open another running Verb workspace using its own URL. (R4)
- [x] Opening the destination preserves the source terminal and browser draft. (R6)
- [x] The destination authenticates independently and receives no source credential or browser opener. (R6, R10)

## Out of scope

Starting hosts, listing local project paths, sharing credentials, or moving a terminal between projects.

## Traceability

- 1 · 9856714ebcffbaaeb6ccbc2511645cc8e1e8f4174f01aa769e01adda7844e289 · R4
- 2 · f2cee0e47770c9e4cc2f8a04f419f104e6217be47f69b6f4cc91db8cd4eac6d7 · R6
- 3 · 8b3a3b14cbf22387be733eec7b3f93331df6123b0c71b9bb7916c22b7b66cb5b · R6, R10

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-11 · Shailesh Rawat via Codex · created spec "Open another authorized project workspace"
- 2026-11-11 · Shailesh Rawat via Codex · stage spec → plan
- 2026-11-11 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-11 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-11 · Shailesh Rawat via Codex · verified separate destination credentials and browser context, source terminal identity and retained draft at 1440 and 390
- 2026-11-11 · Shailesh Rawat via Codex · formatting, Clippy, 468 Rust tests, 96 web unit tests and seven browser journeys passed. Five Rust tests were ignored. Captured both screens at both widths in light and dark.
- 2026-11-11 · Shailesh Rawat via Codex · stage verify → review
- 2026-11-11T11:08:43Z · Shailesh Rawat via Verb web · linked work item 001-bench-consolidation
