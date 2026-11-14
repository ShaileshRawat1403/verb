# Interrupted Git commit recovery

Validated source work. Deployment state is recorded in the private operations handoff; phone-boundary integration is not implied.

## User experience

Opening the commit dialog checks for an interrupted selected-file transaction. A pending receipt
replaces the new-commit form with **Review interrupted commit**. The UI explains whether the recorded
commit exists and, when its evidence is intact, offers **Repair interrupted Git operation**. Repair is
a separate explicit action. It never replays the message, creates another commit, uploads or deploys.
The new-commit fields are hidden in this state so recovery cannot be mistaken for another commit.

Unknown outcomes, changed branches, newer staging, missing/corrupt evidence, foreign locks or a
possibly active original process require terminal inspection. Opening the dialog never clears locks
or changes Git state. An interrupted repair is not retried automatically.

## Durable transaction receipt

Before the branch compare-and-swap, Verb syncs the prepared index and writes an owner-only,
size-bounded receipt in the worktree's Git metadata directory. It contains the branch, previous and
prepared commit IDs, original/prepared index hashes, the owned lock's filesystem identity, process
identity, private-index basename and optional Spec audit before/after text. It contains no agent
conversation, provider credential or browser profile. It is not a tracked project file.

The receipt remains until index installation and directory synchronization finish. An ambiguous
error preserves it and the verified prepared index for review, rather than deleting the evidence or
reverting an audit line that may already belong to a published local commit.

| Observed state | Explicit repair |
| --- | --- |
| Branch still at the reviewed parent | Clear proven temporary state; restore only an unchanged copy of the attempted Spec audit. Keep other working edits. |
| Branch at the prepared commit; original index remains | Install the verified prepared index under the Git lock. Keep the same commit ID. |
| Branch and index already match the prepared commit | Finish receipt/temporary-file cleanup. Keep the same commit ID. |
| History, workspace root, staging, evidence or lock ownership differs | Refuse repair and leave those facts intact for terminal review. |

Repair validates the receipt version again, serializes with commit/Spec writes, acquires its own
recovery guard, and checks branch/index/lock identities before replacing or deleting anything. Another
process that may still own the transaction is not interrupted. Unknown Git or recovery locks are not
removed automatically. A stale client cannot apply a repair against a newer receipt.

Guided Git operations also require Git's resolved working-tree root to equal the open workspace.
Nested directories or a `core.worktree` configuration pointing elsewhere fail closed.

## Acceptance and limits

Rust tests execute real selected-file commits in child test processes and kill those processes before
ref publication, after ref publication and after index installation. Recovery must retain the observed
commit ID, clear only owned temporary state and preserve new staging, foreign locks and newer Spec
text. Stale and malformed receipts must not alter history or the index. Test-only crash points are
not compiled into the production binary.

Desktop and phone browser fixtures exercise the actual recovery endpoints against a prepared receipt
and a real mismatched Git ref/index. Read-scoped tokens cannot invoke repair. These browser fixtures
do not claim to kill and restart an HTTP host or simulate power loss.

Crashes before the durable receipt is prepared can still leave an unknown Git lock requiring terminal
inspection. Interrupted repairs, corrupt/missing evidence and full power-loss durability remain manual
or separate acceptance work. External phone execution boundaries remain unsupported. Do not infer
production readiness from native tests alone.

## Busy read recovery discovered during acceptance

One phone acceptance case initially received HTTP 503 (`desktop host is busy; retry shortly`) from the
file-list endpoint during concurrent test load. Studio now retries only that rejected GET, at most
twice with short staggered waits. Persistent congestion retains the explicit Refresh action. Other
errors and every write (save, commit, repair, launch) are never automatically replayed. Unit tests
check this bound; a browser test exercises sustained congestion followed by explicit manual refresh.


Final local validation: **443 Rust tests passed (five ignored), 86 web unit tests passed, and all42
Studio/workbench browser tests passed**. Strict clippy, rustfmt, web/native builds and diff checks
passed. The two repair browser cases also passed with a 390px visual capture and a >=44px repair
button assertion. The phone recovery screen was inspected visually.

Logs: `/private/tmp/verb-recovery-final-rust.log`, `/private/tmp/verb-recovery-unit.log`,
`/private/tmp/verb-recovery-final-acceptance.log`, `/private/tmp/verb-recovery-final-clippy.log`,
`/private/tmp/verb-recovery-visual.log`. An earlier concurrent run recorded one file-list 503 timeout;
the bounded read retry and a dedicated persistent-busy/manual-refresh test were added afterward.
No source commit, push, deployment or persistent service restart was performed for this work.
