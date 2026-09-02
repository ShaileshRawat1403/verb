# beta.8 execution handoff

*Written 2 September 2026, for the local agent that has the Vivo I2202 on ADB. Everything below is
either already done, or a step that needs a device and therefore could not be.*

The code in this working tree is written and unit-tested by construction. **None of it has been
compiled or run.** The environment it was written in has Java 11, no Android SDK and no network, so
`compileKotlin` has never executed against these changes. Treat step 1 as load-bearing.

---

## 0. Before anything else: protect the working world

The phone holds live sign-ins for Claude, Codex and Antigravity. Do this first, every time, and do
not skip it because the install "should" be safe.

```bash
# In Verb's own terminal on the phone
verb export ~/pre-beta8.vbak
# then: System -> Working world -> Save to Downloads
```

Confirm the archive lists `files/agent-runtime/homes/default/.gemini` in `verb world list`. If it
does not, Antigravity's sign-in is **not** in the archive and step 5 has found a real gap.

---

## 0.5. Two things the commit must not miss

* **`scripts/enforce-emulator-only-connected-tests.sh` is untracked.** `app/build.gradle.kts` now
  registers an `Exec` task pointing at it, so a commit that includes the build file without the
  script gives everyone else a build that cannot configure. `git add scripts/` belongs in the same
  commit as the Gradle change.
* **`.claude/` has been added to `.gitignore`.** It holds a local agent session checkpoint, which is
  machine-specific and not part of the product.

---

## 1. Compile and run the unit tests

```bash
./gradlew :app:compileFullCliDebugKotlin
./gradlew :app:testFullCliDebugUnitTest
```

Expect breakage here rather than in the app. The changes with the highest chance of a compile error,
in order:

1. `TerminalScreen.kt` — a new `WorkspaceContextBar` call, a new `onCloseTerminalSession` parameter,
   the removal of the private `ProjectSheet`, and `expandVertically` / `shrinkVertically` imports
   deleted after their last use was removed.
2. `WorkspaceSheet.kt` and `WorkspaceContextBar.kt` — new files, imports written by hand.
3. `VerbViewModel.resolveInstallCommand` — uses `AgentRuntimePaths.DEFAULT_AGENT`, a companion
   object added in this change.
4. `TermuxTerminalRuntimeAdapter.recordGeometryForMetrics` — reads `emulator.mColumns` / `mRows`,
   which are public fields on the vendored `TerminalEmulator`.

New tests that must pass:

* `RuntimeProfilesTest` — no install command may contain `/data/data/`.
* `WorldCoversSignInTest` — every credential marker Verb reports on must be inside `WORLD_PATHS`.
* `WorkspaceOrientationTest` — the context line and the workspace sheet.
* `ProjectWorkspaceTest` — project display name and short id.

Fix compile errors in place. Do not weaken a test to make it pass; if a test is wrong, say so
explicitly in the change description.

---

## 2. Install without destroying anything

Use the `device` build type. It keeps the canonical application id `com.aistudio.verb.app` and is
signed with `debug.keystore`, so it upgrades the app already on the phone in place.

```bash
./gradlew :app:assembleFullCliDevice
adb install -r app/build/outputs/apk/fullCli/device/app-fullCli-device.apk
```

**If `adb install -r` fails with `INSTALL_FAILED_UPDATE_INCOMPATIBLE`, stop.** That is a signing key
mismatch, and the only way past it is an uninstall, which destroys the working world. Confirm the
archive from step 0 is in Downloads *and copied off the device* before you consider it.

Do **not** run `:app:assembleFullCliDebug` onto the phone expecting the same app. Debug now carries
a `.debug` application id suffix, so it installs beside the real app with its own empty storage: no
projects, no runtime, no logins. That is deliberate, and it is also useful — see step 3.

Verify after install: System → Device Information should read
`0.1.0-beta.8 (8)` and `com.aistudio.verb.app`.

---

## 3. The Antigravity flicker

**Symptom as reported:** `agy` starts, then the terminal flickers during use.

**Hypothesis, and what was changed on the strength of it.** `TerminalView.updateSize()` resizes the
PTY whenever the character row or column count changes, and each resize sends SIGWINCH. A shell
absorbs that invisibly; a full-screen agent UI repaints its whole frame. Four rows above the terminal
canvas were animating their *height* with `expandVertically`/`shrinkVertically`, which changes the
row count on many frames of a single animation, and two of them toggle with the keyboard. Those
transitions are now fades, and the "start an agent" offer is suppressed while a full-screen agent
already owns the canvas.

**This is a hypothesis, not a diagnosis.** It was reached by reading code, not by watching the
device. Confirm or kill it before claiming it in the release notes.

### Measure it

`TermuxTerminalRuntimeAdapter` now logs, every five seconds, under `LogCategory.DIAGNOSTIC`:

```
PTY callback rate: <n> onTextChanged / <n> snapshots published in <ms>ms (<n>% coalesced by
throttle); geometry <cols>x<rows>, <n> resize(s) in window
```

Read it in the app: Terminal → overflow → Diagnostics, filter to Diagnostic. On a `device` build
`run-as` is refused, so this in-app sheet is the read path. If you would rather use `adb logcat`,
install the `.debug` build instead — it has its own storage and cannot touch the real app's logins,
at the cost of having to sign in again inside it to reproduce.

### Decide

* **`resize(s) in window` is 0 while `agy` runs and it still flickers.** The hypothesis is wrong.
  Geometry is stable, so the repaint is coming from the agent or the emulator, not from Verb's
  layout. Next suspects, in order: the alternate-buffer swap (`isAlternateBufferActive` toggling
  rapidly — log it in `refreshTerminalContext` if needed); qemu emulation delivering the TUI's
  redraws in fragments, which would show as a very high `onTextChanged` rate with stable geometry;
  and `postInvalidateOnAnimation` on every callback.
* **`resize(s) in window` is non-zero and correlates with the keyboard opening or closing.** The
  hypothesis holds and the fix is the right one. Record the before/after numbers.
* **`resize(s)` is non-zero with the keyboard untouched.** Something else above the canvas is
  changing height. Find it before writing any of this into the release notes.

Whatever the answer, put the numbers in `docs/BACKLOG.md` with a date. A flicker report with a
measurement attached is the difference between this being fixed and this being guessed at again in
beta.9.

---

## 4. Antigravity's credential marker

`RuntimeProfiles` gives Antigravity **no** `signedInMarkers`, so Verb reports its sign-in state as
unknown. That is correct today and the catalog's own rule is that a marker is added only once it has
been seen on a real device. Do that now:

```bash
# Agent Runtime home, before signing in
adb shell run-as com.aistudio.verb.app.debug \
  ls -la files/agent-runtime/homes/default/.gemini/antigravity-cli/
```

Sign in with `agy`, list again, and diff. Note that
`.gemini/antigravity-cli/settings.json` is **seeded by Verb itself** in
`QemuAgentRuntimeEnvironment`, so it exists before any sign-in and is not a valid marker. The marker
is whatever appears *because* the sign-in completed.

Then add it, relative to the guest `$HOME`:

```kotlin
signedInMarkers = listOf(".gemini/antigravity-cli/<the file you observed>"),
```

`WorldCoversSignInTest` will immediately require that path to be inside the archive. It already is,
because `WORLD_PATHS` covers the whole `.gemini` directory — but if the observed file lives
somewhere else in that home, the test will fail and `world.sh` needs the additional path. That
failure is the test doing its job.

Note `run-as` works only against the `.debug` package. The `device` build sets
`isDebuggable = false` on purpose, which is what stops a USB connection reading agent credentials
out of app storage.

---

## 5. Working World round trip, with Antigravity in it

The claim this release makes is that the archive now restores every agent Verb reports on. Prove it:

1. `verb world list` — confirm `files/agent-runtime/homes/default/.gemini` is listed with a size.
2. `verb export ~/w1.vbak`, then `verb import ~/w1.vbak` (preview only) and check the manifest names
   that path.
3. On a **disposable emulator**, restore with `--apply` and confirm Antigravity's configuration
   comes back.

Do not test restore on the phone.

---

## 6. Record acceptance

`docs/RELEASE_CHECKLIST.md` and `docs/BACKLOG.md` take dated entries. Keep automated, emulator and
physical-device results separate, as the checklist requires. Anything not accepted on hardware does
not go in `RELEASE_NOTES.md` — the beta.8 draft there is written on that understanding and should be
edited down if a claim does not survive the device.

---

## What is deliberately still open

* Antigravity and Hermes have launch support only. They are not in the durable recovery lifecycle
  that Claude Code, Codex CLI and OpenCode implement, and beta.8 does not change that.
* OpenCode recovery remains unverified, unchanged from beta.7.
* The workspace line shows the project and terminal, not each project's last working directory.
  Verb clears `currentWorkingDirectory` when a session ends, by design, so restoring a per-project
  directory would mean persisting a new fact rather than reporting an observed one. That is a
  product decision, not an oversight, and it is not made here.
