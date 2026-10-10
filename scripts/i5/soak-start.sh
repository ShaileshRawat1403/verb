#!/usr/bin/env bash
# Starts the soak heartbeat inside the *release* Verb terminal, from the host.
#
#   scripts/i5/soak-start.sh <condition> [seconds]     seconds bounds a dry run; omit for a real soak
#
# The release package is not debuggable and cannot read files another uid put on shared storage, so
# the heartbeat cannot be pushed and run. Instead it is typed into the terminal that is already in
# front, as base64 -- whose alphabet survives `adb shell input text` untouched -- decoded there into
# $HOME, and started. The heartbeat then writes to /sdcard/Download, where files the app created
# itself remain readable over adb after the app is gone.
#
# Before running: release Verb in front, its terminal focused at a shell prompt.

set -eu
CONDITION="${1:?condition}"
LIMIT="${2:-}"
ADB="${ADB:-/opt/homebrew/share/android-commandlinetools/platform-tools/adb}"
HERE="$(cd "$(dirname "$0")" && pwd)"

"$ADB" shell monkey -p com.aistudio.verb.app -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1
sleep 3

ui() { "$ADB" shell uiautomator dump /sdcard/ui.xml >/dev/null 2>&1; "$ADB" shell cat /sdcard/ui.xml; }

# Typing lands wherever focus is. A sheet left open (the Verb palette, the workspace sheet) would
# swallow the whole heartbeat as search text, so dismiss sheets and refuse to type unless the
# terminal workspace is what is in front.
for _ in 1 2 3; do
  case "$(ui)" in
    *"Type what you are trying to do."*|*'content-desc="Close sheet"'*) "$ADB" shell input keyevent KEYCODE_BACK; sleep 1 ;;
    *) break ;;
  esac
done
case "$(ui)" in
  *'Terminal session running'*'Run typed terminal input'*) ;;
  *) echo "release Verb terminal is not in front; not typing" >&2; exit 1 ;;
esac
"$ADB" shell input tap 540 1300   # focus the terminal canvas
sleep 1

b64="$(base64 < "$HERE/soak-heartbeat.sh" | tr -d '\n')"
type_line() { "$ADB" shell "input text '$1'"; "$ADB" shell input keyevent 66; }

# Short chunks with a pause between them: typed faster than the terminal echoes, a chunk can land in
# the middle of the previous prompt and corrupt the file (seen in the first dry run).
type_line 'rm%s-f%s$HOME/.soak.b64'
sleep 1
i=0
while [ $i -lt ${#b64} ]; do
  type_line "printf%s%s%s${b64:$i:200}%s>>%s\$HOME/.soak.b64"
  sleep 2
  i=$((i + 200))
done
type_line 'mkdir%s-p%s/sdcard/Download/verb-audit/soak%s&&%sbase64%s-d%s$HOME/.soak.b64%s>%s$HOME/soak-heartbeat.sh%s&&%ssha256sum%s$HOME/soak-heartbeat.sh%s>%s/sdcard/Download/verb-audit/soak/typed.sha256'
sleep 3

# The phone writes the hash of what it decoded; the heartbeat starts only if that matches this file.
want="$(shasum -a 256 "$HERE/soak-heartbeat.sh" | awk '{print $1}')"
got="$("$ADB" shell cat /sdcard/Download/verb-audit/soak/typed.sha256 2>/dev/null | awk '{print $1}')"
echo "expected sha256: $want"
echo "decoded  sha256: ${got:-none}"
if [ "$want" != "$got" ]; then
  echo "transfer corrupted; heartbeat NOT started" >&2
  exit 1
fi
if [ -n "$LIMIT" ]; then
  type_line "timeout%s$LIMIT%sbash%s\$HOME/soak-heartbeat.sh%s$CONDITION"
else
  type_line "bash%s\$HOME/soak-heartbeat.sh%s$CONDITION"
fi
echo "typed. Screenshot to confirm the heartbeat line, then unplug and turn the screen off."
