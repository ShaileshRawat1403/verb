#!/usr/bin/env bash
# Verb I-5 continuity soak: a heartbeat that proves when the terminal stopped being alive.
#
# Run it inside a Verb terminal of the build under test, then turn the screen off:
#
#   bash /sdcard/Download/verb-audit/soak-heartbeat.sh baseline
#
# One row every 30 seconds: sequence, wall-clock epoch, the shell's pid and the gap since the previous
# row. Battery and boot-clock readings are not available from the release sandbox (/proc/uptime and the
# power_supply nodes are denied), so the battery timeline comes from `dumpsys batterystats --history` on
# the host instead. The file lives on shared storage so it can be read from adb after the
# app that wrote it is gone.
#
# Reading the result:
#   - the last row, compared with `dumpsys activity exit-info`, bounds when and why Verb died;
#   - a gap far above 30s followed by more rows means the process was suspended or frozen, not
#     killed -- the heartbeat resumed, so it survived.

set -u
CONDITION="${1:?condition name, e.g. baseline, doze-whitelist, vivo-bg-allow, phantom-off}"
DIR=/sdcard/Download/verb-audit/soak
mkdir -p "$DIR"
START="$(date +%s)"
LOG="$DIR/$CONDITION-$START.log"

{
  echo "# condition=$CONDITION"
  echo "# started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ) epoch=$START"
  echo "# shell_pid=$$ ppid=$PPID"
  echo "# uname=$(uname -a)"
  echo "# seq	epoch	pid	gap_s"
} > "$LOG"
echo "heartbeat -> $LOG (Ctrl-C to stop; leave running and turn the screen off)"

seq=0
prev="$START"
while :; do
  seq=$((seq + 1))
  now="$(date +%s)"
  printf '%d\t%d\t%d\t%d\n' "$seq" "$now" "$$" "$((now - prev))" >> "$LOG"
  prev="$now"
  sleep 30
done
