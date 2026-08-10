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

It deliberately excludes Termux's Android-specific `am` and `termux-am-socket`
companions. They are not required for a regular local terminal and depend on an
Android Gradle build that cannot be provisioned safely inside the pinned package
builder. Android activity-management features can be evaluated separately later.

It also uses the base ncurses source's built-in terminfo database instead of
fetching optional X11 terminal metadata. This removes a nonessential external
download (including Codeberg) while retaining a broader terminal-description
set for the embedded local terminal.

The preflight also probes the pinned `apr-util` archive URL. This prevents an
Apache mirror retirement from consuming a full native build before it is seen.
It similarly uses and probes Savannah's mirror router for `attr`, rather than
depending on a single Savannah download host.

The source workflow persists the Termux build and package-output caches using a
key derived from the pinned upstream revision and Verb's runtime patches. A
retry of the same runtime configuration reuses completed native packages rather
than rebuilding the whole dependency graph.

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
