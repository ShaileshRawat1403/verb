# Public preview release checklist

This is a gate, not a claim that Verb is finished. A checked item must name the evidence that makes
it true. Automated, emulator and physical-device results remain separate.

## Source and legal

- [x] Root Apache-2.0 license for Verb-authored code.
- [x] Third-party runtime licenses and corresponding-source locations documented.
- [x] Contributor and security-reporting guidance present.
- [x] Integrate the vetted source transfer into the primary repository that owns the real Git
  history; no temporary transfer-repository history is imported.
- [ ] Review the primary repository history and final archive for secrets, credentials, agent state,
  transcripts, keystores and generated build output.

## Automated verification

- [ ] Desktop format, clippy with warnings denied, all-target tests and release build pass on the
  exact release commit.
- [ ] Both Android flavors pass unit tests, lint and debug assembly on the exact release commit.
- [ ] CI's emulator-only `connectedFullCliDebugAndroidTest` passes on the exact release commit.
  Never run this task on a phone containing a real Working World: instrumentation deployment may
  uninstall the target package during cleanup and erase app-private data.
- [x] Working World and continuity planted-marker/privacy tests pass.

## Manual desktop acceptance

- [x] Repository, clean Git, dirty Git and non-Git project facts are truthful.
- [x] Narrow and `NO_COLOR` layouts remain readable.
- [x] Redirected/non-interactive invocation does not enter the TUI.
- [x] Shell, Claude and Codex sessions preserve terminal ownership and restore terminal state.
- [x] An interrupted agent is called recoverable only after positive local evidence and resumes the
  same Verb/agent identity.
- [x] JSON status, session and context output parses and agrees with the TUI.

## Physical Android acceptance

- [x] Terminal workspace starts without blocking; the Verb sheet and all named tasks are reachable
  on the primary Vivo device.
- [x] Opening a Verb surface removes terminal input ownership; back resolves keyboard → contextual
  surface → task → sheet → terminal without losing the PTY.
- [x] Claude and Codex start, exit, reconcile and resume the same conversation identity in a new
  process. Saved-login presence is not presented as verified authentication.
- [x] A real encrypted schema-v1 Working World previews and applies through the current v2 reader,
  preserves allowlisted agent/session state across an in-place non-debuggable upgrade, and does not
  claim to contain project source.
- [ ] A newly exported schema-v2 Working World completes its own physical export/preview/apply
  round-trip.
- [ ] Android → desktop → Android `.vcont` export/preview/apply works through the real file pickers;
  imported state remains dated, read-only evidence and never creates a Resume action by itself.
- [x] OpenCode 1.18.21 installs, launches its real TUI and exits to the shell physically; recovery
  is clearly labelled unverified/experimental.

## Packaging and publication

- [x] Version and release notes describe a developer preview, not a finished product.
- [ ] Signed Android artifact and checksum are produced by the release workflow.
- [x] Desktop installation is documented; absence of prebuilt desktop binaries is explicit.
- [x] Return/public archive contains only source, tests and documentation and excludes `.git`, local
  configuration, caches, targets, build output, keystores, agent state and temporary directories.
