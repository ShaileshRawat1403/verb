# Verb V0 Device Validation

This checklist validates the production APK on a physical Android device. It complements
Robolectric tests; it does not authorize changes to the frozen Termux terminal foundation.

## Build artifact

Build from the authoritative `verb` repository:

```bash
./gradlew :app:assembleDebug
```

Install `app/build/outputs/apk/debug/app-debug.apk` with ADB or Android Studio. Use a local
debug keystore only; do not commit a keystore or `local.properties` file.

## Ask and result provenance

For each request below, confirm that the result shows a distinct **Derived**, **Observed**,
and **Explanation** view. Android API observations must not be presented as shell output.

| Request | Expected behavior |
| --- | --- |
| `show me my storage` | Reports device storage through Android `StatFs`. |
| `check memory` | Reports memory through Android `ActivityManager`. |
| `show running processes` | Reports only processes Android exposes to Verb. |
| `show files` | Lists the app-accessible default directory. |
| `find files report` | Searches app storage for `report`; no empty-query expansion. |
| `what is using port 3000` | Performs a local TCP bind check only; never claims a PID owner. |
| `what does ps do?` | Produces a non-executing command explanation. |

## Controlled action safety

1. Submit `stop process 1234`.
2. Confirm that an explicit confirmation dialog appears before any request is made.
3. Cancel it and verify no success result is shown.
4. Repeat and confirm: the result must say the signal was attempted and outcome is unverified.
5. Never test a PID needed by the device or Verb itself.

## Terminal and Semantic Lens

1. Open **Terminal** and confirm that interactive typing goes to the real terminal view.
2. Confirm that the runtime is Android `/system/bin/sh`; do not expect full Termux userland
   commands such as `pkg`, `apt`, or `git`.
3. Select `port 8080`, inspect it, and confirm **Inspect Port 8080** is offered as a typed
   read-only action.
4. Select `rm -rf example`; confirm it is described as destructive and no execute action is
   offered.
5. Copy a selection and confirm copying does not automatically open Semantic Lens.
6. Use the selection action to **Inspect/Explain** and confirm it does open Semantic Lens.

## Evidence to record

Record device model, Android API level, APK commit, and any exact failure output. In
particular, record a reproducible PTY, rendering, selection, or native failure before changing
P0.2/P0.3 terminal code. Platform warnings such as `E/ashmem` are not independently actionable
unless accompanied by observable Verb behavior.
