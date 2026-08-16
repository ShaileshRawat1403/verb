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
