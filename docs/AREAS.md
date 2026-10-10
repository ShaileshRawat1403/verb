# Verb's three areas

One repository, three separately managed areas. Start here to find where something lives, what checks
it, how it ships, and what you must not touch while working on it.

| | **Verb Android** | **Verb Desktop** | **Node 1 (OnePlus host)** |
|---|---|---|---|
| What | The phone app (`com.aistudio.verb.app`) | The Rust host and web workbench (`verb`) | A OnePlus 9 running Verb Desktop for remote use, operated from the private `pocketfabric-node1` repository |
| Code | `app/`, Gradle files, `version.properties` | `desktop/` (Rust in `src/`, browser UI in `web/`) | None. It runs Desktop's code |
| Operations | `scripts/bench/` (device benchmark) | — | Private repo `pocketfabric-node1`: `ops/` (status, deploy, rollback) |
| CI | `.github/workflows/android-ci.yml` | `.github/workflows/desktop-ci.yml` | Desktop CI covers the code |
| Release | `release-full-cli.yml`, tag `vX.Y.Z`, signed APK, draft first | `release-desktop.yml`, tag `desktop-vX.Y.Z`, draft first | `ops/verb/deploy.sh` in `pocketfabric-node1`, logged in its `docs/deployments/verb.md` |
| Current brief | `docs/perf/HANDOFF.md` (benchmark) | `docs/DESKTOP_TERMINALS_BRIEF.md` | `docs/runbook_verb.md` in `pocketfabric-node1` |
| Test device | Vivo I2202 / iQOO over adb | The Mac | The OnePlus itself (`ops/status.sh` in `pocketfabric-node1`) |
| Branch prefix | `android/…` | `desktop/…` | — (node changes are commits in `pocketfabric-node1`) |

## Rules that keep them separate

- **One area per change.** A commit or PR touches one area's paths. The exception is the shared
  contract below, and such a change says so in its message.
- **CI is path-filtered.** Android CI runs only for Android paths and Desktop CI only for `desktop/`.
  An Android release waits only on Android checks, so a desktop lint can never block a phone release
  again (as it did on 2026-10-04).
- **Versions are independent.** Android is `version.properties`, Desktop is `desktop/Cargo.toml`.
  Tags never collide (`v…` vs `desktop-v…`).
- **Toolchains are pinned.** Desktop CI and releases use Rust `1.99.0`. Bump it deliberately, in its
  own commit.

## The shared contract (changes here touch both)

Android and Desktop meet in a few documented places. Change these only with both sides' tests green:

- `docs/VERB_SESSION_CONTRACT.md` and `docs/VERB_SESSION_SCHEMA.md`: session states and records
- `docs/DESKTOP_MOBILE_BRIDGE_PROTOCOL.md`: phone ↔ desktop pairing and live terminal control
  (`desktop/src/mobile.rs`, `desktop/src/phone.rs` ↔ `app/src/main/java/com/example/verb/mobile/`)
- `docs/VERB_CONTINUITY_ENVELOPE.md`: the `.vcont` evidence format

## Secrets

None in this repository. Node 1's token and Cloudflare Access settings live only on the node (see
the private `pocketfabric-node1` repository). Android signing keys live only in GitHub Actions secrets.
