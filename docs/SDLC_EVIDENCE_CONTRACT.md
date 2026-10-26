# SDLC evidence contract v1

Implements the user's reviewed SDLC direction: explicit acceptance, independent
result/freshness/provenance, revision-bound reviews and defined completion goals.
This proposal precedes the implementation. Existing session/task schemas remain
unchanged; existing Spec checkboxes remain assertions, not migrated test results.

New append-only records live under the existing host state root in `sdlc/v1/`,
keyed by resolved project identity and Spec ID. They contain user-authored plan
and report text, criterion identifiers, source fingerprints, commit/branch,
actor/time, check identifier, exit code and duration. They never contain shell
command text, terminal output, prompts, transcripts, credentials or PIDs. Records
are private, bounded, atomically replaced under a project lock and versioned.
Repository text cannot create a host-observed test result. This is not a security
boundary against another process with the same host-account permissions.

Evidence runs use fixed, explicitly selected checks, through the existing
execution boundary. No arbitrary shell endpoint or automatic execution is added.
Source must be committed; Spec Markdown edits are excluded from source dirtiness.
Freshness binds Git-visible source and the Spec's semantic intent/criterion,
rather than audit, checkbox or stage edits. Ignored files and external systems
remain outside this source fingerprint. Before/after snapshots detect changed
source; they do not lock out external writers while a check executes.

A passing check is evidence awaiting explicit acceptance. Agent and manual
reports remain labelled reports. Failed, stale and unknown results cannot be
accepted. Approval requires accepted current evidence for every criterion.
Changing source, criterion definitions or plan makes prior acceptance/review
historical. Failed checks stay failed even when their freshness becomes stale.

Completion targets distinguish code, release artifacts and hosted services.
Remote Git observations require the user's explicit Check remote action. Artifact,
deployment and health reports are attestations with target/reference; they never
masquerade as independent remote observation or trigger a deployment. The board
projects these facts; moving a card does not manufacture proof or release state.
Health references the specific successful deployment record at the same target.
A later deployment requires fresh health evidence, and the latest failed required
delivery report creates an attention item. Python discovery with zero tests is
inconclusive; Rust and Node observations describe the selected project test task,
whose exit status alone never automatically accepts a criterion.

Host backup scope must include `sdlc/`; metadata export is available in the UI.
Automatic CI/provider ingestion and live deployment/health integrations require
their own configured, authenticated receipt sources. No live service is changed
by building or testing this workflow.
