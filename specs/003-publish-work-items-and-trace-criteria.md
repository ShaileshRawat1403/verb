---
id: 003
title: Publish work items and trace criteria
stage: review
branch: spec/003-publish-work-items-and-trace-criteria
created: 2026-11-11
work-item: 001-bench-consolidation
---

# 003 · Publish work items and trace criteria

## Problem

The reader can show work item documents and links. Owners need to publish documents and associate existing specs without disturbing evidence.

## Who it is for

Verb project owners and the later Bench publication interface.

## Acceptance criteria

Requirement IDs refer to the [Bench consolidation PRD](../work/001-bench-consolidation/prd.md).

- [x] Reserve an ID under the project lock and publish complete Markdown without reusing reserved numbers. (R1, R2, R11)
- [x] Reject stale document replacement and record validation warnings with the published document. (R2, R11)
- [x] Link existing criteria using their existing identities and preserve criterion text and audit history. (R3, R10, R11)

## Out of scope

Context generation, terminal launch, layout and task association.

## Publication contract

The owner-only `POST /api/work-items` publishes one document at a time. An
initial BRD takes a title, document kind and Markdown content. Later
replacements take the work item ID and the current document revision returned
by GET. Publishing a first PRD requires a readable BRD. Validation warnings
are returned and appended to the managed audit section of the published file.

Verb reserves numbers under the existing project lock in the shared project
store. Legacy checkout `work/.next-id` remains readable and unchanged.
An isolated checkout requires access to the initialized main project
allocation store. Failed publications may leave gaps.

The owner-only `POST /api/work-items/link` takes the work item ID, spec ID,
expected spec revision, and a map of one-based criterion numbers to
requirement IDs. Omitted rows stay in place. Use `removeCriteria` with criterion
numbers to remove legacy mapping rows. The write retains criterion text and records the association
and warnings in the spec audit.

## Traceability

- 1 · 0b089583e4c87f5e3adc8bed19bed2b1c3d50a596b1f7c1f710eff99aaa886c6 · R1, R2, R11
- 2 · 8d22abcd9ef3f0288693d23ffa25dc2c2bf853c32f4da4da4265ff5f88fac793 · R2, R11
- 3 · 8a2363e3739752edf7836c68d711f64f4ad63cfb58ab90f12cbd33c0cba15460 · R3, R10, R11

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-11 · Shailesh Rawat via Codex · created spec "Publish work items and trace criteria"
- 2026-11-11 · Shailesh Rawat via Codex · stage spec → plan
- 2026-11-11 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-11 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-11 · Shailesh Rawat via Codex · HTTP regression verified concurrent ID allocation, stale publication refusal, retired-reference audit and unchanged legacy criterion identity
- 2026-11-11 · Shailesh Rawat via Codex · formatting, Clippy, 465 Rust tests, 94 web tests and two existing browser smoke journeys passed. Five Rust tests were ignored.
- 2026-11-11 · Shailesh Rawat via Codex · stage verify → review
- 2026-11-11T11:08:43Z · Shailesh Rawat via Verb web · linked work item 001-bench-consolidation
