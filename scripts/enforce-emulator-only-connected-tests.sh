#!/bin/sh
set -eu

adb=${1:?Android Debug Bridge path is required}
physical_serials=$(
  "$adb" devices |
    awk 'NR > 1 && $2 == "device" && $1 !~ /^emulator-/ { print $1 }'
)

if [ -n "$physical_serials" ]; then
  serials=$(printf '%s\n' "$physical_serials" | paste -sd ', ' -)
  printf 'Refusing connected instrumentation while a physical Android device is attached (%s). Use a disposable emulator.\n' "$serials" >&2
  exit 1
fi
