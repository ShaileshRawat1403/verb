#!/usr/bin/env bash
# Pulls soak heartbeats and the platform's own account of why Verb died, side by side.
#
#   scripts/i5/soak-collect.sh <package> <evidence-dir>
#
# For every heartbeat log it prints the first and last row and the largest gap, then the historical
# exit reasons for the package. Correlating the two is the verdict: a FORCE STOP whose timestamp sits
# just after a heartbeat's last row is the kill; a large gap followed by more rows is a suspension the
# process survived.

set -u
PKG="${1:?package}"
OUT="${2:?evidence dir}"
ADB="${ADB:-/opt/homebrew/share/android-commandlinetools/platform-tools/adb}"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$OUT/heartbeats"

"$ADB" pull /sdcard/Download/verb-audit/soak/. "$OUT/heartbeats/" >/dev/null
"$ADB" shell dumpsys activity exit-info "$PKG" > "$OUT/exit-info-$PKG-$stamp.txt"
"$ADB" shell dumpsys deviceidle whitelist > "$OUT/deviceidle-whitelist-$stamp.txt"
"$ADB" shell settings get global settings_enable_monitor_phantom_procs > "$OUT/phantom-setting-$stamp.txt"
"$ADB" shell dumpsys battery > "$OUT/battery-$stamp.txt"
"$ADB" shell dumpsys batterystats --history > "$OUT/batterystats-history-$stamp.txt"

for f in "$OUT"/heartbeats/*.log; do
  [ -e "$f" ] || continue
  echo "== $(basename "$f")"
  grep '^# condition\|^# started_utc' "$f"
  awk -F'\t' '!/^#/ {
      if (first == "") first = $2
      last = $2; rows++
      if ($4 + 0 > maxgap) { maxgap = $4; maxgap_at = $2 }
    }
    END {
      if (rows == 0) { print "no rows"; exit }
      cmd = "date -u -r " first " +%FT%TZ"; cmd | getline f; close(cmd)
      cmd = "date -u -r " last  " +%FT%TZ"; cmd | getline l; close(cmd)
      cmd = "date -u -r " maxgap_at " +%FT%TZ"; cmd | getline g; close(cmd)
      printf "rows=%d first=%s last=%s alive_for=%ds largest_gap=%ds ending %s\n", rows, f, l, last - first, maxgap, g
    }' "$f"
done

echo "== exit-info ($PKG), most recent first"
grep -E 'timestamp=|reason=|description=' "$OUT/exit-info-$PKG-$stamp.txt" \
  | sed -E 's/^[[:space:]]+//' | paste - - - | head -12
