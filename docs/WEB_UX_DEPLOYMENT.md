# PocketFabric Node 1 deployment — 1 October 2026

The user authorized deploying and restarting only Verb. The deployed build contains `9ef6e50`
plus the asset-cache compatibility fix recorded in this commit. Runtime and PTYs execute on
the OnePlus; the Mac remains a build/admin/browser client.

Installed ARM64 binary SHA-256:
`c70e1f493a493d587eb2c813b7c67d9dbe532b48ee1bdedd60f074067165eee3`

Original rollback binary: `/root/verb-deployments/9ef6e50/verb.previous`, SHA-256:
`7df6dd305c6ba11815410b819fa0664861805f1789bf6e127c7640c0c2ffb5fb`

Node manifest: `/root/verb-deployments/9ef6e50/manifest.json`. Rollback requires replacing
`/usr/local/bin/verb` with the backup and restarting only Verb after accounting for active work.
Persistent state was not cleared/replaced. Do not restore older session data over new work.

## Findings during deployment

The initial real-URL reload mixed new HTML with previously cached JavaScript. It showed
incorrect counts and an unfinished deployment label. Versioned JS/CSS query strings now
make existing browsers fetch matching assets; no-store responses remain in place. An ordinary
reload then displayed the correct polished UI.

Supervised restart stalled at the PRoot wrapper. After verifying the parent/child relationship,
terminating only its old Verb child let the existing supervisor restart successfully. The final
supervised wrapper PID was 31677. No service script or other service was changed. The authorized
first restart stopped the two existing live terminals; their records remained in History. Before
the follow-up restart, the fast workspace endpoint confirmed no new active sessions.

## Real URL acceptance

Chrome used `https://verb.pruningmypothos.com`, through the existing Cloudflare/Access path:

- Sessions opened with `PocketFabric Node 1 / REMOTE`, corrected assets, and collapsed History.
- New Terminal ran `pwd`, `git status`, and `uname -a`: `/root/verb-test-project`, clean master,
  and ARM64 Linux. Typed commands and output rendered correctly.
- Claude and Codex panes opened and switched correctly. Claude was at theme setup; Codex was
  at sign-in. No authentication, model interaction, or onboarding was performed.
- Closing the tab and reopening the real URL restored the same three active session IDs.
- Navigation and Sessions panels collapsed/restored without replacing the terminal session.
- Only the two agent acceptance sessions were explicitly ended afterward. The final view had
  one plain terminal left ready for the user and eleven collapsed historical records.
- Browser inspection on the reopened page showed no warnings/errors.

Final node loopback samples: `/api/workspace` **39.55 ms**, `/api/state` **917.99 ms**, one
hosted session and eleven historical records. These are individual server-side observations,
excluding Cloudflare/network/browser time. HTTPS acceptance was functional; no new quantitative
Cloudflare-path latency claim is made. Earlier before/after measurements remain in the review.

The full Rust suite passed again (248 passed, 2 existing ignored); all 11 web tests passed.
ARM64 release cross-build passed with the existing deprecated linker-optimization warning.

The deployed web flow is accepted for user testing. Normal agent work still needs user-owned
CLI onboarding. Narrow-viewport acceptance, graceful Verb restart handling, and the separate
sshd supervision issue remain in the backlog. Full integration PR/CI review and merge remain
pending; no PR or merge was performed.

Cloudflare, DNS, Access policies, AdGuard, SSH configuration, LanguageOps, Android APK, and
PocketFabric tunnel configuration were not modified. The OnePlus was not rebooted.

# PocketFabric Node 1 Phase 1 Streaming Deployment — 6 October 2026

The user authorized deploying and restarting Verb on PocketFabric Node 1 (OnePlus 9).
The deployed build incorporates Phase 1 streaming WebSockets, `@xterm/addon-webgl`, backpressure flow control, and remote `Host` header whitelisting for `verb.pruningmypothos.com`.

- Installed ARM64 binary SHA-256:
  `00c342b1dbf9d4d0f5624d249ff6e5736d3872602707e98c897825e5b2ff289e`
- Rollback binary: `/root/verb-deployments/phase1-streaming/verb.previous`, SHA-256:
  `c70e1f493a493d587eb2c813b7c67d9dbe532b48ee1bdedd60f074067165eee3`
- Node manifest: `/root/verb-deployments/phase1-streaming/manifest.json`.
- Supervised restart was clean: terminated previous Verb PID 4842; `runsv` immediately launched new PID 5988 running `/usr/local/bin/verb web --port 3005`.
- Verified `/api/terminals/ws` WebSocket upgrade responds with 400 (Bad Request: missing Sec-WebSocket-Key) for both loopback and remote host `verb.pruningmypothos.com` with authenticated token.
- `cloudflared` remained alive with its original PID 9179.
# PocketFabric Node 1 Phase 2 Performance & Terminal Emulation Deployment — 7 October 2026

The user authorized deploying the latency elimination and Phase 2 terminal emulation build to PocketFabric Node 1 (OnePlus 9).

- Installed ARM64 binary SHA-256:
  `a912fc0b47ae1e1cdb2dede2b0378d52a233813ddf9d39192e17550107773fab`
- Rollback binary: `/root/verb-deployments/phase2-perf-terminal/verb.prev`, SHA-256:
  `00c342b1dbf9d4d0f5624d249ff6e5736d3872602707e98c897825e5b2ff289e`
- Key latency & emulation improvements deployed:
  - Fixed `unacked_bytes` unsigned underflow in `TerminalStreamSink::ack` that was triggering artificial 50-100ms pause penalties on PTY reads.
  - Eliminated client micro-ACK packet storm: batched stream ACKs with 120ms debounce so single keystroke echoes never flood the uplink.
  - Pre-encoded and memoized terminal ID UTF-8 byte arrays to remove per-keystroke garbage collection and allocations.
  - Immediate queue flush in WebSocket event loop right after handling incoming terminal input, shaving off event loop cycles.
  - Added `@xterm/addon-web-links`, `@xterm/addon-unicode11`, and `@xterm/addon-search` with `scrollback: 10000`.
  - Configured `macOptionIsMeta: true`, `macOptionClickForcesSelection: true`, `scrollOnUserInput: true`, `windowsMode: false`.
  - Added dynamic window title synchronization (`OSC 0/2` -> `.terminal-title`) and visual bell animation (`.terminal-tile.bell`).
  - Added automatic alternate screen buffer reattach prefixing (`\x1b[?1049h`) when restoring full-screen apps (`vim`, `htop`, `opencode`).
  - Set default environment variables `TERM=xterm-256color` and `COLORTERM=truecolor` for full 24-bit truecolor support.
  - Expanded reader sync channel capacity to 2048 and drain batch to 512 chunks per tick.
- Supervised restart was clean: terminated previous Verb PID 5988; `runsv` restarted PID 21949 running `/usr/local/bin/verb web --port 3005`.
- Verified `/` serves updated assets `?v=stream-phase2-20261007` and public endpoint `https://verb.pruningmypothos.com` routes correctly.


