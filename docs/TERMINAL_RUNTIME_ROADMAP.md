# Terminal Runtime Roadmap

## Why Termux is here

Verb does not depend on the Termux Android application, its services, or its UI. Verb uses
Termux-derived components because Android does not provide a general Linux userland:

- `TerminalView`, `TerminalSession`, and the PTY JNI provide terminal emulation and a real PTY.
- The Termux bootstrap provides Bash, coreutils, Git, curl, certificates, apt, and dpkg.
- `libtermux-exec` fixes Android/Termux command execution behavior inside the guest.
- `proot` maps Verb's private files into the paths expected by the prebuilt userland.

The userland runs under Verb's UID and private storage. No Termux app installation is required.

## Product dependency boundary

Required for the current local CLI runtime:

- arm64 Termux-compatible bootstrap
- Bash, coreutils, env, login, certificates, apt/dpkg, and Git
- `libtermux-exec`
- static `proot`
- Android PTY JNI and terminal emulator

Optional capability packages:

- Python and pip for Python applications
- Node.js and npm for JavaScript applications
- clang, make, cmake, pkg-config, and Rust for native development
- OpenSSH for remote shells and Git over SSH
- jq, ripgrep, ffmpeg, and database clients for common workflows
- bubblewrap only when the tool's sandbox requires it and Android permits it

## Distribution variants

Verb ships two deliberately different variants:

1. `fullCli`: direct-distribution build targeting API 28. It supports the proot-backed,
   Termux-compatible userland and apt-installed ARM64 CLI packages.
2. `play`: Google Play build targeting API 36. It never provisions or executes the writable
   userland because modern Android blocks that execution model. It provides the Android system
   shell while a modern package execution backend is developed.

The variants use distinct package IDs so they can coexist on a device. Do not claim that the Play
variant supports arbitrary apt-installed binaries until the execution backend is replaced.

## What "run all code" means

Verb can support ARM64 Android/Linux command-line programs that do not require a privileged
kernel feature or a desktop display server. It cannot transparently run:

- x86, Windows, macOS, or desktop GUI binaries
- programs requiring root, Docker, kernel modules, or unrestricted namespaces
- daemons that need to bind privileged ports or survive independently of the app
- Android APIs unless an explicit Android bridge is provided

These limits should be surfaced as capability status, not discovered as opaque shell failures.

## Runtime profiles

The base profile stays small and reliable. Additional profiles are installed through apt and
reported by capability checks:

1. Core: Bash, POSIX tools, Git, curl, certificates, apt/dpkg.
2. Python: Python, pip, venv support, and common build prerequisites.
3. JavaScript: Node.js, npm, and native build prerequisites.
4. Native: clang, make, cmake, pkg-config, Rust, and linker headers where available.
5. Remote: OpenSSH client and known-host/key management.
6. Media/data: jq, ripgrep, ffmpeg, SQLite, and compression tools.

Profile installation must be transactional, resumable, and versioned. A failed profile must never
replace a working base runtime.

Verb owns the profile catalog and preflight reports in `RuntimeProfiles.kt`; apt/dpkg remains the
low-level package installer inside the guest. This split matters because apt can resolve Debian
package dependencies, while application environments (like Python agent venvs) isolate dependencies
and configure necessary toolchains (such as Rust and C toolchain flags for native ARM64 wheels). Hermes
serves as the primary Python agent model: installed in an isolated venv with native cryptography and
a Verb-owned dynamic launcher that resolves its declared entry point without touching package paths.

## Robustness requirements

- Verify archive provenance and SHA-256 before extraction.
- Extract into a staging directory and activate only after required files validate.
- Serialize bootstrap installation and retry operations.
- Keep PTY lifecycle changes on the main thread and bound all diagnostic subprocesses.
- Never log raw commands, credentials, or full terminal transcripts by default.
- Expose session state, exit status, working directory, PATH, and installed capabilities.
- Keep the Android system shell as a truthful diagnostic fallback, not as a fake full runtime.
- Keep the `fullCli` target SDK 28 until its execution model moves binaries out of writable app
  storage; newer targets block the current Termux-style execution model on the device.

## Next implementation phases

1. Add a validated runtime manifest and capability detector.
2. Add profile install/remove/repair operations backed by apt transactions.
3. Add command boundaries, exit status, job/process tracking, and dynamic working directory.
4. Add SSH and Android bridge capabilities with explicit permission boundaries.
5. Replace target-28 writable-storage execution with a modern supported execution architecture before
   raising target SDK.
