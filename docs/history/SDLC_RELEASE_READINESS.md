# SDLC candidate review and release readiness

Review date: 2026-10-25. This document describes a local candidate, not a deployed
release or current phone-health evidence. Public promotion is not authorized by
running these checks. Keep PocketFabric Node repositories, processes and services
outside this work.

## Review outcome

The candidate adds bounded Git reads, reviewed upload/download operations and a
Spec workbench with separate stage, execution, verification and delivery facts.
Evidence is private host metadata, not a session-schema migration. Existing
Markdown checkmarks are assertions. Observed checks require an explicit action;
acceptance and review remain explicit decisions. Historical results survive code
changes but lose current acceptance. Health reports bind to a specific deployment.

Reviewed boundaries: authenticated API routing, strict evidence requests,
optimistic Spec/ledger/source guards, storage bounds and atomic writes, credential
removal for check children, bounded discarded output, Git destination/commit
pinning, fail-closed reads, evidence provenance and keyboard/touch alternatives.
No additional blocker was identified in this review. This is a local engineering
review, not an independent security audit or phone UAT.

One review finding was corrected before committing: individual unreadable or
non-UTF-8 Spec files previously disappeared from a partial list. Checked listings
now return an explicit error, and creation refuses ID allocation from that
incomplete list. Rust and real-host browser regressions verify preservation of
the unreadable file and the visible error state.

The initial browser run also exposed a fixture teardown failure: a polling
`route.fetch` remained active while its context closed, restarting the shared
fixture and causing dependent Spec tests to fail. The supervisor scenario now
fetches one real report before interception and fulfils polling from that report.
This corrects test lifecycle handling without changing production host reporting.

Known limits that remain release considerations:

- Test-task success needs a person to judge whether it proves a criterion.
- Git-visible source fingerprints do not cover ignored dependencies or external
  environments, and do not lock out external writers during a check.
- Provider CI/release/deployment receipts are not integrated. Delivery and health
  entries remain manual reports, and remote state is the last explicit check.
- Local integration checks use local main/master; they do not establish remote
  merge approval, protected-branch policy or production deployment.
- Writer roles/worktree advice do not enforce exclusive write permissions.
- Slow or blocked operations can still occupy bounded background workers. A test
  check may take three minutes. Boundary cleanup can remain unconfirmed.

## Build and verification

Use the existing feature checkout, not an older shared integration checkout.
Record the exact clean commit and binary SHA256 after committing. Rebuild embedded
assets before the Rust release binary:

```sh
npm test --prefix desktop/web
npm run build --prefix desktop/web
cargo fmt --manifest-path desktop/Cargo.toml --check
cargo clippy --manifest-path desktop/Cargo.toml --all-targets --offline -- -D warnings
cargo test --manifest-path desktop/Cargo.toml --all-targets --offline
python3 desktop/scripts/test-workstate.py
cargo build --release --manifest-path desktop/Cargo.toml --offline
cargo build --manifest-path desktop/Cargo.toml --offline
VERB_BIN="$PWD/desktop/target/debug/verb" npm run e2e --prefix desktop/web
```

A clean-checkout CI run at the resulting commit remains required before public
promotion. Local Mac evidence does not establish the production Linux boundary
or device behaviour. Use disposable synthetic-boundary/Linux tests where the
runtime is available, without modifying a node checkout or installing on a phone.

## Isolated preview

Validation on 2026-10-25: formatting and Clippy passed, all 401 native Rust tests
passed (five intentional ignores), 62 browser unit tests and 10 backup checks
passed, and the optimized binary rebuilt successfully. The full browser attempt
passed the independent Git/SDLC/Spec journeys, including the new unreadable-file
case, but had the fixture failures described above. After the harness correction,
all 21 workbench journeys passed on a complete file rerun: 31 distinct browser
journeys passed across those runs; one Linux-only journey is skipped on Mac.
No fresh Linux/device or remote CI claim follows from these local checks.

Use a disposable clone/archive of the candidate, its own HOME and VERB_STATE_DIR,
loopback binding and a dedicated port. Never point a second Verb host at a live
ledger. Do not inherit a real project remote or provider login. Populate the
preview with genuine Verb requirements while retaining truthful manual-vs-observed
labels. Local browser and API exercise is not a deployment trial.

The operator's private preview receipt records its source commit, URL, checkout,
state path, PID and stop instructions. Such process handles belong in private
operator receipts only, never in product session records or evidence exports.

## Promotion preflight — later, after explicit approval

1. Finish the preview trial, resolve findings and obtain exact-commit CI evidence.
2. Agree the target service, maintenance window and rollback acceptance criteria.
3. Read the retained owner-only deployment handoff; its October 20 service details
   are historical. Re-observe current binary, launcher, ledger ownership, active
   sessions and recovery receipts before constructing a concrete transaction.
4. Preserve user sessions. Any interruption or closure requires explicit scope.
5. Take and verify a private backup including `sdlc/`, without printing credentials
   or copying live state into Git. Rehearse restoration to a disposable location.
6. Build for the target architecture. Record source, binary and launcher hashes.
   Stage immutable artifacts and verify execute access as the actual service user
   before stopping anything. Preserve the accepted executable and launch config.
7. Review the exact promotion and rollback transaction. Do not start a second host
   against the public ledger. No Node repository, controller, service or policy
   change is part of this feature's rollout.
8. Request owner approval for that concrete transaction. This document itself
   executes nothing and supplies no blanket deployment permission.

## Post-promotion acceptance — later

Check actual authenticated browser access, existing-session reattachment, a new
owned terminal, Spec list/board, evidence provenance/staleness, read errors and
host capacity. Test only disposable work; inspect real target health separately.
A selected Ship stage or a manual report is never deployment-health proof.
Retain the preflight and acceptance receipt without raw terminal content.

## Rollback — prepare against observed target state

Use the retained reviewed deployment transaction only after its guards and
artifacts have been revalidated. Do not copy a historical rollback command from
memory or bypass a guard. Confirm active user work and cleanup/recovery state.
Restore the prior immutable binary/launcher and verify the actual service user
can execute it; keep one host writer. Preserve newer workspace and ledger data.
The older binary may ignore `sdlc/`; that does not justify deleting it or restoring
an old full backup over newer work. Validate service access and session continuity,
record the outcome, and leave unrelated services alone. Destructive data recovery
needs a separate reviewed plan.
