# Shared helpers for the Node 1 scripts. Sourced, not executed.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
[ -f "$HERE/node.env" ] && . "$HERE/node.env"
: "${NODE_HOST:=192.168.68.114}" "${NODE_PORT:=8022}" "${NODE_USER:=u0_a306}"
TARGET=aarch64-unknown-linux-gnu
BIN="$REPO/desktop/target/$TARGET/release/verb"

# Runs a command in Termux on the node.
termux() { ssh -o BatchMode=yes -o ConnectTimeout=8 -p "$NODE_PORT" "$NODE_USER@$NODE_HOST" "$@"; }
# Runs a command inside the Debian PRoot where Verb lives.
debian() { termux "proot-distro login debian -- bash -lc $(printf '%q' "$1")"; }
say() { printf '\033[1m==>\033[0m %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }
