# Page-only release (design)

Status: proposed, not built. Approve this before any code.

## Problem

The web page (`desktop/web/index.html`, `dist/app.js`, `dist/app.css`, the favicon and fonts) is
compiled into the `verb` binary with `include_bytes!` (`desktop/src/web.rs`, the `GET` asset
match). Any page change therefore ships as a new binary through the full release:

- CI
- tag and four-platform build
- staging
- candidate rehearsal
- a backup with service restarts
- plan, apply, verify

rc.11 and rc.12 changed no Rust, yet each took over an hour and closed every open terminal on
public with about four minutes of downtime.

## Goal

Ship a change that touches only the page in minutes, with no host restart, no downtime and an
instant rollback, and without weakening what the full release proves. Anything that touches the
host keeps the full release.

## What counts as page-only

A change is page-only when nothing that builds into the host changed since the running binary's
`sourceRevision`:

```
git diff --quiet <host revision> <page revision> -- desktop/src desktop/scripts desktop/build.rs desktop/Cargo.toml desktop/Cargo.lock
```

Changes to any of those paths (Rust, the bridge script that is compiled in with `include_str!`,
the build script or dependencies) need the full release. The release workflow enforces this rule;
it is not left to judgement.

## Design

### 1. Host: an optional page folder

- **The setting:** a new `VERB_PAGE_DIR` environment variable. Unset, the default, keeps today's
  behaviour exactly: the built-in page is served. Desktop users and local runs never see a change.
- **The layout** when set (on Node 1: `/var/lib/verb/page`):

  ```
  /var/lib/verb/page/
    releases/<page revision>/{manifest.json, index.html, app.js, app.css, favicon.svg, fonts/*.woff2}
    current -> releases/<page revision>
  ```

- **The manifest:**

  ```json
  {
    "format": 1,
    "pageRevision": "<git commit of the page>",
    "hostRevision": "<sourceRevision of the binary it may run on>",
    "files": { "index.html": "<sha256>", "app.js": "<sha256>", "...": "..." }
  }
  ```

- **The host serves the folder only if all of these hold. Otherwise it falls back to the built-in
  page and logs one line saying why:**
  1. `hostRevision` equals the binary's own `VERB_BUILD_REVISION`, so a page can never run on a
     host it was not paired with.
  2. Every file in the manifest exists and its SHA-256 matches. The served set is exactly the
     manifest's: the same paths the built-in page serves today, nothing extra.
  3. Permissions: the folder, `current` and each file are owned by root, and nothing is
     group-writable, world-writable or writable by the service's own user. The page has full
     authority over terminals in the browser, so it gets the same integrity bar as the binary.
- **Picking up a switch without a restart:** on each request for `/`, the host reads where
  `current` points. If the target changed, it loads and checks the new release once and keeps the
  verified bytes in memory. Other assets are served from that in-memory set, so one page view
  never mixes two releases.
- **No caching issue:** every asset is already sent with `Cache-Control: no-store`, so a reload
  always fetches the new files.
- **Health:** `/healthz` gains a `page` field, for example
  `{"source": "folder", "revision": "<page revision>"}` or `{"source": "built-in"}`. Verify and
  rollback check against it.

### 2. Build: a page bundle from CI

- **New workflow:** `release-page.yml`, triggered by a tag `page-<short page revision>` with an
  input `host_revision`. It:
  1. Enforces the page-only rule above, and fails otherwise.
  2. Runs the same desktop CI as the full release. The e2e suite builds the host at the page
     revision; because the host paths are unchanged, that host is equivalent to the deployed one.
  3. Runs `npm run build`, writes `manifest.json`, packs `verb-page-<revision>.tar.gz` plus its
     `.sha256`, and attaches them to a draft release, as the desktop releases do.
- **Time:** about 10 to 15 minutes, nearly all of it CI. There is one artifact, not four.

### 3. Node: page modes in `release.sh` (pocketfabric-node1)

| Mode | What it does |
|---|---|
| `page-plan --service public\|candidate` | Checks the tarball hash, checks that the manifest's `hostRevision` equals the live `/healthz` `sourceRevision`, and shows the current and new page revision. Changes nothing |
| `page-apply` | Extracts into `releases/<revision>` (root-owned, read-only), checks every file hash, then swaps `current` atomically (`ln -sfn` to a temp name, then `mv -T`). No restart |
| `page-verify` | Checks that `/healthz` reports `page.revision` equal to the new revision and that `/` returns 200 |
| `page-rollback` | Points `current` back to the previous release, or removes it to return to the built-in page. Takes seconds |

- **Order:** candidate first (`page-plan`, `page-apply`, `page-verify`, a look at the page), then
  public.
- **Steps not needed:** the quiet check and backup are not needed, because no process restarts
  and no data is touched. The binary, the run file, leases and the recovery helper are untouched.

### 4. One-time setup (this one goes through the full release)

1. The host change above, released as a normal rc with the full cycle.
2. A one-time change to the run file on the node for both services:
   `export VERB_PAGE_DIR=/var/lib/verb/page` (public) and `/var/lib/verb/page-candidate`
   (candidate). This is a node service change and needs its own approval.
3. The `release.sh` page modes and runbook, as a records PR.

After that, page changes use this path. A full release resets the pairing: a new binary has a new
`hostRevision`, so an old page folder stops matching and the host falls back to its built-in page
until a matching page is applied. The full release should therefore clear `current` as part of
apply.

## Risks and answers

| Risk | Answer |
|---|---|
| A page that calls an API the host lacks | Impossible by construction: the host paths must be unchanged since the running binary, and the host refuses any other `hostRevision` |
| Tampered files on the node | The host checks hashes when it loads a release and serves only root-owned, non-writable files; otherwise it uses the built-in page |
| A half-written release | Files are extracted and checked before the atomic `current` swap; the host checks again before serving |
| Drift between what was tested and what runs | CI builds the bundle from the tag. The node checks the tarball hash against the release, then every file hash |
| Mixed versions within one page view | The host serves all assets from one verified in-memory set |
| Open tabs | They keep the old page until reloaded, which is safe because the host API is unchanged |
| Something goes wrong | `page-rollback` in seconds, or remove `current` to return to the built-in page |

## Not in scope

- Desktop app downloads: they keep the built-in page, and `VERB_PAGE_DIR` stays unset.
- Hot-swapping Rust: host changes always take the full release.
- Renaming `studio-*` files.

## Tests

- Rust unit tests on the folder loader:
  - accepts a matching folder
  - falls back on a wrong `hostRevision`, a hash mismatch, an extra or missing file, a writable
    file or folder, or a dangling `current`
  - picks up a `current` switch without a restart
- e2e: run the host with `VERB_PAGE_DIR` pointing at a built bundle. Check `/healthz` `page.source`
  and that the page loads; switch `current` and check that the next load serves the new revision.
- Workflow: a tag whose diff touches `desktop/src` must fail the page-only rule.
