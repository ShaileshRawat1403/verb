# The working world

*Android. Written 22 August 2026, after the export/import path was verified on the validation
device.*

The "working world" is everything on a phone that took effort to create and that Verb cannot
recreate: the agents' logins, `~/.env`, and the session records that make recovery possible. It is
not the Linux userland — that is 279 MB of bytes Verb can download again — and it is not the
terminal's scrollback, which is display state and deliberately never durable.

```text
files/home/.env                     API keys the user pasted
files/home/.claude                  Claude Code's login and history
files/home/.claude.json
files/home/.codex                   Codex's auth and session transcripts
files/home/.config/opencode
files/home/.local/share/opencode
files/agent-runtime/homes/default/.gemini
                                    Antigravity's configuration and sign-in, which live in the
                                    app-owned Agent Runtime home rather than the local userland
shared_prefs/verb_session*.xml      allowlisted Verb session records
shared_prefs/verb_projects.xml      project metadata, not project contents
```

Agents that run under the optional Agent Runtime keep their state in a second home, and until
`0.1.0-beta.8` this list held only the first one. Someone signed into Claude, Codex *and*
Antigravity therefore had an archive that restored two of the three, while this page said it held
"the agents' logins". `WorldCoversSignInTest` now compares the catalog Verb reads sign-in state from
against the paths this archive copies, so an agent added to one and not the other fails a test
rather than a restore.

Only the configuration directory is taken from that home, not the whole of it: `.local/bin` there
holds the installed agent binary, which is large and can simply be fetched again. A backup is for
what cannot be.

Android deletes all of it on uninstall, on "clear storage", and on any install that has to remove
the package first. Before this existed, that cost an evening each time.

Project source directories are intentionally **not** part of Working World. The archive is a
sensitive recovery envelope, not a repository backup: keep project source in Git or another
independent backup. Restoring metadata for a project whose directory is gone cannot recreate that
directory or its files.

## Two commands, and one thing the app does

`verb export` and `verb import` live in `app/src/main/assets/verb/world.sh`, installed into the
guest as `verb`. The app's part is deliberately small: **System → Working world** moves an archive
to Downloads, or brings one back in. It never decides *when* to make one.

```text
verb world list                  what is in the world, and how big
verb export ~/world.vbak         write the archive (asks for a passphrase)
verb import ~/world.vbak         preview only: manifest, checksum, what would change
verb import ~/world.vbak --apply restore, after snapshotting what it overwrites
```

### Why the archive is never automatic

An archive of this world contains live credentials for two paid accounts. Verb-owned metadata could
safely be snapshotted on a timer; this cannot. So the sensitive step is one a person takes
knowingly, with a passphrase they chose:

* **Encrypted** — AES-256-CBC, PBKDF2 with 200,000 iterations, keyed by that passphrase. Losing it
  loses the archive; there is no recovery path, and the command says so before asking.
* **Versioned and checksummed** — a manifest travels *inside* the archive carrying `schemaVersion`,
  `createdAt`, the payload's SHA-256, and the list of paths. Import verifies the checksum before it
  will act.
* **Previewed** — plain `verb import` decrypts, prints the manifest, lists what it *would* replace,
  and exits having changed nothing. `--apply` is a separate decision, and snapshots the current
  world first.

CBC has no authentication tag, so a wrong passphrase can decrypt into garbage rather than fail
cleanly. Import therefore checks that the result is a tar containing a manifest, and treats anything
else as "the passphrase is wrong, or the file is damaged" rather than as data.

### Two things the archive skips

Both were found on the device, not reasoned about. Codex keeps a git object store under
`.codex/.tmp` and proot leaves `.l2s.*` symlink shims; neither can be `stat`ed, and `tar` failing
mid-run would have produced a partial file that still looked like a backup. They are excluded by
name, and a read failure anywhere else aborts the export rather than writing one.

`verb world list` has the matching rule for sizes: a directory it cannot measure prints `size
unknown` and the total becomes "at least". Printing `0 B` would be Verb stating a fact it does not
have.

### The export that its own importer refused

Found on 26 August by running the round-trip this repository had listed as unverified for weeks.

`verb export` wrote an archive, and `verb import` refused it:

```text
verb: the archive contains a link or special file; restore refused.
```

Import rejects any tar member that is not a regular file or a directory, and that refusal is right —
a symlink inside an archive is a path traversal waiting for somewhere to land. The bug was that
export never checked it satisfied that rule, and two sources of symlinks had appeared since the
exclusions were written:

* `.codex/tmp/arg0/*` — Codex's arg0 shims. The existing exclusion said `.tmp`; the real directory
  is `tmp`. One dot, and every archive made by anyone who had run Codex was unrestorable.
* `node_modules/.bin/*` — npm's executable shims, which arrived under `.config/opencode` when
  OpenCode was admitted. Nothing about a person's world lives in them.

Both are derived state that the tools regenerate, so excluding them loses nothing. But adding two
more names to a list is the same fix that failed last time, so export now **verifies** the archive
against import's own rule before it will write one, and aborts saying so if it ever cannot. An
export that produces what import refuses is not a backup; it is a file.

The round-trip then completed: export, Save to Downloads, bring back in through the real system
picker, preview (which listed all ten paths and changed nothing), and `--apply`, which snapshotted
the existing world to `verb-world-before-import-<timestamp>.tgz` before restoring.

There were no unit tests for `world.sh` at all, which is why this shipped.
`WorldArchiveInvariantTest` now covers the invariant, the two exclusions by name, and that export
actually calls the check before encrypting.

### Schema compatibility

The current writer produces schema v2 and the reader accepts v1–v2. A v1 archive may contain a
broad `shared_prefs/` directory, but import restores only the current allowlist: session/project
records. Legacy chat memory, provider ciphertext tied to another Android Keystore, UI preferences
and privacy markers are ignored. Preview lists the allowlisted paths actually staged, not every
member that happened to exist in the older archive.

## Install hygiene

The archive is the recovery path. Not needing it is better, and that is a packaging property:

* **Disposable tests have a different identity.** Debug and instrumentation builds use
  `com.aistudio.verb.app.debug`. Instrumentation cleanup may uninstall that package, but can never
  remove the canonical `com.aistudio.verb.app` release and its Working World.

* **Signing continuity.** Published upgrades keep the canonical application id and release signing
  key, and are installed with `adb install -r`. A local `device` build may replace an existing app
  only when its signing key matches; a signature mismatch is a stop condition, never a reason to
  uninstall the existing app.
* **`isDebuggable = false` on the device build.** `run-as` is refused, so a USB connection can no
  longer read the agents' credentials out of app storage. The trade is that `adb exec-out run-as …`
  is no longer a way to fetch an archive either, which is why Save to Downloads exists.
* **`android:allowBackup="false"`.** Verb's storage holds third-party credentials; it does not belong
  in an automatic cloud backup the user never asked for.

Verified on the device on 24 August: a real encrypted schema-v1 archive was previewed and applied
through the current reader, then upgraded in place to a non-debuggable build. Claude and Codex kept
their conversation identities and resumed in new processes. Codex's saved login remained usable;
Claude's saved files remained present but Claude itself required `/login`, proving why Verb must not
equate a credential file with valid authentication.
