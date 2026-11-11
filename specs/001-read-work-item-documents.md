---
id: 001
title: Read and validate work item documents
stage: review
branch: spec/001-read-work-item-documents
created: 2026-11-11
---

# 001 · Read and validate work item documents

## Problem

Verb needs to read work item BRDs and PRDs and explain invalid or missing links before it can publish work items.

## Who it is for

Verb project owners and the later CLI context and work item publishing slices.

## Acceptance criteria

Requirement IDs refer to the [Bench consolidation PRD](../work/bench-consolidation/prd.md).

- [x] Read standard, small and draft work items and report document validation warnings without writing project files. (R1, R2)
- [x] Resolve goal and requirement IDs, report duplicates and dangling or retired references, and detect stale legacy criterion links. (R3)
- [x] Preserve existing spec reads and report specs without a work item as unlinked. (R3, R10)

## Out of scope

Publishing, persistent warning writes, ID allocation, CLI context, Board, Bench, navigation and migration. This spec is a development record for the reader itself.

## Reader format

The reader accepts numbered slug directories such as `work/001-example/`.
BRD goal rows begin with `G1` and PRD requirement rows use
`R1 · G1 · Requirement text · Verification`. Markdown table rows can use the
same columns. Add ` (retired)` to a goal or requirement ID to retain it as
retired. A small BRD can use `Goal: G1 · Goal text` for its goal line.

Linked specs use the optional `work-item: 001-example` front-matter field.
Legacy mapping rows use `- 1 · <criterion identity> · R1, R2` under
`## Traceability`. The number is one-based. Verb compares the recorded
identity to the existing SDLC criterion key. A stale association stays visible.

The read-only `GET /api/work-items` response returns items, document content,
goal and requirement IDs, spec links and warnings. Missing PRDs are allowed for
drafts. Warnings are returned with the read and can be recorded by the later
publishing slice. The reader does not write them to disk.

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-11 · Shailesh Rawat via Codex · created spec "Read and validate work item documents"
- 2026-11-11 · Shailesh Rawat via Codex · stage spec → plan
- 2026-11-11 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-11 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-11 · Shailesh Rawat via Codex · verified criteria 1–3 with the HTTP work item regression and legacy spec read checks
- 2026-11-11 · Shailesh Rawat via Codex · cargo test --all-targets, cargo fmt --check, cargo clippy --all-targets -- -D warnings, and npm test --prefix web passed. Clippy and the HTTP regression were rerun after simplifying two equivalent predicates.
- 2026-11-11 · Shailesh Rawat via Codex · stage verify → review
