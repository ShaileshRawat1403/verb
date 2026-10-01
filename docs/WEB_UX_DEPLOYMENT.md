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
