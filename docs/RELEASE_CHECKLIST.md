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
- [x] OpenCode 1.18.21 installs, launches its real TUI and exits to the shell physically.
  The second half of this line -- "recovery is clearly labelled unverified/experimental" -- was
  **not true of the shipping UI** and has been struck rather than left standing. No such label
  exists anywhere in the app: on 5 September the OpenCode card read "Session recoverable" beside a
  "Resume" button, in the same words and the same weight as Claude Code and Codex CLI.
  That turns out to be the *correct* behaviour rather than a missing caveat.
  `agentSessionDisplay` is a pure function of `VerbSession.state` and is shared by every agent on
  purpose, and `OpenCodeAgentAdapter.canResume` earns `RECOVERABLE` from real local evidence --
  it copies `~/.local/share/opencode/opencode.db` and returns `YES` only when that project has a
  session row carrying messages. Adding an "experimental" caveat would contradict the evidence the
  state was derived from. Hermes and Antigravity, which have no adapter and therefore no session
  coordinator, never reach this display at all and were observed showing only "Ready" -- so the
  rule that they must not be promoted to recovery-capable is held by the architecture, not by
  wording.
- [x] Multi-terminal concurrent isolation and reattachment verified on physical Vivo I2202 device:
  $T_1$ running Claude Code v2.1.250, $T_2$ running OpenAI Codex CLI, $T_3$ running interactive shell;
  tab switching causes zero state mutations to $T_1$ or $T_2$, background command execution in $T_3$ does
  not mutate agent state, interrupting Codex in $T_2$ via `^C` transitions only $T_2$ while leaving Claude in $T_1$
  running, and Activity re-creation cleanly reattaches sessions with intact state.

### beta.8 acceptance, and the beta.9 fixes it produced

Every observation below was made against the **published beta.8 artifact**, which is what makes them
worth recording: they are findings about the thing that shipped, not about a working tree. The
defects that were fixed are fixed in **beta.9**; the boxes that remain open are still open.

Exercised on the physical Vivo I2202 on 5 September 2026 against the **published** artifact --
the installed `com.aistudio.verb.app` base APK hashes to
`e9272bc85cf3f1784fdc6a1bff336dedde3ca42ba927542514a9dbe4985255c6`, which is the beta.8 release
asset pinned to `74f0df8`. So these observations are of the thing that would ship, not of a local
build of it. `docs/BETA8_HANDOFF.md` carries the procedure for each.

- [x] The changes compile and the unit suite passes.
  `:app:testFullCliDebugUnitTest :app:testPlayDebugUnitTest :app:lintFullCliDebug :app:lintPlayDebug
  :app:assembleFullCliDebug` all pass locally.
- [ ] `assembleFullCliDevice` installs over the existing app with `adb install -r`.
  **Cannot be done on this device, and the box should be rewritten rather than ticked.** The phone
  carries the release-signed artifact; a local `device` build is signed with `debug.keystore`, so
  the install is refused with `INSTALL_FAILED_UPDATE_INCOMPATIBLE: signatures do not match`. The
  only way through is an uninstall, which is precisely what destroys the working world this box
  exists to protect. Local UI changes were verified on the separately-installed
  `com.aistudio.verb.app.debug` package instead.
- [x] System -> Device Information reports `0.1.0-beta.8 (8)` and `com.aistudio.verb.app`.
  Also `Vivo I2202`, `Android 14 (API 34)`, `arm64-v8a`.
- [x] The workspace line names the project and the terminal with two terminals open, where the
  header chip degrades to a glyph. Observed with three terminals: the header chip collapsed to the
  folder glyph and grew a `2/2` switcher while the workspace line kept reading
  `demo / Terminal 2 · codex running`. Switching terminal from the workspace sheet works.
  Switching *project* does not move a terminal that is already open -- which is the runtime's
  documented and correct behaviour, but the sheet claimed the opposite in words. Copy corrected.
- [ ] Antigravity installs from the Agents surface onto a build whose application id carries a
  suffix. **Not shown, and blocked earlier in the chain than this box assumes.** On the release
  build the Agents surface refuses with "Antigravity needs the optional Agent Runtime. Install it in
  System first", and System's only route to an Agent Runtime is importing three files from a GitHub
  Actions artifact that are not on the device. On the `.debug` package -- which does carry a
  suffixed application id and an already-imported runtime -- Antigravity launched and rendered its
  real TUI, so the launch path itself is sound.
- [x] The flicker is *measured*, not asserted -- and the measurement says this cycle's fix is not
  the fix, exactly as this box anticipated. Read off the Diagnostics sheet with Codex running:
  * idle, untouched, 25 s: **0 geometry resizes** per 5 s window, against 66 `onTextChanged`
    callbacks in 5043 ms -- Codex repaints its animated welcome logo about 13 times a second.
  * five keyboard open/close cycles: **2 resizes** in the window containing a cycle, one per
    transition, which is the minimum possible.
  So PTY geometry churn is not what people are seeing. The keyboard fix in `7170c94` holds; the
  visible flicker is Codex's own render loop, and the release notes must say so rather than claiming
  a flicker fix.
- [x] Antigravity's credential marker is observed on a device and added to the catalog.
  beta.9 shipped it as `UNKNOWN`, which was the honest half of this box: it declared no
  `signedInMarkers`, so `AgentSignInDetector` said nothing rather than guessing a path. Once the
  OAuth fix let a sign-in complete, the marker was read off the Vivo I2202 --
  `.gemini/antigravity-cli/antigravity-oauth-token`, in the *Agent Runtime* home rather than the
  local userland one -- and added in beta.10. `RuntimeProfilesTest` had pinned the empty list
  precisely so that adding it would be a deliberate act with a failing test attached, which is how
  it went.
  The same pass found Hermes signed in at `~/.hermes/auth.json` and equally unreported, so its
  marker went in too. That exposed a second gap: `WORLD_PATHS` did not cover `files/home/.hermes`,
  so `verb export` would have restored a Hermes that was logged out -- the identical failure the
  `.gemini` entry had been added to fix one agent earlier. `WorldCoversSignInTest` compares the
  Kotlin catalog against the shell script and now fails when they drift, and a second test names
  the four covered agents rather than counting them, because a marker quietly dropped from the
  catalog would make the first test pass by having nothing to compare.
- [ ] `verb world list` includes the Agent Runtime home, and a schema-v2 export/preview/apply
  round-trip restores Antigravity's configuration on a disposable emulator. Not attempted.
  `verb export` was run far enough to print its manifest -- `.env`, `.claude`, `.claude.json`,
  `.codex`, `.config/opencode`, `.local/share/opencode` and three `shared_prefs`, 64 MB total -- and
  was then aborted at the passphrase prompt rather than encrypting the owner's real credentials
  under a passphrase chosen by a tool. No `.gemini` entry appeared, consistent with Antigravity not
  being installed on this device.

### beta.8 defects found on the device, still open

- ~~**Antigravity sign-in cannot be completed.**~~ **Fixed and signed in on the device.** Tapping the
  printed OAuth URL used to open `accounts.google.com/signin/oauth/error`, whose `authError`
  base64-decodes to `invalid_request / Required parameter is missing: response_type` and whose echoed
  `client_id` was `1071006060591-tmh` -- the first wrapped line of the URL and nothing after it.
  Logging the buffer rows on the device found the cause: `numCols=91` while **every row of the URL
  came back as one leading space plus 90 characters**. Antigravity draws its sign-in screen inside a
  one-column inset and wraps the URL itself, so these are not emulator-wrapped rows -- the native
  join puts real newlines between them. `joinWrappedTerminalLines` appended the rows verbatim, that
  indent landed inside the URL, and the regex stopped at it.
  A URL continuation now contributes its content and not its indent. Emulator-wrapped URLs are
  unaffected, since their continuations start at column zero. `AntigravityOAuthUrlTest` pins the
  rows exactly as logged, leading spaces included, and also holds the two cases a blanket join would
  break: prose above the URL, and a short URL above an ordinary sentence.
  Verified end to end: Chrome opened the full URL, Google accepted it, `agy` reported
  "Signing in..." and reached its prompt as `Gemini 3.8 Flash · high`.
- **The authorization-code field is unreachable while typing.** It is only visible with the keyboard
  closed, and the keyboard is what you need to enter the code with. Open, and Antigravity truncates
  its own pane to "(1-17 of 27 lines)". Only relevant on the paste-the-code fallback path, since the
  browser round-trip now completes on its own.
- ~~**The Agent Runtime called itself incompatible while Antigravity ran inside it.**~~ **Fixed in
  beta.11.** The card read "Installed, but incompatible on this device. This Linux runtime cannot
  execute inside this Android app sandbox" on a device where `agy` was running in that same runtime
  one screen away. The wording was not the defect:
  `AgentRuntimeCompatibilityProbe.AGENT_PROBE_COMMAND` ran `/usr/local/bin/claude --version`, so a
  Claude-specific result became the runtime's whole-device verdict -- and Claude Code does not use
  the Agent Runtime in the shipping product at all, since it installs into the local userland. The
  check was gating every agent on one that never runs there.
  The probe is the runtime's own shell again, which is the only question a runtime-wide verdict can
  honestly answer; whether a given agent works stays that agent's own catalog probe, on its own card.
  The message now names the evidence -- "its shell would not start on this device, so nothing in it
  can run" -- rather than pronouncing on the sandbox, keeping the original rule against naming an
  Android policy nobody identified.
  The probe's test was already called "the probe runs bash --version, never a login or interactive
  shell" but asserted a locally-built argv instead of the constant, so the drift was invisible for
  as long as it existed. It now asserts the constant, and a second test fails if `claude`, `codex`,
  `opencode`, `agy` or `hermes` reappears in the runtime-wide probe.
  **Behaviour change:** `canOpen` becomes true wherever the runtime shell works, so "Open agent
  terminal" is enabled where it was previously greyed out.
- ~~**Antigravity's PTY is resized about ten times per keyboard toggle.**~~ **Fixed and measured.**
  A *different* defect from the Codex flicker above, and the opposite shape: Codex churns output
  with no resizes, Antigravity churned resizes. Measured with `agy` at its prompt, three keyboard
  open/close cycles:

  | build | resizes per 5 s window | `onTextChanged` |
  | --- | --- | --- |
  | as shipped | **11**, oscillating `91x8` / `91x48` / `91x54` | 40 per 5 s |
  | fixed | **0 and 1**, `91x21` then `91x67` | 1 in 14 s |

  `TerminalScreen` already meant to keep its chrome off a running agent -- its comment says showing
  the first-action row "would mean this row appearing and disappearing with the keyboard, resizing
  the PTY under a TUI each time" -- but the guard was `alternateScreenState == ACTIVE`. Claude Code
  and Codex CLI switch buffers; Antigravity renders inline and never does, so it was the one agent
  the guard could not see. The guard now also asks whether an agent *occupies* the terminal.
  For that to mean anything, Verb had to start recording it: Antigravity has no session coordinator,
  so nothing claimed the foreground when it launched. `launchAgent` now claims it directly, and
  releases it when the terminal stops being the Agent Runtime -- not merely when the PTY exits,
  because restarting the session runs the same guest command again and Antigravity is back a second
  later.

  **Occupancy is not recovery.** No `VerbSession` is written, no coordinator exists, and
  `VerbTerminalSessionHolderTest` pins that: Antigravity can hold a terminal while
  `agentSessionDisplay` still returns null, which is where "Session recoverable" and the Resume
  button come from. Verified on the device -- the workspace line reads `Terminal 1 · agy running`,
  it survives a session restart, and the Agents surface still offers Antigravity no Resume.
- ~~**Resume typed its command into the running agent.**~~ **Fixed.** Found while testing the above:
  with Antigravity running, tapping Resume on Claude Code put the literal text
  `claude --resume d4a9f42d-…` into *Antigravity's chat box*. Nothing resumed, and a Claude session
  id was handed to a different vendor's agent. `launchAgent` had always returned the terminal to the
  Verb CLI userland first; `resumeAgentSession` never did. It does now, and the round trip was then
  observed end to end: the shell came back, `claude --resume` ran in it, and the 21 August
  conversation returned with the workspace line reading `Terminal 1 · claude running`.
  `RuntimeProfilesTest` pins the assumption underneath it -- Antigravity is the only admitted agent
  that runs in the Agent Runtime -- so a second one fails there rather than on a device.


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
