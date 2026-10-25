# Spec SDLC workbench

Specs have six workspaces: Spec, Plan, Build, Verify, Review and Ship. The searchable
list and Kanban board project the same backend facts. Chosen stage, session state,
verification and delivery remain independent; moving a card never creates proof.

## Daily workflow

1. Open Specs, find work by title, ID, branch or stage, and use Needs you only to
   focus on review requests, failures and stale evidence. Choose List or Board.
2. Open a Spec to reach its terminal/session actions and focused workspaces.
   Back to work overview restores the list and keyboard focus to search.
3. In Plan, record the approach, risks, verification strategy and completion goal:
   integrated code, a release artifact, or a hosted service. Artifact and service
   goals name the required delivery target.
4. In Build, open the terminal, switch to the Spec branch or use reviewed Git
   upload/download controls. Save implementation changes to Git before verification.
   Sessions and checkout changes do not establish exclusive ownership.
5. In Verify, explicitly run a supported project check or link a person/agent
   report to a criterion. Passing evidence awaits explicit acceptance and rationale.
   Result, source and freshness are separate. Legacy checkboxes remain assertions.
6. In Review, inspect the bounded committed implementation comparison and record
   approval or requested changes. Approval requires current accepted evidence for
   every criterion and is bound to that evidence set and implementation intent.
7. In Ship, explicitly check the linked remote and record release/deployment/health
   attestations. Service health belongs to a specific deployment and target.
   A newer deployment needs fresh health; reported delivery failures need attention.
   Neither card movement nor an attestation deploys or restarts anything.

Drag a desktop board card to open the reviewed transition dialog, or use the
labelled Target stage selector and Review move with touch or keyboard. The dialog
shows skipped stages and incomplete readiness. Changed Spec/source/evidence
rejects a stale reviewed move; external edits are preserved. Legacy callers may
omit the optional context guard. This is optimistic concurrency, not a filesystem
lock against arbitrary editors.

## Evidence and limits

See [the evidence contract](SDLC_EVIDENCE_CONTRACT.md). Evidence is stored privately
under the existing host state root, outside the checkout. The history is
append-only, bounded and exported from the UI; workstate backups include `sdlc/`.
Damaged/unsupported records block writes and are never silently replaced.

Checks use fixed Rust, Node or Python choices through the existing execution
boundary, with explicit consent, a three-minute limit and bounded discarded
output. No arbitrary-command API or durable terminal output is introduced. Verb
credentials are removed from the child environment. Python zero-test discovery is
inconclusive. Rust/Node project tasks can themselves provide weak checks, so task
success never automatically proves a criterion.

Freshness tracks committed Git-visible code plus semantic Spec intent and plan.
Stage/checkmark/audit edits preserve proof. Dirty or changed implementation and
changed criteria/plan invalidate current acceptance/review; historical results
remain visible. Ignored dependencies, external systems and code changing away
and back during a check are not covered by a source fingerprint. There is no
enforced writer lease or automatic CI receipt ingestion in this implementation.
Focused evidence details refresh when the workspace observes changed source or
ledger context. Refresh evidence also requests a fresh observation explicitly.

Remote state reflects the last explicit check. Local main/master integration is
labelled local. Artifact, deployment and health reports remain person-reported;
provider-authenticated release receipts and continuous health probes are not
configured. Completion means the selected conditions are recorded, not that Verb
independently observed a production deployment.

Execution projections share the existing Rust resolver across APIs. Observer
waiting hints are volatile and labelled agent-reported. Unknown, expired or
future-dated execution observations do not become live requests for attention.
Ambiguous session prefixes remain unassociated. Read failures show an error
instead of a false empty project.

## Validation

Disposable-host browser journeys cover the entire plan → real Python test →
acceptance → diff review → remote observation → deployment/health workflow,
source changes, failed checks, deployment-health binding, delivery attention,
forged observations, forbidden commands, stale requests and damaged storage.
Board journeys cover reviewed drag moves, keyboard alternatives, cancellation,
external edit preservation, filters and retained focus during polling.

Phone checks at 390 pixels cover no page overflow, 44-pixel controls, focused
navigation and reduced motion. Board secondary text meets 4.5:1 contrast in both
themes; screenshots are visually inspected. Rust regressions cover independent
results/freshness, evidence supersession, intent invalidation and delivery binding.
Private evidence backup/restore is covered by the workstate fixture.

Validated on 2026-10-21: 400 native Rust tests passed (five intentional ignores),
62 browser unit tests passed, 30 browser journeys passed (one Linux-only skip on
Mac), and 10 workstate backup checks passed. The five new SDLC invariants also
passed on ARM64 GNU Linux in Docker. Formatting, diff whitespace checks, local
Clippy, pinned Rust 1.99 Clippy with the synthetic-boundary feature, and the final
optimized release build passed. Embedded assets were rebuilt from source.

The candidate is developed and validated locally. Read Git for its current commit
and working-tree state. No production remote, deployment, service restart or phone
mutation is used by these tests.
Exact-source CI and separate deployment review remain promotion gates.
