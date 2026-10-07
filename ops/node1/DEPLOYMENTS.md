# Node 1 deployments

One line per install or rollback, appended by `deploy.sh` and `rollback.sh`. Newest last.
Entries before this file existed are reconstructed from `docs/WEB_UX_DEPLOYMENT.md`.

- 2026-10-01 `9ef6e50` (+ asset-cache fix) sha256 `c70e1f49…`, restarted
- 2026-10-04 Phase 1 streaming, uncommitted tree, sha256 `00c342b1…`, restarted
- 2026-10-07 Phase 2 and latency fix, uncommitted tree, sha256 `a912fc0b…`, restarted
- 2026-10-07 **unrecorded** install found by `status.sh`: sha256 `bc06dfe7dd9abb48faf8dde63104d0367ce161664ed7de97d4619bea7bfd1d95`
- 2026-10-07 `37a0424` sha256 `2019f89fde897b65f544b7947eace6dccff56960496f36f64579d21722a119ce`, installed, not restarted
- 2026-10-07 token rotated on the node (`/root/.verb/web.token`; new fingerprint `d37ed9f99837`, value never left the node)
- 2026-10-07 restarted onto `37a0424` (sv restart stalled; terminated only the `verb web` child, runit relaunched it)
- 2026-10-07 `71b4500` sha256 `30956a2479a904af64bd073e48394bd1e6331693d824a76405303af986390150`, restarted
- 2026-10-07 restarted onto `71b4500` (sv restart stalled again; stopped only the `verb web` process)
- 2026-10-07 `66f214e` sha256 `ef1bc10b510fa525d17da4ed218bf767bbed38ef185f16e8dfda20886782ffaa`, restarted
