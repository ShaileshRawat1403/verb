# Shared helpers for the Node 1 scripts. Sourced, not executed.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
[ -f "$HERE/node.env" ] && . "$HERE/node.env"
: "${NODE_HOST:=192.168.68.114}" "${NODE_PORT:=8022}" "${NODE_USER:=u0_a306}"
TARGET=aarch64-unknown-linux-gnu
# The toolchain Desktop CI pins (.github/workflows/desktop-ci.yml). Built through rustup, not
# Homebrew's rustc, which has no standard library for the ARM64 Linux target.
RUST_TOOLCHAIN=1.99.0
BIN="$REPO/desktop/target/$TARGET/release/verb"

# Runs a command in Termux on the node.
termux() { ssh -o BatchMode=yes -o ConnectTimeout=8 -p "$NODE_PORT" "$NODE_USER@$NODE_HOST" "$@"; }
# Runs a command inside the Debian PRoot where Verb lives.
debian() { termux "proot-distro login debian -- bash -lc $(printf '%q' "$1")"; }
say() { printf '\033[1m==>\033[0m %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# Restarts Verb under runit. `sv restart` reliably stalls at the PRoot wrapper (seen on every restart
# on 2026-10-07), so when the old process survives, stop only the real `verb web` process; runit then
# starts a fresh one. The pattern is anchored so it never matches this command's own shell.
restart_verb() {
  termux 'export SVDIR=$PREFIX/var/service
    old=$(pgrep -f "^/usr/local/bin/verb web")
    sv restart verb >/dev/null 2>&1
    sleep 6
    if [ -n "$old" ] && [ "$(pgrep -f "^/usr/local/bin/verb web")" = "$old" ]; then
      echo "sv restart stalled; stopping only verb web ($old)"; kill $old; sleep 6
    fi
    new=$(pgrep -f "^/usr/local/bin/verb web")
    [ -n "$new" ] && [ "$new" != "$old" ] && echo "restarted: verb web pid $new" || { echo "restart did not complete"; exit 1; }'
}
