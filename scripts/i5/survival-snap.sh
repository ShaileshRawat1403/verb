#!/usr/bin/env bash
# One survival-table observation of the release Verb package, from the host.
#
#   scripts/i5/survival-snap.sh <label> <evidence-dir>
#
# Records the app uid's processes (PID, PPID, start time, name), whether TerminalHoldService is in the
# foreground, the newest historical exit record, a UI text dump and a screenshot. Comparing the PIDs of
# proot, bash and the stand-in child across two snapshots is the survival verdict for a row.

set -u
LABEL="${1:?label}"
OUT="${2:?evidence dir}"
PKG="${PKG:-com.aistudio.verb.app}"
ADB="${ADB:-/opt/homebrew/share/android-commandlinetools/platform-tools/adb}"
mkdir -p "$OUT"
uid="$("$ADB" shell dumpsys package "$PKG" | grep -o -E '(appId|userId)=[0-9]+' | head -1 | cut -d= -f2)"
user="u0_a$((uid - 10000))"
f="$OUT/$LABEL.txt"
{
  echo "label=$LABEL host_utc=$(date -u +%FT%TZ) phone_epoch=$("$ADB" shell date +%s)"
  echo "--- processes ($user)"
  "$ADB" shell ps -A -o PID,PPID,STIME,RSS,NAME | awk -v u="$user" 'NR==1 || $0 ~ u'
  "$ADB" shell ps -A -o PID,PPID,USER,STIME,NAME | awk -v u="$user" '$3==u {print "  " $1, "ppid=" $2, $4, $5}'
  echo "--- TerminalHoldService"
  "$ADB" shell dumpsys activity services "$PKG" | grep -E 'ServiceRecord|isForeground' || echo "not running"
  echo "--- newest exit record"
  "$ADB" shell dumpsys activity exit-info "$PKG" | grep -E 'timestamp=|reason=|description=' | head -3 | sed -E 's/^[[:space:]]+//'
  echo "--- UI text"
  "$ADB" shell uiautomator dump /sdcard/ui.xml >/dev/null 2>&1
  "$ADB" shell cat /sdcard/ui.xml | tr '>' '>\n' | grep -o -E '(text|content-desc)="[^"]+"' | grep -v -E '"(▲|▼|TAB|DEL|\^C|ESC|CTRL|PASTE)"' | head -25
} > "$f" 2>&1
"$ADB" exec-out screencap -p > "$OUT/$LABEL.png"
grep -E '^  [0-9]' "$f"
