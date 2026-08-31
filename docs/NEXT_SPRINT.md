# Next sprint — carried-over work

> **Status of this document (2026-08-21): a dated snapshot.** Current scope lives in
> `docs/ROADMAP.md` (M1 — desktop workspace) and the product layer in `docs/PRODUCT_VISION.md`,
> `docs/PRD.md` and `docs/TUI_VISION.md`. Kept as written: it records what was measured at the time,
> including agent states that have since changed.

Written at the close of the agent-runtime sprint. Everything here is either measured on a physical
Vivo I2202 or explicitly marked as unverified.

## Where agents stand

| Agent | State |
| --- | --- |
| Codex CLI | Blocked. Authenticated, but neither installed copy executes — see 4b. |
| Claude Code | Runs (`2.1.235`). Authenticated. Launch hardened; see 4. |
| OpenCode | Launches (`1.18.18`). No authenticated session yet. |
| DeepSeek Harness (`dsh`) | Launches (`0.1.0-rc.7`). No authenticated session yet. |
| Bun runtime | Runs (`1.3.14`). |
| DAX | Installs and starts; blocked, see below. |
| Hermes | Runs (`0.15.2`). FIXED & verified on device (2026-08-31) — see 3. |

The rule that decides all of it: **a binary runs on-device when its ELF interpreter exists.** Static
musl and Bionic builds run as-is; dynamically linked musl builds run once the bundled loader and C++
runtime are installed; glibc builds do not run at all.

## Blocked, with the exact failure

### 1. `bun run` fails under PRoot

```
bun run i.ts   -> error: An internal error occurred (CouldntReadCurrentDirectory)
bun i.ts       -> works
```

Reproduced with a two-line throwaway package, so it is not specific to any project. Ruled out:
working directory (`bun -e process.cwd()` reports correctly) and `--link2symlink` (fails without it
too). Invoking the entry file directly is a working substitute.

This blocks any project whose scripts go through `bun run`, which is the normal way to start one.

### 2. DAX is missing a workspace dependency

```
bun src/index.ts --help
-> error: Cannot find module '@opentui/solid/bun-plugin'
```

`bun install` reports 1774 packages installed successfully, so this is dependency resolution in the
DAX workspace rather than a platform limit. DAX's own `bin/dax` honours `DAX_BIN_PATH`, which is
probably the cleanest way for Verb to launch it once this resolves.

### 3. Hermes and native Rust/C Python extensions — FIXED, verified on device (2026-08-31)

`hermes-agent` runs in its own virtual environment (`$HOME/.venvs/hermes`) with `--system-site-packages`
to link prebuilt Termux binary packages (`python-cryptography`, `python-psutil`), and compiles
native ARM64 extensions (`jiter`, `pydantic-core`, `cffi`, `ruamel.yaml.clib`) directly on-device using
Termux `clang` and `cargo` with:
- `TMPDIR=$PREFIX/tmp` (resolving Android's missing `/tmp` directory)
- `SSL_CERT_FILE=$PREFIX/etc/tls/cert.pem` & `CARGO_HTTP_CAINFO=$PREFIX/etc/tls/cert.pem` (TLS trust)
- `CARGO_BUILD_TARGET=aarch64-linux-android`, `ANDROID_API_LEVEL=24`
- `CC=clang`, `CXX=clang++`, `AR=llvm-ar`, `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=clang`
- `RUSTFLAGS="-C link-arg=-landroid-support"`

Console scripts are automatically wrapped into `$PREFIX/bin` at install time. Hermes v0.15.2 executes
natively and launches directly in Verb's terminal.

### 4. Agent wrappers are overwritten and shadowed — FIXED, verified on device

Both causes were reproduced on the Vivo before the fix and are gone after it.

```
$PREFIX/bin/claude        -> ../lib/node_modules/@anthropic-ai/claude-code/bin/claude.exe   (npm)
$HOME/.local/bin/claude   -> $HOME/.local/share/claude/versions/2.1.235                     (self-installer)
```

There were **two** causes, and finding only the first is what made an earlier "fixed" claim wrong.

**Cause A — the launcher was written once, at install time, into a directory other people's
installers own.** Nothing written there can survive them. Launching is now
`AgentWrapperBootstrap`'s, and holds because of three properties rather than a repair:

1. `$PREFIX/libexec/verb/bin` is Verb-owned — npm writes to `$PREFIX/bin`, vendor self-installers
   to `$HOME/.local/bin`, and neither has any reason to touch it.
2. It is **first** on the guest PATH, ahead of both.
3. It is rewritten on every launch, like the shell-integration script. A wrapper anything manages
   to damage is repaired by the next app start.

**Cause B — a shell startup file put the broken launcher back in front.** Fixing the base PATH is
necessary but not sufficient: a base PATH is a starting point, and `$HOME/.bashrc` runs afterwards.
The Codex installer had written this into it:

```
# >>> Codex installer >>>
export PATH="$HOME/.local/bin:$PATH"
```

so every real shell re-prepended the vendor directory ahead of Verb's, and `claude` resolved back to
the launcher that fails with `e_type: 2`.

This is also why the Agents tab and the terminal disagreed. `GuestCommandRunner` deliberately never
sources user startup files, so the probe resolved `claude` through Verb's base PATH and reported
**Ready**, while the terminal — a login shell that *does* source them — got the broken one. Any
future "the card says Ready but it does not run" belongs to this same gap: **check a real login
shell, not just the probe.**

The shell-side fix is a Verb-owned marked block kept *last* in `.bashrc`, re-appended on every
launch so a later installer cannot outrank it, plus the same correction at the top of
`shell-integration.bash` ahead of both of that script's early returns (login shells read
`.bash_profile`, not `.bashrc`). It prepends only when the directory is not already leading, so
re-sourcing cannot grow PATH without bound.

Resolution moved from install time to launch time, which is what makes a self-update a non-event:
each wrapper walks a candidate list, resolving globs newest-first, and reads the ELF interpreter out
of the file instead of assuming it. Exit codes are POSIX `env`'s 127/126, so an agent that is
genuinely absent still probes as MISSING rather than hiding behind a wrapper that always exists.

Measured after installing the fix:

| Command | Result |
| --- | --- |
| `claude --version` | `2.1.235 (Claude Code)`, exit 0. Agents tab: **Ready**. Sign-in intact. |
| `opencode --version` | `1.18.18`, exit 0. Agents tab: **Ready**. |
| `dsh --version` | `0.1.0-rc.7`, exit 0. Agents tab: **Ready**. |
| `command -v claude` | Verb's wrapper — the `$HOME/.local/bin` launcher no longer shadows it. |

Both Claude candidates were checked, not just the one that won: the npm musl package runs through
the bundled loader, and the self-installer's `versions/2.1.235` declares
`/lib/ld-musl-aarch64.so.1` and runs through the same loader once detection routes it there. So the
fallback that would carry a future self-update is verified, not assumed.

### 4b. Codex — FIXED. Verb now resolves what it can, and proot was the real obstacle

Two separate things were wrong, and an earlier note in this document blamed the wrong component for
the second. Corrected here, with how it was actually established.

**The dependency npm skipped.** `@openai/codex` is a launcher; the real binary ships in an
*optional* dependency declared as an alias:

```
"@openai/codex-linux-arm64": "npm:@openai/codex@0.147.0-linux-arm64"
```

npm skips it here, and `codex.js` then stops with `Missing optional dependency`. Verb calling that
"cannot launch" would be giving up on something plainly resolvable, so it is resolved: `codex.js`
falls back to `<package>/vendor/<triple>/bin/codex` when the optional package is absent — read out
of `codex.js`, not guessed — and Verb's install unpacks the published platform tarball there.
`npm pack` resolves the tarball location, and the version comes from the launcher actually
installed, so the two cannot drift.

**proot refuses static binaries — and that is what `e_type: 2` always meant.** The earlier claim in
this document, that the message came from Codex's own launcher and not from Verb's runtime, was
wrong. It came from a `strings` search that returned nothing and was read as evidence of absence.
Re-run with `grep -a`, `proot` does contain the string. The decisive checks:

```
codex (static musl, ET_EXEC) run from Android's shell   -> codex-cli 0.147.0
the same binary run inside proot                        -> error: "..." has unexpected e_type: 2
proot --version, run inside proot                       -> error: "..." has unexpected e_type: 2
```

Verb's own `proot` is such a build, so proot refusing itself is the tell. A terminal session is
always inside proot and nothing escapes a ptrace sandbox from within, so the binary cannot simply be
exec'd.

`qemu-aarch64` maps the ELF itself rather than handing it to proot's loader, and runs it:
`codex-cli 0.147.0`. Verb already shipped that as the **Agent Emulator** profile, so Codex now
declares it a prerequisite and the wrapper routes static builds through it. The emulator's
description, which claimed it was for Claude Code and OpenCode, was stale and has been corrected.

Verified on device: `codex --version` -> `codex-cli 0.147.0`; Agents tab shows Codex CLI **Ready**.
No reinstall was needed — the wrapper picked up the standalone build already present.

### The general rule this establishes

A launch failure is only worth reporting to the user once Verb has tried what it owns:

| Failure | Resolution |
| --- | --- |
| npm skipped a platform package (`os`/`cpu`/`libc` mismatch on Android) | install it with `--force`, or unpack the published tarball where the launcher looks |
| Dynamically linked musl build, no interpreter on Android | bundled musl loader |
| Static `ET_EXEC` build, refused by proot | `qemu-aarch64` |
| glibc-only build | genuinely unresolvable — say so |

The wrapper reads the ELF interpreter at launch to decide which of these applies, so a binary Verb
has never seen still lands in the right branch. "Cannot launch" is reserved for the last row.

## Not yet verified

- An authenticated session for OpenCode, `dsh`, or DAX.
- Whether an agent can spawn subprocesses (`git`, `rg`) during a real task. Agents do this
  constantly, and it is the difference between "launches" and "usable".
- Battery, thermal, and memory cost under sustained agent use.
- Interactive TUI rendering over a long session.

## Also carried over

- **Diagnostics `Copy Report` sits in a bottom action row** that was clipped on the validation
  device. Fixed, but the same layout pattern exists elsewhere and is worth auditing.
- **The terminal dock was rebuilt** (see below). The Agent Runtime "Choose" buttons sitting under
  the scrolling thumb is still open.
- **The Agent Runtime (QEMU/PRoot Debian rootfs)** works for a single process but cannot exec a
  second one, so nothing that forks can run inside it. It is currently redundant now that agents run
  natively; decide whether to keep, gate, or remove it.
- **Remote SSH backend** — discovery written, no code. Still the answer for heavy workloads
  regardless of what runs on-device.

## The terminal dock — done

At rest the dock took roughly 40% of the screen: a command field, then three stacked key rows, then
the navigation bar. Two of those rows were also gated on `!isKeyboardVisible`, so ESC / CTRL / PASTE
disappeared the moment the soft keyboard opened — precisely when a terminal user reaches for them.

Now two rows, and nothing is gated on the keyboard:

- Command field, send, and one chevron that opens the rest.
- One resting key row: `▲ ▼ TAB ^C ESC CTRL PASTE`. All seven fit across a 1080px phone without
  scrolling, which was the constraint that decided the set — a resting row you have to scroll is a
  row you stop using.
- The chevron adds `SHIFT ◄ ►` and the editable symbol keys. Arming CTRL still reveals the `^X`
  combinations on its own, expanded or not, since arming it is a request for the key that follows.

`KeyButton` stopped being a Material `OutlinedButton`, whose 58dp minimum width made a two-character
key as wide as a five-character one and let only five keys fit. Sizing to content fits all seven and
allowed the touch target to grow from 34dp to 38dp at the same time.

Measured on the device: dock plus navigation went from ~38% of the screen to ~27%, so the terminal
output area gained about a fifth of its height.

## Subprocess spawning — verified, and it was the real question

The gap between "launches" and "usable". Measured on the device:

```
git 2.55.0 / rg 15.2.0 / node v24.18.0 / bash 5.3.15 / grep / find / sed   all resolve
node -e child_process.execSync("git --version")                            works
bash -c "bash -c 'git --version'"  (nested to depth 3)                     works
claude -p "Run: git --version"  -> tool call -> subprocess -> output       git version 2.55.0
```

The last line is the one that matters: Claude Code took a prompt on the phone, called its Bash tool,
spawned `git`, captured the output and answered with it. An agent doing real work is no longer
unverified.

## Signed-in state per agent — mechanism done, two agents covered

"Ready" only ever meant the binary runs, which is not what a user is asking. Each agent card now
also reports sign-in, from the **presence** of a credential file -- never opened, never logged,
never in the diagnostics report, the same boundary the API keys card holds.

Observed on the device while both were authenticated:

| Agent | Marker |
| --- | --- |
| Claude Code | `.claude/.credentials.json`, `.claude.json` |
| Codex CLI | `.codex/auth.json` |

OpenCode and `dsh` report **UNKNOWN**, and the card says nothing for them rather than implying
"signed out". Both binaries reference a bare `auth.json` with the path built at runtime, and neither
has been signed into, so there is nothing to observe yet. `AgentSignInState.UNKNOWN` exists
precisely so Verb does not invent the fact. Adding a marker once one is seen is a one-line catalog
change.

**Not yet seen on device:** the sign-in line itself. It is unit-tested and builds, but the app would
not return to the foreground after an Android USB dialog interrupted the session, so the rendered
label is unconfirmed.

## Next up (agreed)

1. A persistent agent session for the terminal.
2. Testing and hardening the agent path with real API keys.
3. Key handling — see `$HOME/.env`, added this sprint.
