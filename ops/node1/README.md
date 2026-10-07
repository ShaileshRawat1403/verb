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

The token is a full shell on this phone. Rotate it whenever it has appeared anywhere but the node:

1. On the node, in Debian, edit `/root/bin/start_verb.sh` and set `VERB_TOKEN` to the output of
   `openssl rand -hex 32`.
2. Restart Verb (live terminals end).
3. Use the new token only through the local SSH tunnel; over the internet, Cloudflare Access is the gate.

## Restart troubleshooting

`sv restart verb` can stall at the PRoot wrapper: `sv status` shows `got TERM` while the old process
lives on. Find the `verb web --port 3005` child (`ps -eo pid,args | grep "verb web"`), terminate only
that PID, and runit starts a fresh one. Leave the wrapper and every other service alone.

## Known issues (2026-10-07)

- The installed binary (`bc06dfe7…`) matches none of the recorded deployments. It was installed
  after the last recorded entry without a record. The next `deploy.sh` run replaces it with a
  traceable build.
- `sv status verb` reports `got TERM` from an earlier restart that never completed.
- The log shows `proot info: vpid 1: terminated with signal 7` (SIGBUS) at 09:40 IST on 7 Oct;
  runit restarted Verb 2 s later. Cause not investigated.
