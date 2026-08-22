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
shared_prefs                        Verb's own session and project records
```

Android deletes all of it on uninstall, on "clear storage", and on any install that has to remove
the package first. Before this existed, that cost an evening each time.

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

## Install hygiene

The archive is the recovery path. Not needing it is better, and that is a packaging property:

* **One signing key.** The `device` build type initialises from `debug` and keeps `debugConfig`, so
  `adb install -r` upgrades in place across build types instead of forcing an uninstall — which
  would take the world with it.
* **`isDebuggable = false` on the device build.** `run-as` is refused, so a USB connection can no
  longer read the agents' credentials out of app storage. The trade is that `adb exec-out run-as …`
  is no longer a way to fetch an archive either, which is why Save to Downloads exists.
* **`android:allowBackup="false"`.** Verb's storage holds third-party credentials; it does not belong
  in an automatic cloud backup the user never asked for.

Verified on the device on 22 August: upgraded in place from the debug build, world intact, both
agents still signed in, both able to resume the sessions they had before the upgrade.
