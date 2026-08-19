# Third-Party Licenses

Verb embeds a locally-built Linux userland and the proot wrapper so the Terminal is a real
PTY environment with no runtime dependency on another Android app. The components below are
distributed with the app; the obligations of their licenses are addressed here.

## Components and licenses

| Component | License | Notes |
|-----------|---------|-------|
| termux-packages (build framework + packaged binaries) | GPL-3.0-or-later | Upstream `termux/termux-packages` pinned at commit `bd2e956e639090779a728c96fcca2e387dd3a246`, patched under `runtime/termux-packages/`. |
| termux-tools | GPL-3.0-or-later | Shell scripts shipped in the userland. |
| libtermux-exec | GPL-3.0-or-later | ELF preload (`libtermux-exec.so`) that makes `execve` resolve guest shebangs and loaders. |
| termux-core | MIT | Binary-side companion package (`com.aistudio.verb.app` private paths). |
| proot | GPL-2.0-or-later | Statically linked v5.3.0 wrapper bundled as an app asset (`assets/proot-arm64-v8a`). |
| GNU tar | GPL-3.0-or-later | Termux package 1.35-3, bundled as an app asset (`assets/gnu-tar-aarch64`) and installed over the bootstrap's toybox `tar` when needed so dpkg can unpack packages. |
| Per-package libraries and tools (git, curl, bash, coreutils, apt, dpkg, openssl, etc.) | Each under its own license (GPL-2.0, GPL-3.0, LGPL-2.1/3.0, MIT, BSD, Apache-2.0, ISC, MPL-2.0, CC0, public domain, ...) | Full license texts ship inside the userland. |

Full license texts are installed in the userland under `usr/share/LICENSES/` (all SPDX
variants used by packages) and `usr/share/doc/<package>/` (per-package notices), and the
installed package list is recorded in the release artifact's
`verb-runtime-aarch64-packages.txt`.

The termux-packages scripts are governed by GPL-3.0-or-later; termux-tools and
libtermux-exec by GPL-3.0-or-later; proot by GPL-2.0-or-later. These are copyleft licenses
that require, on request, access to the corresponding source of the GPL-covered portions.

## Source code

The corresponding source for the GPL-covered portions is available from:

- termux-packages (including termux-tools, libtermux-exec build recipes):
  <https://github.com/termux/termux-packages> at the pinned commit above, with Verb's
  modifications under `runtime/termux-packages/`.
- proot: <https://github.com/proot-me/proot> (v5.3.0).
- Verb's build workflow and patch set: <https://github.com/ShaileshRawat1403/verb>.

## Written offer

You may request the corresponding source as shipped in any Verb release by opening an issue
at <https://github.com/ShaileshRawat1403/verb/issues>; we will provide it in machine-readable
form within a reasonable time and without charge except for the cost of conveyance.

## musl libc dynamic loader (`ld-musl-aarch64.so.1`)

Bundled as an app asset and installed into Verb's prefix by `MuslLoaderBootstrap`.

Several agent CLIs (Claude Code, OpenCode, and other Bun-compiled tools) publish `linux-arm64-musl`
builds but no Android build. Those binaries are valid aarch64 ELFs whose only unmet dependency on
Android is their interpreter, `/lib/ld-musl-aarch64.so.1`. Bundling that one file is what lets them
run; nothing is patched or rebuilt.

- Upstream: musl libc, <https://musl.libc.org/>
- Source package: Alpine Linux `musl` 1.2.6-r2 (aarch64)
- License: MIT

## musl C++ runtime (`libstdc++.so.6`, `libgcc_s.so.1`)

Bundled alongside the musl loader and installed into `usr/lib/musl` by `MuslLoaderBootstrap`.
Some musl agent builds (OpenCode) link against the musl C++ runtime, which is not ABI-compatible
with Bionic's. They are reached only through the generated agent wrapper's `LD_LIBRARY_PATH`, never
Verb's own library path.

- Source packages: Alpine Linux `libstdc++` and `libgcc` 15.2.0-r5 (aarch64)
- License: GPL-3.0-or-later WITH GCC-exception-3.1
