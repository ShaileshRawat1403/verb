# Public preview release checklist

This is a gate, not a claim that Verb is finished. A checked item must name the evidence that makes
it true. Automated, emulator and physical-device results remain separate.

## Source and legal

- [x] Root Apache-2.0 license for Verb-authored code.
- [x] Third-party runtime licenses and corresponding-source locations documented.
- [x] Contributor and security-reporting guidance present.
- [x] Integrate the vetted source transfer into the primary repository that owns the real Git
  history; no temporary transfer-repository history is imported.
- [x] Review the primary repository history and final source for secrets, credentials, agent state,
  transcripts, keystores and generated build output. The staged source scan for PR #4 found none.

## Automated verification

- [x] Desktop format, clippy with warnings denied, all-target tests and release build pass in CI
  (PR #4, run 32732143051).
- [x] Both Android flavors pass unit tests, lint and debug assembly in CI
  (PR #4, run 32732143051).
- [x] CI's emulator-only `connectedFullCliDebugAndroidTest` passes
  (PR #4, run 32732143051).
  Debug instrumentation targets the disposable `com.aistudio.verb.app.debug` package, never the
  canonical release package containing the user's Working World. The task fails before deployment
  if ADB reports any physical device.
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
- [x] A newly exported schema-v2 Working World completes its own physical export/preview/apply
  round-trip. Completed 26 August on the Vivo I2202 — and it failed the first time, which is why
  this box was worth having: `verb export` wrote an archive `verb import` refused. See
  `docs/WORKING_WORLD.md`, "The export that its own importer refused".
- [x] Android → desktop → Android `.vcont` export/preview/apply works through the real file pickers;
  imported state remains dated, read-only evidence and never creates a Resume action by itself.
  Completed 26 August on the Vivo I2202, both directions — see `docs/BACKLOG.md` G2. The round-trip
  found and fixed two desktop defects on the way (`a37bc7c`), which is the evidence that it really
  ran rather than being asserted.
- [x] OpenCode 1.18.21 installs, launches its real TUI and exits to the shell physically; recovery
  is clearly labelled unverified/experimental.
- [x] Multi-terminal concurrent isolation and reattachment verified on physical Vivo I2202 device:
  $T_1$ running Claude Code v2.1.250, $T_2$ running OpenAI Codex CLI, $T_3$ running interactive shell;
  tab switching causes zero state mutations to $T_1$ or $T_2$, background command execution in $T_3$ does
  not mutate agent state, interrupting Codex in $T_2$ via `^C` transitions only $T_2$ while leaving Claude in $T_1$
  running, and Activity re-creation cleanly reattaches sessions with intact state.

### beta.8, unaccepted

Every box below is open. `docs/BETA8_HANDOFF.md` carries the procedure for each, including which
observation would falsify the claim rather than confirm it.

- [ ] The changes compile and the unit suite passes. Nothing in this cycle has been compiled.
- [ ] `assembleFullCliDevice` installs over the existing app with `adb install -r`, and the working
  world -- projects, runtime, and the Claude, Codex and Antigravity sign-ins -- is intact afterwards.
- [ ] System → Device Information reports `0.1.0-beta.8 (8)` and `com.aistudio.verb.app`.
- [ ] The workspace line names the project and the terminal with two terminals open, where the
  header chip degrades to a glyph. Switching project and terminal from the workspace sheet both work.
- [ ] Antigravity installs from the Agents surface onto a build whose application id carries a
  suffix. This is the defect the install-path change exists for and it can only be shown on a device.
- [ ] The Antigravity flicker is *measured*, not asserted: PTY geometry-change counts from the
  Diagnostics sheet, before and after, with the keyboard opening and closing. If the count is zero
  while it still flickers, the fix in this cycle is not the fix and the release notes must say so.
- [ ] Antigravity's credential marker is observed on a device and added to the catalog, or the
  release ships with its sign-in state honestly reported as unknown.
- [ ] `verb world list` includes the Agent Runtime home, and a schema-v2 export/preview/apply
  round-trip restores Antigravity's configuration on a disposable emulator.

## Packaging and publication

- [x] Version and release notes describe a developer preview, not a finished product.
- [x] Signed Android artifact and checksum are produced by the release workflow. The beta.5 assets
  were built and uploaded by `Release Full CLI` run 33289341030 on `6284bc3`, which decodes the
  signing keystore from secrets, runs `assembleFullCliRelease`, and self-verifies the checksum with
  `sha256sum --check`. The published APK's SHA-256 matches its checksum asset, and it is signed by
  `CN=Shailesh Rawat` rather than a debug key.
- [x] The built APK's own manifest states the package, versionName and versionCode the release was
  asked for. Read from the artifact with `aapt dump badging` after the build and before publication,
  never from Gradle source.
  Enforced by `Release Full CLI` run 33289341030 for `v0.1.0-beta.5` at `6284bc3`, which
  reported `APK verified: com.aistudio.verb.app 0.1.0-beta.5 (5)` before publishing. SHA-256
  `7cfda6560e6dba0b9bd35f206bcd56b81ebf7ae3711ccd47ae3db37c401661a9` matching its checksum asset,
  signed by `CN=Shailesh Rawat`.
- [x] CI and Release GitHub Actions workflows pinned to immutable full commit SHAs with version comments.
- [x] Desktop installation is documented; absence of prebuilt desktop binaries is explicit.
- [x] Return/public archive contains only source, tests and documentation and excludes `.git`, local
  configuration, caches, targets, build output, keystores, agent state and temporary directories.
