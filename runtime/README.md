# Verb local terminal runtime

Verb's embedded terminal is a real PTY, but Android's `/system/bin/sh` is only a
diagnostic shell. A usable CLI environment needs a native userland built for the
same private Android path as the app: `/data/data/com.aistudio.verb.app/files`.

Do not substitute a stock Termux bootstrap or packages. They are compiled for
`com.termux` and their executables, loader paths, and package metadata will not
work in Verb's app sandbox.

## First runtime image

The manual GitHub Actions workflow `.github/workflows/build-verb-terminal-bootstrap.yml`
has three deliberate modes:

- `preflight-only` (the default) validates source pins, patches, and known upstream
  downloads in seconds. It never compiles a runtime.
- `core-shell-git-release` is a one-time release build for the shell, certificates,
  curl, Git over HTTPS, and GNU `tar`. It intentionally excludes Node LTS and npm; the pinned
  upstream bootstrap's `command-not-found` dependency closure currently includes Python.
  GNU `tar` is Essential and is required by `dpkg` to unpack packages (`apt` itself does
  not depend on it), so it must ship in every bootstrap.
- `developer-runtime-release` additionally adds Node LTS and npm (and explicitly
  verifies Python). It is a
  release-engineering operation, not a normal development command, and can take hours.

OpenSSH is intentionally not in either first-release profile. Run #20 demonstrated why:
after the native Python and Node compilation completed, its optional Kerberos source
download from `kerberos.org` timed out. SSH will be an independently built and verified
add-on rather than a late network dependency that invalidates the whole runtime.

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

The source workflow separates its caches deliberately. Compiled sources and
intermediates may be reused across profiles, while generated package output is cached
only for the exact runtime profile that produced it. The upstream bootstrap script
extracts every package in its output directory, so sharing that directory would silently
mix packages (for example, Node and Python) into the core Git image. Every completed
archive is checked against its selected profile and is accompanied by a package manifest
and SHA-256 file before upload. The profile policy is also exercised with small fixture
manifests in the seconds-long preflight job, before any source compilation begins. Caches
can reduce work but do not guarantee a short rebuild; do not use a cache hit as a reason
to retry a multi-hour source build.

For the first run after this cache separation, the workflow may reuse the old combined
cache's compiled-source directory, but explicitly discards its generated package output
before building. This is a one-time migration optimization: it reuses verified native
work without inheriting the prior profile-mixing bug. The migration runs only when no
current cache matched; this relies on the cache action's matched-key signal rather than
its `cache-hit` flag, which is false for prefix-key restores, and is covered by the fast
preflight check.

The completed archive is written directly to the workflow's writable output mount before
artifact upload. The package-builder's source checkout is read-only to its unprivileged
build user; moving the finished archive into that checkout caused the otherwise successful
core build in run #21 to be lost.

The workflow deliberately uploads a review artifact only. It does not publish a
release or change the Android app automatically. Before an artifact is accepted,
we must record its SHA-256, review its license manifest, and add a verified
installer to Verb.

## Package-management boundary

The first image is self-contained. `npm` can install pure JavaScript packages
into the user's Verb home. Verb also publishes a package registry on GitHub
Pages: `https://shaileshrawat1403.github.io/verb/apt/`, a flat apt repository
served from the `gh-pages` branch (`apt/Packages` + `apt/*.deb`).

The bootstrap's on-device `sources.list` carries both the official Termux
repository (for base runtime dependencies such as `clang`, which `dpkg-perl`
requires) and Verb's flat registry (for Verb-specific packages), added as a
trusted source because the minimal userland omits `gpgv`-based signature
verification of arbitrary repos:

```
deb https://packages-cf.termux.dev/apt/termux-main/ stable main
deb [trusted=yes] https://shaileshrawat1403.github.io/verb/apt/ ./
```

New Verb packages are published by rebuilding the flat repo with
`runtime/scripts/build-verb-apt-repo.py --out apt/`, then pushing the
`gh-pages` branch. Packages install into the Verb prefix
(`/data/data/com.aistudio.verb.app/files`) like the bootstrap itself, so
official Termux packages (whose metadata is path-bound to `com.termux`) must
only be installed from inside the proot userland, where Verb's files directory
is bound onto the `com.termux` guest path.

## Runtime independence

Verb never requires the Termux Android app, its companion apps, an external service, or
the `com.termux` private directory. The embedded terminal code is vendored into Verb and
the optional userland is built for Verb's application id. Those implementation sources
are not a runtime dependency on a separate Termux installation.

## Safety boundary

The runtime only executes text authored directly in the Terminal screen. The
assistant, natural-language intents, and provider responses never receive a
path to the PTY. Credentials for CLI tools remain in the local terminal home;
they are not copied to the provider settings store or sent to an AI provider.
