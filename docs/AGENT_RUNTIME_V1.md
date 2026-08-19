# Verb Agent Runtime v1

Verb has two execution environments:

1. The existing Verb CLI userland, built for Android and used by the normal terminal.
2. The Agent Runtime, a replaceable ARM64 Linux rootfs for Linux-native CLI agents.

Claude Code and OpenCode belong in the second environment. They must not be launched by nesting
another proot inside the existing Verb userland. The Android PTY host selects one backend directly.

The Agent Runtime is an isolated filesystem and compatibility environment, not a security sandbox.
PRoot does not provide kernel namespaces, cgroups, seccomp, or a security boundary. Verb limits the
design with app-private storage, a selected project bind mounted at `/workspace`, separate agent
homes, signed/hashed artifacts, and explicit user launch.

Each artifact carries a manifest declaring its immutable runtime version, ARM64 architecture,
rootfs digest, Linux distribution, Node version, Claude version, OpenCode version, and required
absolute command paths. Installation is staged under `agent-runtime/versions/<version>/rootfs` and
activated only after verification. The active version is never overwritten in place.

The implementation order is intentionally narrow:

1. Prove one pinned Debian-style ARM64 rootfs can boot `/bin/bash`, Node, npm, networking, and Git
   through a direct Agent Runtime PTY.
2. Install and smoke-test the pinned Claude Code and OpenCode versions inside that rootfs.
3. Add transactional download, activation, rollback, and diagnostics.
4. Expose one explicit Agent Runtime launcher in the existing terminal UI.

The normal Verb CLI runtime remains unchanged while this work is in progress.

## Backend evidence (physical device: Vivo I2202, Android 13, arm64)

This section records what was measured, what it proves, and what it does not. It exists because an
earlier conclusion in this investigation was wrong and was corrected by later evidence.

### PRoot backend: cannot execute glibc guests in the app sandbox

Launching the Debian rootfs through PRoot from the Android app process fails immediately:

- PRoot itself starts (`proot --version` exits 0 in-app).
- Any `proot -r <rootfs> <glibc binary>` exits 255, silently, with no PRoot diagnostic.
- The identical argv, environment, host working directory and binds succeed under `run-as`.
- No PRoot failure mode reproduces 255: a missing guest binary, a bad rootfs and an unwritable
  `PROOT_TMP_DIR` all exit 1.
- `PROOT_NO_SECCOMP=1`, which disables PRoot's own seccomp acceleration, does not help.

Verb's launch contract was verified correct and is not the cause: 32 argv entries with no duplicated
`argv[0]`, 7 environment entries, an existing host working directory, and a PTY layer that reaches
`execvp` (proved by a marker printed from the child before the guest is started).

### Correction: the zygote seccomp filter is not the proven cause

An earlier revision of this document attributed the failure to the Android zygote seccomp filter,
on the strength of a process-state differential:

| Fact | In-app child | `run-as` |
| --- | --- | --- |
| `Seccomp` | 2 (filter installed) | 0 |
| `CapBnd` | `0000000000000000` | `00000000008000c0` |
| SELinux context | `untrusted_app_27` | `runas_app` |

That inference does not survive the QEMU result below: QEMU executes the same glibc binaries
successfully **in the same app process, under the same seccomp filter and the same SELinux domain**.
Seccomp therefore cannot be the discriminator on its own.

What the evidence supports is narrower: the app sandbox blocks PRoot's mechanism -- native execution
of glibc binaries under `ptrace` -- while permitting emulation. Which specific restriction is fatal
was never uniquely identified, and no product surface claims one.

### QEMU user-mode backend: executes the same binaries

`qemu-user-aarch64` 11.0.3, running in the same app process, executes the rootfs Verb's PRoot
backend could not:

| Guest binary | Result | Startup |
| --- | --- | --- |
| `/bin/bash` (Debian glibc) | `GNU bash 5.2.15(1)-release` | under 1s |
| `node` | `v24.18.0` | under 1s |
| Claude Code | `2.1.233` | ~5s |
| OpenCode | `1.18.18` | ~14s |

Three launch requirements were established by re-running the probes from scratch, and are not
optional:

1. The agent entry points (`claude.exe`, `opencode.exe`) are Bun-compiled ELF binaries, not Node
   scripts. They must be executed directly; invoking them through `node` segfaults.
2. The guest environment must be set with QEMU's own `-E`/`-U` flags. Verb's Bionic
   `LD_LIBRARY_PATH`, which the QEMU executable itself needs, must not reach the guest or the glibc
   loader fails with `libc.so: cannot open shared object file`.
3. `HOME` must point at the writable persistent agent home. With `HOME` inside the read-only rootfs,
   OpenCode aborts with `EROFS: read-only file system, mkdir '/root'`.

### What this does not yet establish

These are startup measurements of `--version`, not workload measurements. Untested: external DNS,
HTTPS and certificate validation; interactive TUI rendering; authentication persistence; access to
the selected project; agent-launched subprocesses; resume across restart; and memory, battery and
thermal cost under sustained use.

Emulation carries a constant-factor cost that a version probe does not reveal. Whether the QEMU
backend is a product or only a demonstration depends on the cost of one real agent turn, which has
not been measured. That measurement should precede any commitment to a production QEMU backend.

## Working backend: PRoot for paths, QEMU for execution

Neither tool alone is sufficient, and the reason is different in each case.

PRoot alone cannot execute the rootfs: a glibc binary launched from the app process exits 255.
QEMU alone can execute it, but `qemu-user` does not chroot -- it only redirects the ELF interpreter
-- so the guest sees Android's filesystem, where `/etc/resolv.conf` and `/etc/ssl/certs` do not
exist. The glibc resolver falls back to `127.0.0.1:53` and every lookup fails `ECONNREFUSED`.

Pairing them works:

```
PRoot   filesystem view: /etc/resolv.conf, CA bundle, /workspace, /home/verb
  -> QEMU   execution: emulates the aarch64 glibc binaries
    -> agent
```

PRoot's first exec is QEMU, a Bionic binary Android runs natively. QEMU emulates everything after
it, so the kernel is never asked to execute a glibc binary directly. This is one layer, not nested
PRoot, and it is not PRoot's `-q` option (which resolves the emulator path on the host before the
guest root exists, and did not work here).

Details that are easy to rediscover painfully:

1. The app directory must be bound onto itself. QEMU is launched by absolute host path, so that
   exact path must also resolve inside the guest, or PRoot reports `execve(...): No such file or
   directory`.
2. `/linkerconfig` must be bound from Verb's app-local copy; QEMU is a Bionic binary and its loader
   needs it.
3. Guest environment is set with QEMU's `-E`/`-U`, never inherited: Verb's Bionic `LD_LIBRARY_PATH`
   reaching the guest breaks the glibc loader with `libc.so: cannot open shared object file`.
4. Node needs `--jitless`; `JSC_useJIT`/`BUN_JSC_useJIT` are JavaScriptCore settings and cover only
   the Bun-compiled agents.

Measured on the validation device, inside Verb: an interactive Debian shell at `/workspace`, DNS
resolving, TLS verifying against the guest CA bundle (`authorized=true`, TLSv1.3), and `curl`
reaching an external HTTPS endpoint.

### Open: agents die with SIGSYS on a TTY

The agent launchers behave differently depending on how they are started, in the same app process
with the same seccomp filter:

| How it is launched | Result |
| --- | --- |
| Non-interactively (bounded probe, pipes) | `claude --version` exits 0 |
| Interactively in Verb's PTY | `Bad system call` (SIGSYS) |
| Under `run-as`, which carries no app seccomp filter | exits 0 either way |

So the earlier "seccomp cannot be the discriminator" note was itself too strong. What the evidence
now supports: Android's seccomp policy permits the narrow syscall set `bash` uses, and refuses at
least one syscall the Bun-compiled agents make when stdout is a terminal. QEMU emulates CPU
instructions but passes syscalls through to the real kernel, so the app's filter still applies.

Which syscall is refused has not been identified, and no product surface claims one. Until it is,
the interactive agent path is unproven, and the compatibility probe deliberately runs an agent
launcher rather than `bash`, which would answer "yes" on a device where no agent can run.
