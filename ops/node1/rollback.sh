#!/usr/bin/env bash
# Restores the binary that a given deployment replaced.   ops/node1/rollback.sh <git-sha> [--restart]
. "$(dirname "$0")/lib.sh"
SHA="${1:-}"; [ -n "$SHA" ] || die "usage: rollback.sh <git-sha of the deployment to undo> [--restart]"
debian "set -e; P=/root/verb-deployments/$SHA/verb.previous; [ -f \$P ] || { echo \"no backup at \$P\"; exit 1; }
  install -m 0755 \$P /usr/local/bin/verb.new && mv /usr/local/bin/verb.new /usr/local/bin/verb
  echo restored; sha256sum /usr/local/bin/verb"
[ "${2:-}" = "--restart" ] && termux 'SVDIR=$PREFIX/var/service sv restart verb'
printf -- '- %s rolled back deployment `%s`\n' "$(date +%F)" "$SHA" >> "$HERE/DEPLOYMENTS.md"
"$HERE/status.sh"
