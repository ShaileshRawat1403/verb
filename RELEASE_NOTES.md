# Verb 0.1.0-beta.8 — Know where you are, keep what you signed into

*Draft. Every line below is code that is written and unit-tested; the physical-device acceptance
runs listed in `docs/RELEASE_CHECKLIST.md` are what turn this from a draft into a release.*

## What's fixed in beta.8

- **The workspace says where you are.** A line under the header names the project, the terminal, and
  what is running in it, and one tap opens a single surface listing projects and terminals together.
  The header's project chip degrades to a folder glyph as soon as a second terminal claims the row's
  width, which meant the answer disappeared at exactly the moment it was needed. Projects are shown
  by the name you typed rather than the generated id.
- **Antigravity installs onto the right device.** Its install target was a build-time absolute path
  under `com.aistudio.verb.app`. Debug builds now carry a `.debug` suffix and the Play flavour
  carries `.play`, so on those variants that path named a different package's private storage.
  Install locations are resolved by the running app, and a catalog placeholder that survives
  substitution now refuses the install instead of reaching a shell.
- **Working World covers every agent it reports on.** The archive protected Claude, Codex and
  OpenCode credentials but not the Agent Runtime home, where Antigravity keeps its sign-in. Anyone
  signed into all three held a backup that would restore two. A test now compares the catalog Verb
  reads sign-in state from against the paths the archive copies, so the two cannot drift again.
- **Less redraw churn under full-screen agents.** Rows above the terminal no longer animate their
  height. Height animation changes the terminal's row count on every frame, and every change resizes
  the PTY, which a full-screen agent UI answers with a full repaint. The "start an agent" offer is
  also hidden while an agent already owns the screen. Diagnostics now reports PTY geometry and how
  many times it changed per window, so redraw complaints have a number behind them.
- **Which app is this.** System → Device Information now shows the version and the application id.
  A debug build has its own private storage, so signing in there and finding yourself signed out in
  the release build is two apps, not lost credentials, and the screen now says so.

## Beta limits

Hermes and Antigravity still do not participate in the durable recovery semantics implemented for
Claude Code, Codex CLI and OpenCode.

Antigravity's credential file has not been observed on a device, so Verb reports its sign-in state
as unknown rather than guessing a path. The Working World archive copies its whole configuration
directory, which is the honest way to cover a location whose exact contents are unverified.

---

# Verb 0.1.0-beta.7 — Reliable authentication and startup truth

## What's fixed in beta.7

- **Complete authentication-code paste.** Verb now delivers short, single-line clipboard values at
  normal typing cadence, fixing interactive login fields that accepted only part of an eight-character
  code. Larger, multiline and Unicode pastes remain atomic and unchanged.
- **Truthful Antigravity cold starts.** Antigravity can take about 30 seconds to draw its first screen
  under Android compatibility emulation. Verb now shows bounded launch progress during that blank
  period and clears it as soon as the real TUI is observed.
- **Physical Android acceptance.** On the Vivo I2202, `12345678` reached a shell `read` intact through
  Verb's PASTE key; Antigravity 1.1.22 showed progress during startup, rendered successfully, and
  remained stable after launch.

## Beta limits

Hermes and Antigravity still do not participate in the durable recovery semantics implemented for
Claude Code, Codex CLI and OpenCode.

---

# Verb 0.1.0-beta.6 — More agents, same truth

## What's new in beta.6

- **Hermes Agent on Android.** Hermes Agent `0.15.2` installs in an isolated Python environment
  with the native ARM64 toolchain, then launches directly in Verb's terminal. The exact package
  version is pinned to the physical-device-proven release so new installs are repeatable.
- **Python includes pip.** The Python runtime profile now installs and verifies `pip`, making its
  scripting environment useful without a separate terminal repair step.
- **Antigravity admitted.** The `agy` CLI is available from the Agents surface and launches in the
  Agent Runtime after its own sign-in flow.
- **Safer agent installation.** Python-agent installs no longer write launchers into `$PREFIX/bin`.
  Verb's own wrappers stay in its private, self-healing `libexec` directory, leaving package-manager
  and user commands untouched.

## Beta limits

Hermes and Antigravity are verified as install-and-launch integrations. They do not yet have the
durable session and recovery semantics implemented for Claude Code, Codex CLI and OpenCode.

---

# Verb 0.1.0-beta.5 — Truth & Reliability

## What's new in beta.5

- **Session-bound agent lifecycle in multi-terminal workspaces.** Agent processes and lifecycle
  observers are now bound strictly to their originating concrete PTY session rather than the
  mutable active UI selection. Switching active terminal tabs or running concurrent shell commands
  produces zero false lifecycle mutations or misrouted commands.
- **Ambiguity-free session restoration.** Activity and ViewModel recreation resolves coordinators by
  exact `terminalSessionId`. Restoring multiple foreground sessions no longer defaults to an arbitrary
  active session.
- **Physical multi-agent isolation verification.** Proven concurrently on a physical Vivo I2202
  (Android 14) across $T_1$ (Claude Code v2.1.250), $T_2$ (OpenAI Codex CLI), and $T_3$ (Interactive Shell),
  verifying that interrupting Codex in $T_2$ via `^C` leaves Claude in $T_1$ completely active and intact,
  and Activity recreation cleanly reattaches all sessions.
- **Supply-chain and release hardening.** All GitHub Actions across CI and release workflows are
  cryptographically pinned to immutable commit SHAs.
- **Artifact identity enforcement.** Release workflow verifies package name, `versionName=0.1.0-beta.5`,
  and `versionCode=5` directly from the built APK's manifest via `aapt dump badging` prior to publication.

---

# Verb 0.1.0-beta.4 — developer preview

## Use beta.4, not beta.3

**beta.3's published APK is valid and correctly signed, but it reports the wrong version of itself.**
Android shows it as `0.1.0-beta.2`, with the same version code as beta.1 and beta.2, so nothing on
the device distinguishes the three. The file is genuine — its checksum matches and its signature is
the real release key — it is simply mislabelled.

beta.3 stays exactly as published. Its tag, its APK and its checksum are untouched, because a
released artifact that people may already have installed should keep matching the checksum it was
released with.

beta.4 reports itself truthfully: `0.1.0-beta.4`, version code 4. Codes 2 and 3 are skipped rather
than reused, since no published build ever identified itself with them. Installing beta.4 over an
earlier beta is a normal in-place upgrade and keeps your Working World.

Exported `.vcont` continuity archives now record the version of the build that wrote them. Archives
written by beta.3 claim to come from beta.2; that is the same mislabelling and it does not affect
whether they import.

Nothing about how Verb works changed in this release. It fixes what Verb says about itself.

---

# Verb 0.1.0-beta.3 — developer preview

## Restore notice — read this if you have an archive from beta.1 or beta.2

**A Working World archive made by an earlier beta may not restore.** `verb export` wrote archives
that `verb import` then refused:

```text
verb: the archive contains a link or special file; restore refused.
```

It affects you if you had run Codex or installed OpenCode before exporting — both leave symlinks
(`.codex/tmp/arg0`, `node_modules/.bin`) that the exporter should have excluded and did not, and
that import correctly refuses because a symlink inside an archive is a path traversal.

**What to do:** re-export with this version — `verb export ~/world.vbak` — and save the new file.
Your old archive is not corrupt, but this version cannot restore it either; the fix is on the
writing side. Keep the old one until the new export succeeds.

Export now verifies every archive against import's own rule before writing, and refuses to produce
one that could not be restored. `docs/WORKING_WORLD.md` records how this was found and why the fix
is a check rather than two more exclusions.

## What changed since beta.2

- **Ask about your own work.** One assistant, reachable from Ask Verb and from the terminal,
  answering from the evidence Verb observed itself — including what the working tree did across the
  last command. Every answer renders beside the same facts in plain language. It still cannot
  receive command text, terminal output, file contents, transcripts, credentials, absolute paths or
  a branch name. The provider-only interpretation screen, which sent nothing but the words you
  typed, has been removed.
- **Light and dark, by choice.** Verb follows the device, or you can tell it. Type "theme" into the
  Verb sheet. The terminal itself stays dark in both, because a terminal is dark.
- **A crash on Android 7.** The foreground service that holds a session at priority called an
  API-26 notification channel on a `minSdk` 24 build, so it threw the moment a session started.
- **The Android↔desktop continuity round-trip** is now physically accepted in both directions.

## Known gaps

- OpenCode recovery is unverified and labelled as such in the product.
- The desktop preview still does not publish prebuilt binaries; build from source.

---

# Verb 0.1.0-beta.2 — developer preview

This is an evidence-gathering preview, not a claim that Verb is finished. It is intended for people
comfortable using a terminal who want Claude Code, Codex or OpenCode hosted inside a more
understandable and recoverable development environment.

## What is ready to test

- Native desktop PTY workspace with project/Git context, sessions, recovery, structural evidence,
  scrollback/search, mouse control and JSON CLI output.
- Android terminal-first workspace with a searchable Verb task sheet, real Termux-derived userland,
  Claude/Codex/OpenCode adapters and explicit Working World backup/restore.
- One four-state session contract on both hosts: `LIVE`, `INTERRUPTED`, `RECOVERABLE`, `ENDED`.
- Manual checksummed `.vcont` exchange of read-only structural evidence between hosts.
- Optional provider interpretation that can execute nothing and receives no terminal output,
  command text, file contents, transcript, credential or absolute path.

## Privacy correction on upgrade

Earlier Android development builds contained a local database and preference store for command
text, terminal output and assistant messages. Those categories violate Verb's durable-data boundary
and were not needed by the current product. On first launch, this version deletes that legacy local
database/preferences and does not recreate them.

Verb continues to persist only structural session/project records and allowlisted lifecycle events.
Working World archives remain explicit, user-triggered and encrypted because they may contain agent
credentials and the user's guest environment.

## Known limits

- The terminal-first Android workspace, v1 Working World import through the current v2 reader,
  Claude/Codex conversation recovery, and OpenCode launch have physical-device evidence.
- Claude's restored files produced the same conversation identity, but Claude itself reported that
  the saved login was no longer valid. Verb therefore reports only "Saved login found" and leaves
  authentication truth to the agent.
- OpenCode Android recovery is not physically proven; the restored v1 archive contained no
  OpenCode `VerbSession` record.
- Continuity moves evidence, not a running process or an agent transcript. A destination offers
  Resume only when its own local adapter positively confirms it. The complete physical
  Android→desktop→Android picker round-trip remains pending.
- Working World archives do not contain project source trees. Projects must be protected by Git or
  another independent backup.
- Desktop is installed from source; prebuilt desktop binaries and Windows TUI support are pending.
- Android uses the established `com.aistudio.verb.app` application ID but retains an internal
  `com.example` Kotlin namespace. Renaming it is upgrade-sensitive and intentionally deferred.

See `docs/RELEASE_CHECKLIST.md` and `RETURN_HANDOFF.md` for exact verification and remaining gates.
