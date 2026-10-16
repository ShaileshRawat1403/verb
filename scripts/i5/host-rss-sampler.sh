#!/usr/bin/env bash
# Samples the memory of one Android app uid from the host, over adb, so peak RSS can be measured
# without running anything extra inside the runtime being measured.
#
#   scripts/i5/host-rss-sampler.sh com.aistudio.verb.app.debug evidence/host-rss.tsv [interval_s]
#
# Each row: phone epoch, total RSS (kB) of every process owned by the app uid, process count, and
# the three largest processes. The phone's own clock is used so rows line up with the audit
# driver's start_epoch/end_epoch. Stop it with Ctrl-C.

set -u
PKG="${1:?package}"
OUT="${2:?output tsv}"
INTERVAL="${3:-5}"
ADB="${ADB:-/opt/homebrew/share/android-commandlinetools/platform-tools/adb}"

# Android 14 prints `appId=`; older releases print `userId=`.
uid="$("$ADB" shell dumpsys package "$PKG" | grep -o -E '(appId|userId)=[0-9]+' | head -1 | cut -d= -f2)"
[ -n "$uid" ] || { echo "no uid for $PKG" >&2; exit 1; }
user="u0_a$((uid - 10000))"
mkdir -p "$(dirname "$OUT")"
[ -f "$OUT" ] || printf 'epoch\ttotal_rss_kb\tprocs\ttop3\n' > "$OUT"
echo "sampling $PKG ($user) every ${INTERVAL}s -> $OUT"

while :; do
  "$ADB" shell "echo EPOCH \$(date +%s); ps -A -o USER,RSS,NAME" 2>/dev/null | awk -v u="$user" '
    /^EPOCH/ { ep = $2; next }
    $1 == u { total += $2; n++; top[n] = $2 " " $3 }
    END {
      if (ep == "") exit
      # three largest, by RSS
      for (i = 1; i <= n; i++) for (j = i + 1; j <= n; j++) {
        split(top[i], a, " "); split(top[j], b, " ")
        if (b[1] + 0 > a[1] + 0) { t = top[i]; top[i] = top[j]; top[j] = t }
      }
      s = ""; for (i = 1; i <= n && i <= 3; i++) s = s (i > 1 ? ", " : "") top[i]
      printf "%s\t%d\t%d\t%s\n", ep, total, n, s
    }' >> "$OUT"
  sleep "$INTERVAL"
done
