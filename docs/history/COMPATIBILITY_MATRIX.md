# Verb Compatibility Matrix — I-5 evidence

Measured 11 September 2026 on the iQOO/Vivo I2202 (Android 14, Funtouch OS 14, kernel 4.19.152, 6 CPUs
visible to the app), against a fresh install of `0.1.0-beta.12-debug` built from `77cc07f`, data wiped
first. Every command ran inside a real Verb terminal in the app sandbox (`untrusted_app_27`), never
through `run-as`. Classifications are **proposed**; the evidence lines are the part that is settled.

Raw evidence: `~/verb-i5-evidence/` on the host (stage logs, `.meta` records, host memory samples,
screenshots, `findings-log.md`). Driver: `scripts/i5/verb-audit.sh`.

## Pins

| Workload | Revision | Stress |
| --- | --- | --- |
| DAX | `2e399e62143676a32b80dc56452d8506e9818784` | Bun 1.4.0, Rust cargo workspace, turbo monorepo |
| simonw/llm | tag `0.35`, `bd4d3f185e443f52a6919b9aa1a6d5418f88efa2` | pip/venv, Rust wheels |
| create-next-app | `16.3.4` (next `16.3.4`) | npm, native SWC, localhost dev server |
| cli/cli | tag `v2.100.0`, `45437bc7eeeb3359bbfddd1742f79de7652fd3e2` | Go toolchain, sustained compile |

## The matrix

```text
                    Verb Userland (Bionic, native)            Agent Runtime as shipped (glibc, qemu)
DAX                 clone ✓  install ✗ SANDBOX                every stage BLOCKED BY child-exec SIGSYS
                    build/run BLOCKED BY install
simonw/llm          clone ✓  install ✓* (485 s)  build ✓  run ✓   every stage BLOCKED BY child-exec SIGSYS
create-next-app     scaffold ✓  install ✓  build ✓*  run ✓*   every stage BLOCKED BY child-exec SIGSYS
cli/cli             clone ✓  install ✓  build ✓ (82 s)  run ✓   every stage BLOCKED BY child-exec SIGSYS

✓* = passed with the one documented workaround
```

Userland toolchain as installed by `pkg`: node v24.18.0, npm 11.19.1, Python 3.14.6 (preinstalled),
clang 21.1.8, make 4.4.1, rustc/cargo 1.98.1, go1.27.1 android/arm64. All provisioned in under a minute
each.

## Userland findings

### U1 — DAX install: the app seccomp filter kills Bun · proposed **SANDBOX**

Bun itself runs natively (`bun install v1.4.0 (34cbb9a40)`, "Resolved, downloaded and extracted
[189]"), then exits 159 (128 + SIGSYS):

```
F/libc: Fatal signal 31 (SIGSYS), code 1 (SYS_SECCOMP), syscall 437 in tid 7679 (Bun Pool 7)
F/DEBUG: Cause: seccomp prevented call to disallowed arm64 system call 437
F/libc: Fatal signal 31 (SIGSYS), code 1 (SYS_SECCOMP), syscall 452 in tid 8972 (Bun Pool 7)
```

437 is `openat2`, 452 is `fchmodat2`. Kernel 4.19 has neither, so a plain `ENOSYS` would let Bun fall
back; Android's app filter traps instead. Three attempts, identical result: `--frozen-lockfile`, without
it, and `--backend=copyfile` (so the install backend is not the cause). Build and run: **BLOCKED BY
dax/install**.

Not VERB BUG under the agreed rule: Verb does not break an operation the platform allows; the app
sandbox refuses it. Noted for I-6 only: PRoot already intercepts syscalls and could, in principle, turn
this trap into `ENOSYS`. That is a capability question, not a verdict.

The prediction going in was LIBC. It was wrong: the binary executes.

### U2 — llm install: Rust wheels build on device once `ANDROID_API_LEVEL` is set · proposed **VERB BUG** (environment)

Attempt 1 failed after 434 s when `jiter` (a Rust dependency of `openai`) ran maturin:

```
💥 maturin failed
  Caused by: Failed to determine Android API level. Please set the ANDROID_API_LEVEL environment variable.
```

Attempt 2 set `ANDROID_API_LEVEL` from `getprop` and passed in 485 s. PyPI has no Android wheels for
Python 3.14, so `jiter`, `pydantic-core 2.46.5` and `PyYAML` all compiled on the phone
(`pydantic_core-2.46.5-cp314-cp314-android_34_arm64_v8a.whl`). `llm --version`, `llm models list` and
`llm plugins` work.

Classified VERB BUG because the toolchain can do the work and the environment Verb provides omits a
variable the Android Rust/Python ecosystem expects. Severity is low; the cost is not: see R1.

### U3 — Next.js: no native SWC for android/arm64, and Turbopack refuses WASM · proposed **NATIVE MODULE**

`npm install` passed (60 s) but installed no `@next/swc-*` binary. `next build` downloaded
`@next/swc-wasm-nodejs`, then:

```
Error: Turbopack is not supported on this platform (android/arm64) because native bindings are not
available. Only WebAssembly (WASM) bindings were loaded, and Turbopack requires native bindings.
```

`next dev` fails the same way. With Next's own suggested flag, both pass: `next build --webpack` in 66 s,
and `next dev --webpack` answers `HTTP 200 after 15s` (`GET / 200 in 13.1s`). Localhost networking works
in the userland — which also refutes the 10 September note that PRoot isolates loopback.

### U4 — cli/cli: works end to end · no finding

`go mod download` 23 s, `go build -trimpath -o bin/gh ./cmd/gh` 82 s, `gh version
2.0.0-20260903152419-45437bc7eeeb`.

## Agent Runtime findings

### A1 — The shipped Agent Runtime shell cannot execute any program · proposed **VERB BUG**

Opened the supported way (import artifact → install the Agent emulator profile → Retry check →
"Compatibility: runs on this device" → Open agent terminal → Restart). The prompt appears
(`I have no name!@localhost:/workspace$`, bash 5.2.15), builtins and redirects work, and every external
command prints `Bad system call` — `date`, `cat`, `git --version`, `timeout`, `sha256sum`, `du`,
`bash -c true`.

The same shell, running the same binaries explicitly through the emulator, succeeds:

```
LD_LIBRARY_PATH=$F/usr/lib $F/usr/bin/qemu-aarch64 -L / -U LD_LIBRARY_PATH /bin/date      -> Fri Sep 11 05:14:57 UTC 2026
LD_LIBRARY_PATH=$F/usr/lib $F/usr/bin/qemu-aarch64 -L / -U LD_LIBRARY_PATH /usr/bin/git --version -> git version 2.39.5
```

So the guest `bash` is emulated, but the programs it execs are not: they run natively under PRoot,
where glibc binaries die in the app sandbox (consistent with `AGENT_RUNTIME_V1.md`). The launch gives
QEMU to PRoot as the first program rather than routing every exec through it. Because the audit driver
itself needs external programs, **all four workloads are BLOCKED BY A1** in this runtime.

The one workaround — the same rootfs under `proot -q qemu-aarch64`, launched from a userland terminal —
could not start: nested PRoot rejects its own temp directory (`proot error: trying to remove a directory
outside of '<PROOT_TMP_DIR>'`) for all three path spellings tried. A non-nested `-q` launch needs the app
to start it, which is a product change and out of scope here. Recorded in
`scripts/i5/agent-runtime-q.sh`.

### A2 — Emulation cost · proposed **EMULATION-COST**

Measured without child processes, so it runs in the shipped shell. Three runs each, same device, same
scripts (`scripts/i5/bench.sh`, `bench-node.js`):

| Probe | Userland | Agent Runtime (qemu) | Ratio |
| --- | --- | --- | --- |
| pure-bash loop, n=300000 | 0.95 s | 8.19 s | ~8.6× |
| node v24.18.0, `--jitless` | ~930 ms | ~14,000 ms | ~15× |
| node v24.18.0, JIT (userland default) | ~420 ms | `qemu: uncaught target signal 11 (Segmentation fault)`, exit 139 | JIT unusable; ~33× vs jitless under qemu |

Under emulation every V8 tool must run jitless. Applied to the userland timings above — llm install
485 s, Next build 66 s, gh build 82 s — an 8–33× factor puts each beyond the 30-minute stage budget or
near it. That is an extrapolation from micro-benchmarks, not a measured workload, and is labelled so.

## Environment findings outside the four journeys

| # | Observation | Proposed class |
| --- | --- | --- |
| E1 | A fresh install cannot read files another uid created on shared storage (`bash: /sdcard/Download/verb-audit/go.sh: Permission denied`); it can create and read its own. No storage permission is declared. A browser download is unreadable from Verb. | SANDBOX (declared-permission gap is Verb's to decide) |
| E2 | npm 11.19.1 blocks install scripts not covered by `allowScripts`: `bun@1.4.0` (so no real Bun binary was fetched on the first attempt) and `unrs-resolver@1.12.2`. Postinstall-dependent tools fail later, far from the cause. | VERSION |
| E3 | The Agent Runtime card reads "Compatibility: check could not run … installed but unverified" until the separate Agent emulator profile is installed; nothing names the emulator as the missing piece. | VERB BUG (surfacing) |
| E4 | The file picker offers two artifact sets with identical filenames (Aug 16 and Sep 5); mixing them fails checksum verification. | not a runtime finding; UX |

## Memory

Peak total RSS of the app uid, sampled over adb every 5 s (`scripts/i5/host-rss-sampler.sh`,
`stage-peaks.py`). These are sums of per-process RSS, so shared pages count more than once, and stages
shorter than the interval have no sample. Highest: llm install attempt 1 at ~2.58 GB (parallel `rustc`),
Next dev server ~0.81 GB, Next webpack build ~0.70 GB, gh link ~0.64 GB. No stage was killed for memory.

## What this does not establish

- Whether U1 reproduces on newer kernels (5.6+ has `openat2`; the filter may then allow it).
- Agent Runtime workload timings: A1 prevents running them, and A2 is a micro-benchmark factor.
- Anything about the Play flavor, other OEMs, or the release package's own sandbox label.
- Lifecycle: no stage was interrupted during the audit (phone on charger, screen held on — see Track 2).
