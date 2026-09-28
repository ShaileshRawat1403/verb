# Desktop live-session bridge, protocol v1

Status: **same-network live control implemented**. The desktop web terminal and `verb mobile share
SESSION_ID` create a session-scoped TLS relay. Verb Mobile opens a pairing link through Android's
link handler or accepts it pasted into **Control a desktop session**.

## Boundary

Each live session hosted inside `verb ui` owns one ephemeral Unix socket. The socket is under a
directory owned by the current Unix account with mode `0700`; the socket is mode `0600`. Its name
is derived from the state-root and session ID, avoiding path traversal and Unix socket path limits.
The relay listens on one ephemeral LAN port only after the desktop user chooses to share a live
session. Its self-signed certificate has IP subject alternative names for the offered desktop
addresses. The phone verifies the exact SHA-256 certificate fingerprint from the pairing link and
the IP hostname before sending the one-use pairing code. Each connection carries one request and
one response over TLS. Closing the share revokes the device token and stops the listener. The web
UI itself remains loopback only.
The code is in the fragment of a `verb://pair` URI, never an HTTP query or request URL.

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
| `snapshot` | `secret: deviceToken` | `sessionId`, `revision`, `bytes`, `controller` | `bytes` is a byte array containing the current plain-text screen, or null when too large. |
| `take` | `secret: deviceToken` | `controller: phone` | Gives phone the sole input lease. |
| `input` | `secret: deviceToken`, `bytes` | `status: queued` | Requires phone control; 1–4096 bytes per request and 64 KiB total queue. |
| `disconnect` / `reconnect` | `secret: deviceToken` | connection status | Disconnect returns input control to desktop. |
| `desktop_take` / `revoke` | none | controller or status | Owner-account actions. Both clear queued phone input; revoke invalidates the device token. |

`offer`, `desktop_take` and `revoke` are refused when the connecting process belongs to the hosted
program's own process session (checked from the socket peer's pid with `SO_PEERCRED` on Linux and
`LOCAL_PEERPID` on macOS; an unidentifiable peer is refused too). The hosted program knows its own
`VERB_SESSION_ID` and runs as the same account, so without this check it could offer, pair and
take control of its own terminal. A descendant that deliberately calls `setsid` is not detected.
The browser workbench takes input back with `POST /api/terminals/ID/control`, which needs the
page's token.

`snapshot` and accepted phone input refresh the client's heartbeat. Thirty seconds without contact
marks the phone disconnected and returns input control to desktop. Reconnect uses the same device
token only while the original hosted process remains live. Control transfer and PTY writes use one
lock, so input queued under an old lease cannot arrive after desktop takeback. Screen revisions
increase when the current screen changes; they are volatile and are not task or memory revisions.

## User flow and scope

In `verb web`, start a terminal and choose **Phone** in its title bar. Scan its QR code or copy the
link; if several desktop addresses are offered, select the one the phone can reach. On Android,
tap **Connect**, then **Take input control** to send text and terminal keys. The screen displays the
current control owner; a connection failure stops the live claim and offers **Reconnect**. The
desktop can take control back, and **Stop sharing** invalidates the phone token. The same flow for
a TUI-hosted session starts with `verb mobile share SESSION_ID` in another desktop shell.

The desktop and phone must be on the same reachable network, and the desktop must stay running.
This is remote control of one existing process; it does not transfer files, agent credentials, or
the CLI's native conversation to Android. The phone token stays in app memory and a fresh pairing
link is required after the Android process exits. Phone-local successor sessions remain a separate
handoff feature.

## Verification

`mobile::tests::local_process_protocol_pairs_and_controls_only_the_chosen_live_session` exercises
the socket in a separate thread, its file mode, protocol version check, pairing, screen snapshot,
input, desktop takeback, revocation, and cleanup. Other mobile tests cover expiry, reconnect,
single-controller input, idle disconnection, and byte bounds.
`tests/web_integration.rs::phone_controls_exact_live_web_terminal_over_pinned_tls_and_revocation`
exercises the HTTPS-token gate, certificate pin, one-use pairing, screen over TLS, phone input to
the actual PTY, desktop takeback, and revocation. Android link parsing has unit tests. A physical
Android device and network are still required for final touch and camera acceptance.
