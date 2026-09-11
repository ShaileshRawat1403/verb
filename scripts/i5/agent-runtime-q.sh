#!/data/data/com.aistudio.verb.app/files/usr/bin/bash
# I-5 WORKAROUND LAUNCH -- not the shipped Agent Runtime.
#
# Mirrors QemuAgentRuntimeEnvironment's binds and guest environment exactly, with one difference:
# QEMU is handed to PRoot's `-q` instead of being the program PRoot starts. In the shipped launch
# the guest bash is emulated but every program it execs runs natively and dies with SIGSYS; with
# `-q`, PRoot routes each guest exec back through QEMU.
#
# Two further, deliberate differences, both forced by `-q` re-running QEMU for every exec:
#   - the guest environment is set once, on the PRoot process, not with QEMU `-E` flags -- `-E`
#     would be re-applied at every exec and silently reset any PATH a build extends;
#   - Verb's files are also bound at /data/data/com.termux/files, where QEMU's RUNPATH points, so
#     each child QEMU finds its Bionic libraries without LD_LIBRARY_PATH leaking into glibc guests.
#
# Run from a Verb *userland* terminal in the project:  bash ./agent-runtime-q.sh [command...]
# Evidence recorded under this launch is labelled agent-runtime+proot-q.

set -u
# Paths as the userland terminal sees them. PRoot runs nested under the userland's own PRoot here, and
# its temp-directory check rejects /data/user/0/... because the outer view canonicalises it as
# /data/data/com.aistudio.verb.app/....
F="${PREFIX:?run from a Verb userland terminal}"; F="${F%/usr}"
APP="${F%/files}"
ROOTFS=$F/agent-runtime/versions/0.1.0/rootfs
HOMEDIR=$F/agent-runtime/homes/default
PROJECT="${VERB_AUDIT_PROJECT:-$F/projects/i5-audit-a1407469}"
QEMU=$F/usr/bin/qemu-aarch64

mkdir -p "$HOMEDIR/.local/bin" "$HOMEDIR/.tmp"

args=(-r "$ROOTFS")
for p in /dev /proc /sys /system /apex /vendor; do [ -d "$p" ] && args+=(-b "$p"); done
[ -f "$F/linkerconfig/ld.config.txt" ] && args+=(-b "$F/linkerconfig:/linkerconfig")
args+=(-b "$APP:$APP" -b "$F:/data/data/com.termux/files")
[ -f "$F/usr/etc/resolv.conf" ] && args+=(-b "$F/usr/etc/resolv.conf:/etc/resolv.conf")
args+=(-b "$PROJECT:/workspace" -b "$HOMEDIR:/home/verb" -w /workspace)
args+=(-q "$QEMU -L / -U LD_PRELOAD -U LD_LIBRARY_PATH")

if [ $# -eq 0 ]; then set -- /bin/bash; fi

exec env -i \
  TERM=xterm-256color COLORTERM=truecolor LANG=C.UTF-8 SHELL=/bin/bash \
  HOME=/home/verb PATH=/home/verb/.local/bin:/usr/local/bin:/usr/bin:/bin TMPDIR=/home/verb/.tmp \
  PROOT_TMP_DIR=/data/data/com.termux/files/usr/tmp QEMU_CPU=cortex-a76 VERB_AUDIT_VARIANT=proot-q \
  "$F/usr/bin/proot" "${args[@]}" "$@"
