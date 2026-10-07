# Node 1: the OnePlus 9 that hosts Verb Desktop

Verb Desktop (`desktop/`) runs here for remote use. This folder is everything needed to operate it.
Code changes belong in `desktop/`; this folder only builds, installs, checks and rolls back.

## Topology

```
Browser ──https──▶ Cloudflare Access (verb.pruningmypothos.com)
                       │ cloudflared tunnel (Termux runit service "cloudflared")
                       ▼
OnePlus 9 · Termux ── runit service "verb" ── proot-distro Debian
                                                └ /root/bin/start_verb.sh
                                                    └ /usr/local/bin/verb web --port 3005  (127.0.0.1 only)
```

| Thing | Where |
|---|---|
| SSH (Termux) | `ssh -p 8022 u0_a306@192.168.68.114` (LAN) |
| Service | `$PREFIX/var/service/verb`, control with `SVDIR=$PREFIX/var/service sv status|restart verb` |
| Logs | `$PREFIX/var/log/sv/verb/current` |
| Binary | `/usr/local/bin/verb` inside Debian |
| Config and secrets | `/root/bin/start_verb.sh` inside Debian: `VERB_TOKEN`, `VERB_ALLOWED_ORIGIN`, `VERB_ALLOWED_EMAIL`, `VERB_CF_ACCESS_ISS`, `VERB_CF_ACCESS_AUD`. **Never copy these values into the repo, docs or chat.** |
| Deployments | `/root/verb-deployments/<git-sha>/` holds `verb.previous` and `manifest.json` |
| Other services on the node | `cloudflared`, `sshd`, `adguard`, `languageops`, `languageops-web`. Do not touch them from Verb work. |

## Everyday operations

```sh
cp ops/node1/node.env.example ops/node1/node.env   # once; node.env is git-ignored
ops/node1/status.sh                                 # read-only: service, binary hash, HTTP, logs
ops/node1/deploy.sh --dry-run                       # build only
ops/node1/deploy.sh                                 # install; running Verb keeps serving the old binary
ops/node1/deploy.sh --restart                       # install and restart: ENDS every live terminal
ops/node1/rollback.sh <git-sha> [--restart]         # put back what that deployment replaced
```

Rules:

- **Deploy only committed code.** `deploy.sh` refuses a dirty tree, so every running binary maps to a
  commit. Commit the line it appends to `DEPLOYMENTS.md`.
- **A restart ends live terminals and agents.** Check the workspace first, and tell the owner.
- **Ask the owner before deploying or restarting.** Node 1 is their live environment.
- **Cross-building** needs rustup (the script installs the pinned toolchain and ARM64 target),
  `cargo install cargo-zigbuild` and `brew install zig`. Homebrew's own `rustc` cannot build for the node.

## Rotating the token

The token is a full shell on this phone. `start_verb.sh` reads it from `/root/.verb/web.token`.
Rotate it whenever it has appeared anywhere but the node. This generates the new value on the node
and never prints it:

```sh
. ops/node1/lib.sh
debian 'umask 077; head -c 32 /dev/urandom | od -An -tx1 | tr -d " \n" > /root/.verb/web.token.new && mv /root/.verb/web.token.new /root/.verb/web.token'
# then restart Verb (see below); live terminals end
```

To read it, use your own terminal, never an agent's:
`ssh -p 8022 u0_a306@192.168.68.114 "proot-distro login debian -- cat /root/.verb/web.token"`.
Over the internet, Cloudflare Access is the gate. The token is for the LAN SSH tunnel.

## Restart troubleshooting

`deploy.sh` and `rollback.sh` handle this automatically (`restart_verb` in `lib.sh`). By hand:
`sv restart verb` can stall at the PRoot wrapper: `sv status` shows `got TERM` while the old process
lives on. Find it with `pgrep -f "^/usr/local/bin/verb web"` (anchored, so it cannot match your own shell), terminate only
that PID, and runit starts a fresh one. Leave the wrapper and every other service alone.

## Known issues (2026-10-07)

- `sv restart verb` stalls (see above). Seen on 2026-10-07; the troubleshooting step worked.
- Resolved 2026-10-07: the SIGBUS crashes (`proot info: vpid 1: terminated with signal 7`). Each one
  happened within about 30 ms of `/usr/local/bin/verb` being overwritten in place while Verb was
  running: 10-06 14:22:32 (Phase 1 install), 10-07 02:12:14 (Phase 2), 10-07 09:40:35 (the
  unrecorded install). Overwriting a running executable changes the pages the process has mapped,
  and the kernel kills it on the next fault. **Never `cp` over the binary.** `deploy.sh` writes
  `verb.new` and renames it into place, which gives it a new inode, so the running process keeps
  its own file; none of its deploys crashed. Some earlier SIGSEGVs (signal 11) may share this cause
  but were not correlated.
- Resolved 2026-10-07: an unrecorded binary (`bc06dfe7…`) was replaced by `37a0424`, a committed
  build.
