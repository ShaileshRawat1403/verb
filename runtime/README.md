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
  curl, and Git over HTTPS.
- `developer-runtime-release` additionally adds Node LTS, npm, and Python. It is a
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

The source workflow persists build caches keyed to the pinned upstream revision and
Verb's runtime patches. These may reuse downloads and intermediates, but they are not a
guarantee of a short rebuild: the upstream bootstrap process clears generated package
output. Do not use a cache hit as justification to retry a multi-hour build.

The workflow deliberately uploads a review artifact only. It does not publish a
release or change the Android app automatically. Before an artifact is accepted,
we must record its SHA-256, review its license manifest, and add a verified
installer to Verb.

## Package-management boundary

The first image is self-contained. `npm` can install pure JavaScript packages
into the user's Verb home. `apt`/`pkg` must not be advertised for arbitrary
post-bootstrap installs until we operate a Verb-specific package repository:
official Termux packages are also path-bound to `com.termux`.

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
