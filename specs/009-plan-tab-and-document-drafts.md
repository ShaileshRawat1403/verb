---
id: 009
title: Plan tab and document drafts
stage: review
branch: design/bench-consolidation
created: 2026-11-11
work-item: 001-bench-consolidation
---

# 009 · Plan tab and document drafts

## Problem

The owner needs to follow a criterion to its requirement and goal, see its spec's tasks, and review an agent's BRD or PRD draft before publishing.

## Acceptance criteria

- [x] Show document states, criterion links and proof, and tasks scoped to the selected spec. (R1, R2, R3, R6)
- [x] Start BRD and PRD drafts using the approved planning wording, reserve new work item numbers, and let the owner review and publish through Plan. (R1, R5, R6)
- [x] Keep drafts out of context and validation, exclude them locally in Git, and surface changes outside the named draft when a planning session ends. (R5, R6, R10)
- [x] Deliver one spec heading with project-root paths and make spec launch controls follow agent, spec, and role. (R6)

## Draft lifecycle

Verb reserves the work item number in the existing project allocator and stores the draft's published-document baseline in the private project store. The agent writes the single requested `brd.draft.md` or `prd.draft.md` in the active checkout. The owner reviews its contents and current published revision in Plan. Successful publication uses the existing document validator and audit writer, then removes the reviewed draft. A changed draft must be reviewed again. If cleanup fails, Verb reports that the published document was saved and the draft remains.

The Plan role uses the approved wording and a dedicated draft launch. Its one-file instruction applies to the agent's behavior. Git status observations are attached to the session and surface paths changed outside the requested draft in Needs you. The comparison does not attribute a change to a particular actor when several processes share the checkout.

## Verification

- [Rust integration](../desktop/tests/web_integration.rs) exercises both draft prompts with a local CLI fixture, a new draft work item, a draft beside a published BRD, stale review, successful cleanup, draft exclusion from context and validation, and a Git change outside the draft.
- [Browser journeys](../desktop/web/e2e/bench-shell.spec.js) exercise criterion-to-goal tracing, spec tasks, a spec-bound shell, legacy unlinked work, and drafting and approving a small BRD. [Screenshots](../work/001-bench-consolidation/screens/plan/) show Plan, launch, new spec task, the legacy unlinked view, the draft launch form, a draft work item, draft review, a published small item and Needs you at 1440 and 390 pixels in light and dark.

Local checks passed with 478 Rust tests (5 ignored), 96 JavaScript tests, and 21 Bench and project-switcher browser journeys. The final Plan build passed five targeted browser cases. Drafting behavior was exercised with CLI fixtures. Live agent compliance with the planning wording is unverified.

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-11 · Shailesh Rawat via Codex · created spec and linked Bench work item
- 2026-11-11 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-11 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-11 · Shailesh Rawat via Codex · stage verify → review
