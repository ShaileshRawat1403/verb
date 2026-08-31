# Handoff

> **Status of this document (2026-08-21): a dated snapshot, superseded in part.** The product layer
> now lives in `docs/PRODUCT_VISION.md`, `docs/PRD.md`, `docs/ROADMAP.md` and `docs/TUI_VISION.md`;
> read those first. Two claims below were later disproved on the same device and are corrected
> where they appear: `dsh` does not run (its `koffi` native module has no Android build), and Codex
> needed the vendor tarball step before it would execute at all. The rest is kept as written,
> because a snapshot that gets edited to look correct stops being evidence.

Written 2026-08-20. Read with `docs/DURABLE_SESSION.md` (the evidence behind the next piece of work)
and `docs/NEXT_SPRINT.md` (carried-over blockers).

## State

- Branch `agent/runtime-truth-hardening`, clean, pushed, HEAD `34832ad`.
- `342 tests, 0 failures` on both `fullCliDebug` and `playDebug`; both APKs build.
- Nothing uncommitted, no temporary instrumentation, device scratch cleaned up.
- Validation device: Vivo I2202 (Android 13, arm64), connected during this session.

## What Verb does now

| Agent | State |
| --- | --- |
| Claude Code | Runs (`2.1.235`). Signed in. Spawns subprocesses during real tasks. |
| Codex CLI | Runs (`0.147.0`) through `qemu-aarch64`. Signed in. |
| OpenCode | Runs (`1.18.18`). Sign-in state unknown to Verb. |
| DeepSeek Harness (`dsh`) | ~~Runs (`0.1.0-rc.7`)~~ — **disproved 2026-08-21.** `dsh --version` answers while `require("koffi")` throws; the native module has no Android build and cannot compile there. Verb now reports it as unavailable with the reason. |
| Gemini CLI | Not installed. |
| Hermes | Runs (`0.15.2`). Installed in isolated venv with native cryptography and ARM64 toolchain. |
| DAX | Blocked on `@opentui/solid/bun-plugin`. |

Plus: PTY terminal with truthful cwd, projects, file browser, diagnostics, package installs that
resolve prerequisites, an Agents surface with per-agent sign-in, and a terminal dock that occupies
~27% of the screen instead of ~38%.

## The four things this session established

**1. A binary runs when the thing it needs exists — and there are three cases, not one.**
Dynamically linked musl needs the bundled loader. Static `ET_EXEC` is refused by *proot* and needs
`qemu-aarch64`. Scripts and Bionic builds exec as they are. `AgentWrapperBootstrap` reads the ELF
interpreter at launch and picks; nothing is assumed from the catalog.

**2. Launching must not be an install-time artifact.** Wrappers live in `$PREFIX/libexec/verb/bin`
(Verb-owned, first on PATH, rewritten every launch) and resolve their binary at exec time. npm
overwriting `$PREFIX/bin/claude` and a vendor self-installer winning PATH both stopped mattering.

**3. A base PATH is only a starting point.** `$HOME/.bashrc` runs afterwards, and the Codex installer
had prepended `$HOME/.local/bin` there. Verb keeps a marked block *last* in `.bashrc`, re-appended
every launch.

**4. Keeping a process alive and keeping the user's work alive are different problems.** Claude and
Codex already persist transcripts to disk, and `claude` supports `--continue`/`--resume`. Those
survive force-stop. See `docs/DURABLE_SESSION.md`.

## Next: Durable Session, step 2

Step 1 (lifecycle ownership) landed in `34832ad`. Step 2 is session identity, and the shape the user
specified:

```
Verb Session
├── sessionId            ├── agent
├── project              ├── agent resume identity   <- may outlive the process
├── runtime              └── lifecycle state
├── last known cwd
└── terminal process     <- may be alive or dead
```

surfaced as three states rather than one:

```
LIVE         process running                      [Attach]
RECOVERABLE  process gone, transcript on disk      [Resume]
ENDED        neither                               [Start new]
```

The promise this supports, which is better than the one it replaces: **if the process survives, Verb
reconnects you; if it dies, Verb helps you continue.** Do not promise that a process never dies.

Project switching should become navigation across sessions, not process destruction. `onCleared()`
still calls `destroy()` — that is step 2's to remove, once something owns lifetime other than the
screen.

Explicitly **not** decided yet: foreground service, `tmux` vs `dtach`. `docs/DURABLE_SESSION.md`
compares them and explains why the multiplexer choice should come last.

## Working method — worth keeping

- **Launch the build and read logcat before claiming anything is done.** No test constructs
  `VerbViewModel`, so the whole suite can pass while the app cannot start. It did: 332 green, crash
  on every launch. Verifying through `adb shell run-as` never exercises the UI process.
  ```
  adb shell am start -n com.aistudio.verb.app/com.example.MainActivity
  adb logcat -d | grep -c "FATAL EXCEPTION"      # must be 0
  adb shell dumpsys window | grep mCurrentFocus  # must be MainActivity
  ```
- **A green card is not evidence.** `GuestCommandRunner` never sources shell startup files, so the
  Agents tab and the terminal can disagree completely. Verify with `bash -lc 'command -v <agent>'`.
- **Read the artifact.** Codex's vendor path came from `codex.js`; its triple from the published
  tarball; entry points from `entry_points.txt`. Two conclusions were published and later disproved
  this sprint by guessing — including a `strings` search returning nothing, read as proof of absence
  when `strings` was simply absent.
- Environment: `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home`; `adb` at
  `/opt/homebrew/share/android-commandlinetools/platform-tools/adb`; `run-as … sh -c` is broken on
  this Vivo, so push a script and run `run-as … sh x.sh`; `PROOT_TMP_DIR` must be exported to drive
  proot by hand.

## Open, in rough priority order

1. **Step 2, Durable Session.** Above.
2. **One acceptance gap from step 1:** project switch and "restart applies the pending change" are
   unit-tested but were not driven on the device. Switch projects on the phone; the terminal should
   keep running and show the restart banner.
3. **Codex/OpenCode/dsh sign-in markers.** Only Claude and Codex have observed credential paths.
   OpenCode and `dsh` reference a bare `auth.json` built at runtime; add a marker once either is
   signed into. One-line catalog change.
4. **Hermes** — **RESOLVED & VERIFIED ON DEVICE (2026-08-31).** Runs in isolated venv `$HOME/.venvs/hermes` with `--system-site-packages` for `python-cryptography` and `python-psutil`, and builds native ARM64 wheels (`jiter`, `pydantic-core`, `cffi`, `ruamel.yaml.clib`) via Termux clang/cargo toolchain with link flags (`-landroid-support`).
5. **DAX** — `bun install` succeeds but `@opentui/solid/bun-plugin` does not resolve.
6. **`bun run` under proot** — `CouldntReadCurrentDirectory`; `bun <file>` works.
7. **Keystore-backed key injection**, deferred by the user. Keys currently live plaintext in
   `$HOME/.env`, `0600`, app-private, never logged, never in diagnostics, never sent to a provider.
   Doing this makes Verb a credential broker — a trust-model change to decide deliberately.

## Two things not to confuse

- **`qemu-aarch64`** (the Agent Emulator profile) is now **load-bearing**: Codex does not run without
  it. Do not remove it.
- **The Agent Runtime** (Debian rootfs under PRoot+QEMU) is a different thing, still redundant for
  agents — it runs one process but cannot exec a second, so nothing that forks works inside it. Keep,
  gate, or remove is still an open question for the user.

## Not verified anywhere

- Authenticated sessions for OpenCode, `dsh`, DAX.
- Battery, thermal and memory cost under sustained agent use.
- Long interactive TUI sessions.
- Whether `tmux` exists in the Termux repository for this ABI, and its size.
- Whether Codex or OpenCode expose a resume flag equivalent to Claude's.
