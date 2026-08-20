# Handoff

Written 2026-08-20 at the close of the agent-runtime sprint. Read this with `docs/NEXT_SPRINT.md`,
which holds the blocker detail, and `docs/AGENT_RUNTIME_V1.md`, which holds the backend evidence.

## State

- Branch `agent/runtime-truth-hardening`, clean, pushed, HEAD `7b2bc32`.
- `295 tests, 0 failures` on both `fullCliDebug` and `playDebug`; both APKs build.
- Nothing uncommitted, nothing half-applied, no temporary instrumentation anywhere.
- Validation device: Vivo I2202 (Android 13, arm64). **Not currently connected** — ask before
  assuming device work can run.

## What Verb does now

Phone-native, all verified on device:

- Real PTY terminal, live working directory from OSC 7, command history with exit codes, Ctrl-C.
- Diagnostics showing launch directory and current directory as separate facts, with a reachable
  Copy Report.
- Projects, file browser, session restart, project switching.
- Package installs that resolve prerequisites automatically, and refuse honestly when nothing can
  resolve a constraint.
- Agents tab: per-agent state, the command each runs, one-tap open, and a keys card showing presence
  only.

Agents:

| Agent | State |
| --- | --- |
| Codex CLI | Runs. Authenticated, real session completed. |
| Claude Code | Runs and is authenticated, but `claude` on PATH is currently broken — see below. |
| OpenCode | Launches (`1.18.18`). No authenticated session yet. |
| DeepSeek Harness (`dsh`) | Launches (`0.1.0-rc.7`). No authenticated session yet. |
| Bun runtime | Runs (`1.3.14`). |
| DAX | Installs and starts; missing `@opentui/solid/bun-plugin`. |
| Hermes | Blocked on building `cryptography`. |

## The one idea that explains the whole sprint

**A binary runs on-device when its ELF interpreter exists.**

Static musl and Bionic builds run as-is. Dynamically linked musl builds run once
`/lib/ld-musl-aarch64.so.1` and the musl C++ runtime are installed — Verb bundles both and binds the
loader. glibc builds do not run.

Everything that looked like a sandbox restriction during this sprint turned out to be a missing
file. Two conclusions were published and later disproved (Android's seccomp filter; "abandon
on-device agents, go remote"). Prefer a trace over an inference here.

## Working method — worth keeping

- **Verify in the app process, not under `run-as`.** `run-as` carries no app seccomp filter and gave
  false positives twice this sprint. A result from `adb shell run-as` does not transfer.
- **Read the artifact rather than guessing.** Entry points came from the wheel's `entry_points.txt`;
  the package came from PyPI metadata. One commit shipped a guessed command (`claude install`) and
  had to be reverted in `3607272`.
- Device driving: helper at `scratchpad/verb.sh` (`dump`, `tapText`, `run`, `diag`, `browse`).
  `run-as … sh -c` is broken on this Vivo — push a script and run `run-as … sh files/x.sh`.

## Next, in priority order

### 1. Wrapper survival (user-visible, do this first)

`claude` on PATH is broken, so the Agents tab reports Claude Code as not installed while it is
installed and authenticated. The card is telling the truth; the underlying state is wrong. Two
causes, both measured:

- `$PREFIX/bin/claude` was overwritten by npm with a symlink to `claude.exe`, destroying Verb's
  generated wrapper.
- `$HOME/.local/bin/claude` was added by Claude's own self-installer, wins PATH, and fails with
  `has unexpected e_type: 2`.

`$PREFIX/bin/opencode` survived and works, which shows the capability is fine and the problem is
surviving vendor installers and self-updates. A version-resolving wrapper placed where it wins PATH
is the likely shape.

### 2. Two small UI fixes

- The soft keyboard hides the `ESC` / `CTRL` / `PASTE` strip, which is exactly when it is wanted.
- The Agent Runtime "Choose" buttons sit where the thumb scrolls; the file picker opens by accident.

### 3. Signed-in state per agent

"Ready" means the binary runs, not that you are authenticated. Codex and Claude are signed in;
OpenCode and `dsh` are not, and nothing in the UI distinguishes them.

### 4. Deferred by the user, in order

- Keystore-backed key injection. Currently keys live in `$HOME/.env`, plaintext, `0600`, app-private,
  never logged, never in the diagnostics report, never sent to a provider. Keystore injection would
  make Verb a credential broker — a trust-model change to decide deliberately, not by default.
- `bun run` fails under PRoot (`CouldntReadCurrentDirectory`); `bun <file>` works.
- DAX missing `@opentui/solid/bun-plugin` — workspace resolution in the DAX repo.
- Hermes' `cryptography` build: cargo cannot execute the build scripts it just compiled. Blocks every
  Rust-backed Python package, not just Hermes. Cheapest path for Hermes alone is dropping the
  `PyJWT[crypto]` extra in the user's fork.

## Open questions for the user

- Hermes: is `cryptography` actually needed, or can the `[crypto]` extra go?
- DAX: is `@opentui/solid` meant to resolve from the workspace, or be installed separately?
- The QEMU/PRoot Debian Agent Runtime works for a single process but cannot exec a second one, so
  nothing that forks runs inside it. It is now redundant for agents. Keep, gate, or remove?

## Not yet verified anywhere

Authenticated sessions for OpenCode, `dsh` and DAX. Whether an agent can spawn subprocesses (`git`,
`rg`) during a real task — agents do this constantly, and it is the gap between "launches" and
"usable". Battery, thermal and memory cost under sustained use. Long interactive TUI sessions.
