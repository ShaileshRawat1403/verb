# Verb Direct Beta

Verb `v0.1.0-beta.4` is a direct-distribution Android build for arm64 devices. It is not a Google Play build.

> **If you have beta.3 installed:** it reports itself as `0.1.0-beta.2` in Android's app info, and
> so does beta.2. Neither the name nor the version code distinguishes them. Installing beta.4 over
> either is a normal in-place upgrade — same signing key, same application id, `adb install -r` —
> and beta.4 is the first build whose reported version is its real one.

## Install

1. Download `app-fullCli-release.apk` and `verb-full-cli.apk.sha256` from the matching GitHub Release.
2. Verify the APK checksum before installation.
3. Enable Android's permission to install unknown apps for the browser or file manager used to open the APK.
4. Install the APK and open Verb.

## Runtime

- The Full CLI edition installs a proot-backed, Termux-compatible userland on first launch.
- It requires an arm64-v8a Android device and adequate free storage plus network access.
- The initial runtime bootstrap and optional tool profiles can take time on a mobile connection.
- Projects are stored privately under Verb's app data. Create/select a project from the terminal header before cloning or working on a repository.

## Agent Tools

Install JavaScript first in System & setup, then install Codex CLI, Claude Code, or OpenCode. Each
CLI owns and verifies its own sign-in flow. A saved credential file is not proof that its login is
still valid. Verb never exports its Assistant API key to a terminal CLI.

## AI Assistant

Verb supports user-provided OpenAI, Anthropic, Gemini, and OpenAI-compatible API credentials. Keys are encrypted with Android Keystore. Suggested model IDs are local conveniences only; availability depends on the selected provider account and endpoint. Terminal explanation sends only bounded structural lifecycle evidence to the selected provider when explicitly requested; raw PTY output, command text, file contents, transcripts, credentials and absolute paths are excluded.

## Beta Limits

- Full CLI is direct-distribution only because Android's current executable-storage policy prevents this runtime model in a Play-targeted app.
- A selected project is the terminal launch directory. Verb does not infer later shell `cd` changes.
- Agent installation/authentication and long-running process behavior should be treated as beta functionality.
- Working World archives exclude project source trees. Keep every project in Git or another
  independent backup before uninstalling or clearing Verb.
- Claude and Codex recovery have physical-device evidence. OpenCode launches physically, but its
  recovery path and Android↔desktop continuity round-trip are not yet accepted end to end.

## Updates And Support

Install later Verb APKs over the existing app to preserve its private runtime and project data. Do not uninstall Verb unless you intend to remove those files. Include the copied terminal diagnostics report, Android version, device ABI, and release version when reporting an issue.
