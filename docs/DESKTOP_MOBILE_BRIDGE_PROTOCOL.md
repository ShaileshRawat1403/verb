# Desktop live-session bridge, protocol v1

Status: **desktop-local developer preview**. This contract is exercised between separate local
processes. It is not a phone connection, and the Android app has no receiver for it.

## Boundary

Each live session hosted inside `verb ui` owns one ephemeral Unix socket. The socket is under a
directory owned by the current Unix account with mode `0700`; the socket is mode `0600`. Its name
is derived from the state-root and session ID, avoiding path traversal and Unix socket path limits.
There is no TCP or LAN listener. A mobile receiver must use an authenticated, encrypted transport
to reach the local protocol. It must not forward terminal data or tokens over an unprotected link.

The PTY and CLI conversation stay on the desktop. The bridge only shares the current parsed screen
and routes input to that exact live PTY. The bridge ends with the PTY. It never loads an imported
`.vcont` session or stores screen bytes, input, pairing tokens, or a transcript on disk.

## Wire shape

One connection carries one UTF-8 JSON request line and one JSON response line. Input is capped at
32 KiB. Requests reject unknown fields. Every request contains `version: 1` and an `op`; every
response contains `version: 1` and either `{ "ok": true, "result": ... }` or
`{ "ok": false, "error": "..." }`. `verb mobile request SESSION_ID` reads this line from stdin;
the session ID is never enough to authorize phone input.

| Operation | Request fields | Result | Rule |
| --- | --- | --- | --- |
| `offer` | none | `pairingToken` | Desktop-local only. Replaces an older offer. |
| `pair` | `secret: pairingToken` | `deviceToken` | One use, 120 seconds, at most five failed attempts. Replaces an earlier paired phone. |
| `snapshot` | `secret: deviceToken` | `sessionId`, `revision`, `bytes`, `controller` | `bytes` is a byte array containing the current ANSI-formatted screen, or null when too large. |
| `take` | `secret: deviceToken` | `controller: phone` | Gives phone the sole input lease. |
| `input` | `secret: deviceToken`, `bytes` | `status: queued` | Requires phone control; 1–4096 bytes per request and 64 KiB total queue. |
| `disconnect` / `reconnect` | `secret: deviceToken` | connection status | Disconnect returns input control to desktop. |
| `desktop_take` / `revoke` | none | controller or status | Owner-account actions. Both clear queued phone input; revoke invalidates the device token. |

`snapshot` and accepted phone input refresh the client's heartbeat. Thirty seconds without contact
marks the phone disconnected and returns input control to desktop. Reconnect uses the same device
token only while the original hosted process remains live. Control transfer and PTY writes use one
lock, so input queued under an old lease cannot arrive after desktop takeback. Screen revisions
increase when the current screen changes; they are volatile and are not task or memory revisions.

## Product integration still needed

The Workbench should reveal **Continue on phone** only when a mobile receiver and protected
transport actually exist. That flow should show the session name and desktop location, pair by QR
or a short code backed by the full-strength token, and clearly show who controls input. The current
CLI and socket are a testable desktop boundary, not that finished user flow. Phone-local execution
from a task handoff remains a separate successor-session feature.

## Verification

`mobile::tests::local_process_protocol_pairs_and_controls_only_the_chosen_live_session` exercises
the socket in a separate thread, its file mode, protocol version check, pairing, screen snapshot,
input, desktop takeback, revocation, and cleanup. Other mobile tests cover expiry, reconnect,
single-controller input, idle disconnection, and byte bounds. The full desktop gate is recorded in
`DESKTOP_CORE_INTEGRITY.md`.
