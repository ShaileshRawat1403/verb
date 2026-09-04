# Closing the Antigravity launch issue

*3 September 2026. Written against the working tree that fixes Hermes, removes `--link2symlink`,
and launches `agy` as the QEMU guest command. One issue is open: `agy` exits 0 in ~160 ms with zero
bytes on the PTY when launched from the Verb UI, while the same command works under
`adb shell run-as`.*

The evidence collected so far is good. What follows is ordered by information gained per run, and
two of the steps are cheaper than anything attempted yet.

---

## 1. First, prove the new path can deliver output at all

"Zero bytes are ever read from the PTY" has two readings that the evidence does not separate: the
child wrote nothing, or Verb's brand-new direct-guest-command path does not deliver what the child
wrote. Everything since has assumed the first. Test it:

Launch, through the exact `activateAgentRuntime(runtime, guestCommand)` path the UI uses, with
`guestCommand = listOf("/bin/echo", "verb-pty-control")`.

* **Text appears.** The path is sound and the problem is `agy`'s own decision to exit. Go to step 2.
* **Zero bytes again.** The bug is in Verb, not in `agy`, and every hour spent on Antigravity's
  startup is spent on the wrong process. Suspects: the PTY is opened and read before
  `restartSession()` swaps the environment, the InputReader is attached to the previous session's
  fd, or `resolveSpec()` is consulted once and cached before `activeGuestCommand` is set.

Follow it with `listOf("/bin/sleep", "5")` and watch whether `waitpid` returns at 160 ms or at five
seconds. If a five-second sleep also returns in 160 ms, the child is being reaped by something other
than its own exit and no amount of `agy` instrumentation will show it.

---

## 2. Remove the contradiction in the launch chain

The root-cause analysis says guest `execve` is killed by Android's seccomp filter. The launch path
then does this:

```
proot -> qemu-aarch64 -> /usr/bin/env  agy
                         ^^^^^^^^^^^^  a guest binary whose only job is to execve another one
```

`QemuAgentRuntimeEnvironment` wraps any non-absolute command in `/usr/bin/env` for PATH resolution.
For `agy` that reintroduces exactly the operation the fix exists to avoid, and it is untested.

`RuntimeProfiles` already declares where the binary is:
`AgentBinaryCandidate("$HOME/.local/bin/agy")`. Pass the resolved absolute guest path
`/home/verb/.local/bin/agy` as the guest command. The existing branch in `resolveGuestCommand`
already skips the `env` wrapper for anything starting with `/`, so this is a change at the call
site, not in the environment builder.

If behaviour is unchanged, that is still progress: one guest `execve` is eliminated from the chain
and the remaining suspects narrow.

While there, check on device what `agy` actually resolves to inside the runtime:

```sh
command -v agy
file "$(command -v agy)"
```

`AgentWrapperBootstrap` writes Verb-owned `#!/bin/sh` wrappers that `exec` the real target. Those
live in `$PREFIX/libexec/verb/bin`, which is on the *local userland* PATH and not on the Agent
Runtime's `/home/verb/.local/bin:/usr/local/bin:/usr/bin:/bin`. Confirm that, because if a wrapper
did reach the agent home, the chain is `env` -> `sh` -> `agy` and there are three guest `execve`
calls where the design allows none.

---

## 3. Ask QEMU why, instead of inferring it

The decisive instrument is already in the stack and has not been used. `qemu-aarch64` supports
guest syscall tracing:

```
QEMU_STRACE=1          # or add -strace to the qemu argv
QEMU_LOG=unimp,guest_errors
```

Add them as `-E` values or to the qemu argv for one instrumented run and read the last twenty
syscalls before `exit_group(0)`. This replaces the whole hypothesis tree with a fact.

Watch specifically for a syscall returning `ENOSYS`. qemu-user returns that for anything it does not
implement, and a program that treats an unsupported probe as "nothing to do here" exits 0 without
printing, which is precisely the observed signature.

---

## 4. Diff the environments, since that is the real delta

`adb shell run-as` works and the app process does not. The argv is identical and a Python PTY
reproduction stays alive, so the PTY and the argv are both ruled out. What is left is the parent:
its environment, its cwd, its signal mask and its session.

qemu passes its own environ through to the guest and then applies `-E` overrides. Under `run-as`
that environ is inherited from `adb shell` and holds roughly twenty-five variables. Under Verb it is
the eight-entry array `TerminalEnvironment.variables` builds. That is a large, unexamined delta.

Make it a diff, not a guess: run `/usr/bin/printenv` (absolute path, no `env` wrapper) as the guest
command from the Verb UI, capture it, and compare with `printenv` from the working `run-as` path.

Two more one-line checks worth taking in the same run, because both differ between a shell and an
Android app process and both can make a TUI exit cleanly:

```sh
/bin/sh -c 'grep -E "SigBlk|SigIgn" /proc/self/status; tty; [ -t 0 ] && echo stdin-tty; [ -t 1 ] && echo stdout-tty'
```

A blocked signal mask is inherited across `execve` and is not reset by it. The app's ART threads
block signals a shell does not. If `SigBlk` differs, that is a real and rarely-considered cause.

Also try launching with `-w /home/verb` instead of `-w /workspace`. If `agy` inspects its working
directory and finds an empty bind, exiting 0 is a reasonable thing for it to do.

---

## 5. Two things that must not ship, whatever the outcome

Both are in the current working tree and neither is about Antigravity.

**API keys are on a command line.** The `.env` forwarding in `QemuAgentRuntimeEnvironment` appends
each key as `-E KEY=VALUE` to the qemu argv. That places every secret in the proot process's
`/proc/<pid>/cmdline` and in any `ps` run inside the guest. Verb's stated boundary is that durable
records never hold command text or credentials, and this puts credentials *into* command text. Pass
them through the process environment array instead of argv.

**API keys are being written to logcat.** The instrumentation in `termux.c` logs every `arg[i]` and
every `env[i]`. Combined with the change above, that writes secrets to the system log, where any
`adb logcat` reader sees them. This is the "cleanup needed" item, and it is more urgent than its
label suggests. Remove both loops before any APK is built for the device, not after the issue closes.

---

## 6. Two risks in what is already marked fixed

**`--link2symlink` was removed globally.** The change that fixed Hermes affects the local userland
for every agent, and Claude Code, OpenCode and DeepSeek were not re-tested in this session. proot's
hardlink emulation exists for the npm and cargo cases, so removing it is plausible and it was
verified on device for cargo. It is still a change to the shared install path, so re-test at least
Claude Code and Codex installs before this is called done. Note that `docs/NEXT_SPRINT.md` already
records a `bun run` failure reproducing both with and without the flag, so that note needs updating
either way.

**The runtime can host exactly one process.** With no `binfmt_misc` and Android blocking guest
`execve`, nothing inside the Agent Runtime can spawn anything. That is not only a bash problem: any
agent that shells out to `git`, a language server or a helper will hit the same wall at the moment
it tries. Whatever the release notes end up claiming about Antigravity should be scoped to what was
actually exercised, because "launches" and "works" are further apart here than usual.

The mechanism designed for exactly this is `proot -q`, which rewrites every guest `execve` to go
back through qemu. `QemuAgentRuntimeEnvironment`'s comment says the ordering "is not interchangeable
with PRoot's own `-q` option", but that reads as an assertion about ordering rather than a recorded
test result. One experiment with `-q` is worth running now that the failure is understood: if it
works, the entire class of problem disappears and the direct-guest-command special case for
Antigravity can be deleted rather than maintained.
