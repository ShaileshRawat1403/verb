# Next sprint — carried-over work

Written at the close of the agent-runtime sprint. Everything here is either measured on a physical
Vivo I2202 or explicitly marked as unverified.

## Where agents stand

| Agent | State |
| --- | --- |
| Codex CLI | Runs. Authenticated, real session completed. |
| Claude Code | Runs. Authenticated. |
| OpenCode | Launches (`1.18.18`). No authenticated session yet. |
| DeepSeek Harness (`dsh`) | Launches (`0.1.0-rc.7`). No authenticated session yet. |
| Bun runtime | Runs (`1.3.14`). |
| DAX | Installs and starts; blocked, see below. |
| Hermes | Blocked, see below. |

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

### 3. Hermes cannot build `cryptography`

`hermes-agent` depends on `cryptography` (transitively, through `PyJWT[crypto]`), which has no
Android wheel and therefore builds from source. The Rust build fails:

```
could not execute process .../build-script-build (never executed)
No such file or directory (os error 2)
```

Cargo compiles a build script and then cannot execute it. The toolchain is present (`cargo 1.97.1`,
`clang 21.1.8`), and moving the build directory out of the `/tmp` bind changed nothing.

This is worth fixing beyond Hermes: it blocks **every** Rust-backed Python package. The cheapest
alternative for Hermes specifically is dropping the `[crypto]` extra, since everything else it needs
is pure Python.

### 4. Agent wrappers are overwritten and shadowed (top priority)

Measured after the Agents surface landed, which made it visible: `claude` on PATH is broken, so the
UI correctly reports Claude Code as not installed while it is in fact installed and authenticated.

Two separate causes, both of which must be handled:

- `$PREFIX/bin/claude` was **overwritten by npm** with a symlink to `claude.exe` when the wrapper
  package installed, destroying Verb's generated wrapper.
- `$HOME/.local/bin/claude` was added by Claude's **own self-installer**, points at
  `~/.local/share/claude/versions/<version>`, wins PATH, and fails with `e_type: 2`.

Verb's wrapper still works when called by full path (`$PREFIX/bin/opencode` survived and runs), so
the fix is about surviving vendor installers and self-updates rather than about capability. A
version-resolving wrapper placed where it wins PATH is the likely shape.

### 4b. Original note: agent wrappers lose PATH precedence

`claude` and `codex` install their own launchers into `~/.local/bin` and `~/.codex`, which exec their
standalone binaries directly and hit Termux's exec shim:

```
has unexpected e_type: 2
```

Both run correctly through Verb's generated wrapper, so this is PATH ordering rather than a
capability limit. Verb's wrappers need to win, or the vendor launchers need wrapping too.

## Not yet verified

- An authenticated session for OpenCode, `dsh`, or DAX.
- Whether an agent can spawn subprocesses (`git`, `rg`) during a real task. Agents do this
  constantly, and it is the difference between "launches" and "usable".
- Battery, thermal, and memory cost under sustained agent use.
- Interactive TUI rendering over a long session.

## Also carried over

- **Diagnostics `Copy Report` sits in a bottom action row** that was clipped on the validation
  device. Fixed, but the same layout pattern exists elsewhere and is worth auditing.
- **The Agent Runtime (QEMU/PRoot Debian rootfs)** works for a single process but cannot exec a
  second one, so nothing that forks can run inside it. It is currently redundant now that agents run
  natively; decide whether to keep, gate, or remove it.
- **Remote SSH backend** — discovery written, no code. Still the answer for heavy workloads
  regardless of what runs on-device.

## Next up (agreed)

1. UI/UX review for ease of use.
2. Testing and hardening the agent path with real API keys.
3. Key handling — see `$HOME/.env`, added this sprint.
