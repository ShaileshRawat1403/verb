#!/usr/bin/env bash
# Read-only health check of Verb on Node 1. Changes nothing.
. "$(dirname "$0")/lib.sh"
say "service (runit, Termux side)"
termux 'SVDIR=$PREFIX/var/service sv status verb cloudflared'
say "installed binary and HTTP check (Debian side)"
debian 'sha256sum /usr/local/bin/verb | cut -c1-64; printf "HTTP %s\n" "$(curl -s -o /dev/null -w %{http_code} http://127.0.0.1:3005/)"; ls -1t /root/verb-deployments 2>/dev/null | head -3 | sed "s/^/recent deployment: /"'
say "last log lines"
termux 'tail -n 5 $PREFIX/var/log/sv/verb/current 2>/dev/null || echo "(no log)"'
