# Verb local terminal runtime

Verb's embedded terminal is a real PTY, but Android's `/system/bin/sh` is only a
diagnostic shell. A usable CLI environment needs a native userland built for the
same private Android path as the app: `/data/data/com.aistudio.verb.app/files`.

Do not substitute a stock Termux bootstrap or packages. They are compiled for
`com.termux` and their executables, loader paths, and package metadata will not
work in Verb's app sandbox.

## First runtime image

The manual GitHub Actions workflow `.github/workflows/build-verb-terminal-bootstrap.yml`
builds a pinned aarch64 bootstrap for the connected Vivo device. It includes the
base Termux-compatible shell and utilities plus CA certificates, curl, Git,
Node LTS, npm, OpenSSH, and Python. This gives us the minimum local substrate
for user-installed CLI tools and agents.

The workflow deliberately uploads a review artifact only. It does not publish a
release or change the Android app automatically. Before an artifact is accepted,
we must record its SHA-256, review its license manifest, and add a verified
installer to Verb.

## Package-management boundary

The first image is self-contained. `npm` can install pure JavaScript packages
into the user's Verb home. `apt`/`pkg` must not be advertised for arbitrary
post-bootstrap installs until we operate a Verb-specific package repository:
official Termux packages are also path-bound to `com.termux`.

## Safety boundary

The runtime only executes text authored directly in the Terminal screen. The
assistant, natural-language intents, and provider responses never receive a
path to the PTY. Credentials for CLI tools remain in the local terminal home;
they are not copied to the provider settings store or sent to an AI provider.
