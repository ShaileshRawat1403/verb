---
id: 004
title: Publish shared and per-spec context
stage: review
branch: spec/004-publish-shared-and-per-spec-context
created: 2026-11-11
work-item: 001-bench-consolidation
---

# 004 · Publish shared and per-spec context

## Problem

Verb needs a bounded checkout projection and a CLI read for spec context. Isolated sessions cannot rely on access to the host ledger.

## Who it is for

Verb owners and supported hosted CLI agents.

## Acceptance criteria

Requirement IDs refer to the [Bench consolidation PRD](../work/001-bench-consolidation/prd.md).

- [x] Publish matching, bounded blocks with explicit omissions and preserve surrounding file content. (R7, R8, R11)
- [x] Refuse ambiguous markers and expose stale or failed projection state. (R8, R11)
- [x] Provide plain-text full and per-spec reads, and a checkout-only read that cannot use the private ledger. (R9)

## Out of scope

New desktop layout, terminal context receipts, and changes to separate-account access permissions.

## Delivery contract

- `verb shared full` prints the complete shared projection. `verb shared spec <spec-id>` prints the selected spec and its currently linked requirement and goal rows.
- `verb shared read --checkout` reads matching published blocks without a private-ledger receipt. The original `verb shared read` keeps its current receipt semantics.
- The owner can explicitly sync an actual session checkout using `POST /api/hub/sync?session=<id>`. Verb verifies that the session checkout belongs to the current project.
- Ordinary spec launchers with an opening argument receive a bounded projection from the actual checkout, subject to the existing 4096-byte argument limit. Reviewed handoffs keep their approved scope and resumed conversations keep their existing launch behavior.
- OpenCode, shell and custom commands have no verified automatic prompt path here. Separate-account CLI availability and native instruction-file discovery remain unverified. Legacy free-form notes have paragraph boundaries only.

## Traceability

- 1 · 2e7cabf2a730271cae47636615112d5499e77493be7e42b66097091d2d15254c · R7, R8, R11
- 2 · 2e3a296761a610ef3b078c3e2ec82d1f412a631f3365cc9f0239e99447948229 · R8, R11
- 3 · e74628a640b97ef957b5bcbe9a85ed957c0173f4032f18ab50249684cad1cdf7 · R9

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-11 · Shailesh Rawat via Codex · created spec "Publish shared and per-spec context"
- 2026-11-11 · Shailesh Rawat via Codex · stage spec → plan
- 2026-11-11 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-11 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-11 · Shailesh Rawat via Codex · verified UTF-8 bounds, omissions, marker preflight, preserved owner text, visible spec context, captured launch prompt and explicit isolated worktree sync
- 2026-11-11 · Shailesh Rawat via Codex · formatting, Clippy, 468 Rust tests, 94 web unit tests and three existing browser journeys passed. Five Rust tests were ignored.
- 2026-11-11 · Shailesh Rawat via Codex · stage verify → review
- 2026-11-11T11:08:43Z · Shailesh Rawat via Verb web · linked work item 001-bench-consolidation
