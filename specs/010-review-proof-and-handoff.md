---
id: 010
title: Review proof and handoff
stage: review
branch: design/bench-consolidation
created: 2026-11-11
work-item: 001-bench-consolidation
---

# 010 · Review proof and handoff

## Problem

The owner needs to see whether the selected spec has proof, review its changes, and hand it off with an explicit scope while the live terminal stays mounted.

## Acceptance criteria

- [x] Lead with failed checks and criterion proof. Clearly mark completion assertions without proof as unproven. (R5)
- [x] Inspect changes since last known good, open a diff, review a commit, and use the existing Git transfer. (R5)
- [x] Record proof and hand off to a second agent with an approved scope. Refuse stale revisions and concurrent submissions. (R5, R12)
- [x] Open and handle Needs you entries with recorded attribution. (R4)
- [x] Simplify Plan and sidebar actions, show one spec terminal action in the empty state, and review draft differences before publication. (R1, R6)

## Review behavior

Review uses the existing SDLC ledger for checks and criterion proof. A completion checkbox without a record displays as Unproven. The project configuration checks are labelled separately from criterion checks. Last-known-good comparison uses the marked Git commit and identifies the tracked diff scope. A mark does not snapshot uncommitted bytes.

The reviewed handoff uses the existing spec revision, assignment role, scope and checks. A stale refusal leaves the modal and its inputs open. Refresh loads the current spec text for a fresh review. A pending request prevents a second submission. Reported outcome, files and checks remain labelled unverified.

Needs you stores the first handling receipt for each displayed kind on the session record. The receipt contains the actor and timestamp. The item stays visible with its receipt, and the outstanding count excludes it. Full clearing rules are reserved for Slice 6.

## Verification

[Bench browser journeys](../desktop/web/e2e/bench-shell.spec.js) exercise an observed failed check, unproven completion, recording and accepting proof, scoped handoff to a second terminal, stale refusal, repeat-submit prevention, diff, commit and transfer. They also cover the Plan menu and sidebar changes, the single spec empty-state action, opening a changed file and recording handling, and comparing and cancelling a replacement draft. [Screenshots](../work/001-bench-consolidation/screens/review/) cover both widths and themes.

Rust formatting and Clippy passed. The all-targets Rust rerun passed 478 tests with 5 ignored. The initial run failed at the existing capability probe's versioned reply assertion. JavaScript passed 97 tests. Bench and project-switcher passed 25 browser journeys. The final build passed all eight Review and Plan journeys. The Connections fixture passed nine and the conversation rerun passed three.

### Prior browser failures

| Test | Local failure | Cause |
| --- | --- | --- |
| Plan distinguishes a document with warnings from a missing document | `Error: expect(locator).toHaveText(expected) failed` | The locator matched whole rows including the newly added draft actions. The rerun checks the document-status portion of each row. |
| spec terminals, context and notes at 1440 in light | `TimeoutError: locator.click: Timeout 12000ms exceeded.` | The initial specs read returned HTTP 503 with `desktop host is busy; retry shortly`. The work tree required both reads. The rerun used the existing bounded read retry for specs. |
| one message id is typed at most once, and a second message cannot overtake the first | `Expected: 2` and `Received: 1` | The test read the fixture transcript after the typed user turn was observed and before the assistant reply was appended. It now waits for the reply before counting both occurrences. |

The prior CI run passed both **external hosts cannot start local Chromium and pairing is owner-only and one-use** and **real pages render with iframe denial while unapproved sites and Verb itself stay blocked** on Linux x86 and ARM. The prior Linux jobs still failed because the selected terminal tab extended past the strip at 390 pixels and because manually created Connections test contexts did not apply the legacy-layout setup. Slice 4 reveals the active tab within the strip and applies that setup to those fixtures. [Linux x86 job](https://github.com/ShaileshRawat1403/verb/actions/runs/43063057591/job/130670510725) and [Linux ARM job](https://github.com/ShaileshRawat1403/verb/actions/runs/43063057591/job/130670510671).

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->
- 2026-11-11 · Shailesh Rawat via Codex · created spec and linked Bench work item
- 2026-11-11 · Shailesh Rawat via Codex · stage plan → build
- 2026-11-11 · Shailesh Rawat via Codex · stage build → verify
- 2026-11-11 · Shailesh Rawat via Codex · stage verify → review
