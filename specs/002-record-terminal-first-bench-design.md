---
id: 002
title: Record the terminal-first Bench design
stage: review
branch: spec/002-record-terminal-first-bench-design
created: 2026-11-11
work-item: 001-bench-consolidation
---

# 002 · Record the terminal-first Bench design

## Problem

The approved desktop layout needs a durable reference before data and interface work resume.

## Who it is for

The Verb owner and the agents implementing Bench slices.

## Acceptance criteria

- [x] Record the approved layout, surface destinations and source gaps. (R4, R6)
- [x] Trace the context strip and session note to the PRD and sequence the implementation slices. (R13, R14)
- [x] Record DAX in the backlog and explain the agent registration path. (R6)

## Out of scope

Product code, launch behavior, screen rendering and data publication.

## Work item

[Bench consolidation PRD](../work/001-bench-consolidation/prd.md).

## Traceability

- 1 · ad3abd231d531648e07a36e98f1872bda7e910c60323bcdb3a5ce61dbeff7a83 · R4, R6
- 2 · 74d2016126419df089e9bbfa85521616398b3b4eec9fff21fc600e3f5eb2918c · R13, R14
- 3 · fcae7f395eae42fe2b01b869db6ec7af3b1b8c18e729448a53f5565e70c6c13f · R6

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-11 · Shailesh Rawat via Codex · created spec "Record the terminal-first Bench design"
- 2026-11-11 · Shailesh Rawat via Codex · stage spec → plan
- 2026-11-11 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-11 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-11 · Shailesh Rawat via Codex · verified 9 view and 20 dialog destinations, 14 requirement references and 8 layout slices
- 2026-11-11 · Shailesh Rawat via Codex · cargo fmt, Clippy, 464 Rust tests, 94 web unit tests and 2 existing browser smoke journeys passed. Five Rust tests were ignored.
- 2026-11-11 · Shailesh Rawat via Codex · stage verify → review
- 2026-11-11T11:08:43Z · Shailesh Rawat via Verb web · linked work item 001-bench-consolidation
