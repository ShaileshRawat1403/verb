Baseline — 969f5a6, the shared checkout assessed on 2026-10-21.

# Verb production hardening plan

Assessment date: 2026-10-21. Initial target: reliable daily use by one owner on a
self-hosted Linux service, confirmed by the owner. Team use and commercial hosting
are outside this stage.

This is a proposed engineering sequence, not a production certification. It does
not authorize deployment, service restarts, user-session interruption or a new
monitoring automation.

## Baseline and evidence

The accepted pilot source is `f352aa2368247248e22389783a89489dac8cf1a9`.
The shared checkout at assessment was older, at
`969f5a60280da7ebdbc8f3fb6854fd6c99de6963`. Implement from a verified descendant
of the accepted source, preserving the shared checkout and checking remote branch
state before choosing an implementation base.

Existing strengths include real PTYs, durable session identity, bounded HTTP
admission, authentication/origin checks, executable provenance, native execution
separation and guarded rollback. Historical synthetic acceptance is useful but
does not establish sustained daily-use reliability.

The source review found these concrete gaps:

- `desktop/src/exec.rs` uses blocking `Command::output()` to discover repository
  filter configuration before returning a Git command. Its callers can therefore
  block during command construction, before an execution timeout is applied.
- `desktop/src/gitview.rs` also uses blocking output collection, including for
  the external snapshot client. The guest timeout does not itself establish a
  host-side deadline for every transport and output-drain failure.
- `desktop/src/web.rs` holds a background-request permit while deferred work or
  state work completes. Moving work off the PTY loop preserves input service but
  does not bound the lifetime of that work.
- `desktop/src/project_read.rs` already demonstrates bounded reply collection
  and a host-side deadline for selected external project reads. It is a useful
  starting point, not proof that every Git path has the same protection.
- The pilot recorded a transient file-list capacity refusal during overlapping
  reads. The single background execution slot is a deliberate resource limit.
- `SECURITY_MODEL.md` still describes an earlier deployment and pending cutover.
  It needs reconciliation with accepted evidence before it can guide operators.

## Ordered work and acceptance

| Order | Work | Completion evidence |
| --- | --- | --- |
| 1 | Bound automatic read-only Git work end to end | A stalled config probe, stalled Git child, oversized output and inherited open pipe each return within a documented host deadline; cleanup is confirmed or explicitly recorded; permits become reusable; terminal input remains responsive. |
| 2 | Make background contention predictable | Concurrent state/Git/files reads respect the existing resource limit; redundant reads are coalesced where safe; capacity, timeout and failure remain distinct; stale results are labelled; interactive launch retains priority. |
| 3 | Prove session and ledger recovery under failure | Disposable fixtures cover host/controller death, interrupted ledger writes and concurrent recovery attempts; live sessions are never falsely claimed or attached to another conversation; unresolved receipts fail closed and remain available. |
| 4 | Make failures diagnosable without exposing work | Separate liveness from dependency readiness; expose bounded, authenticated counts/timings for admission, timeouts, cleanup, storage and memory pressure; collect no terminal text, prompts, credentials or project contents. |
| 5 | Make release and restore reproducible | Exact-source CI, immutable artifact manifest, browser bundle consistency and offline rollout fault tests pass; current recovery procedures are durable; a disposable restore proves selected work state can be recovered. |
| 6 | Complete normal-use acceptance | Owner completes at least three representative development days; record failed launches, reconnects, read latency, resource pressure and recovery outcomes; review every lost-input, lost-work or incorrect-identity incident before promotion. |

For order 1, start with automatic read-only observations. User-requested Git
mutations require a separate cancellation contract: killing a process is not proof
that its operation made no changes. Preserve existing repository-execution guards,
boundary cleanup authority and unknown-versus-clean semantics. Do not merely time
out the HTTP reply while leaving a child, reader thread or lease behind.

For order 2, cache keys must include project and execution identity, with explicit
freshness and invalidation. Never replay writes or a launch whose admission result
is ambiguous. Do not increase execution capacity to conceal unbounded work.

## Proposed daily-use targets

These are targets to calibrate with measurements, not current guarantees:

- No lost accepted terminal input or cross-session input routing.
- No false native conversation attachment; ambiguous recovery stays unknown.
- Every automatic project read has a documented deadline and bounded output.
- Capacity refusal gives an actionable retry state without making an unknown tree
  appear clean or repeatedly resubmitting work.
- Refresh/reconnect preserves the selected live terminal; service interruption
  explains what ended and what can be resumed.
- Resource exhaustion produces a controlled refusal and recoverable state.
- Readiness failure, rollback refusal and incomplete cleanup identify the failed
  condition without printing private work or requiring receipt deletion.

Choose latency and availability thresholds after collecting representative
four-terminal workloads. Historical small samples and gaps in overnight
reachability cannot support percentile or uptime claims.

## Promotion gates and scope

Run meaningful failure regressions and the repository's required Rust, browser,
Linux-boundary and release checks on the final source. A local pass does not prove
phone behavior. Prepare an explicit reviewed candidate and rollback plan before
requesting a live change; preserve user sessions and the single-writer ledger.

The existing owner-only pilot remains the deployment baseline while this work is
developed. Do not resume overnight monitoring, change host power/network settings
or repeat provider UAT automatically.

For a small team, additionally define ownership, per-user authorization, credential
separation and conflicting-edit behavior. For unrelated customers, require a
separate tenancy threat model and independently enforced execution, storage and
network boundaries; the current shared project/agent account is insufficient
evidence of tenant isolation.

The rationale for the base Git read limits of 15 seconds and 8 MiB is unverified.
