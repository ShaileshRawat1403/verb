# Mac browser companion

30 October 2026. Implementation and acceptance work in the conversation worktree. Public deployment
and Cloudflare acceptance must be recorded separately; a passing local fixture is not a public test.

## What runs where

The existing Verb HTTP host relays bounded browser commands and rendered results. An outbound Mac
process runs the existing isolated Chromium worker. No Chromium, Node runtime, public listener or
CDP endpoint is added to the phone. No PocketFabric or Android files, services or privileges change.
The UI identifies the paired Mac; `localhost` means that Mac. The Mac must stay awake and online.
Website code never runs in the Verb document. Saved HTML previews remain a separate sandbox.

The companion is human controlled. It does not add agent browser tools or automatic approvals.
Browser cookies/storage are temporary and are discarded on Stop/disconnect. Personal profiles,
Chrome tabs, saved passwords and cookies are never imported. Downloads/uploads, popup flows,
interactive page dialogs, WebSockets and durable login profiles remain unsupported. This is not a
claim of complete browser compatibility or parity with Claude/Codex.

## Pairing and credentials

Only a Verb owner can create or revoke a pairing through `/api/browser/pairing`. Read and drive
access tokens cannot do so. Studio shows a masked one-use code for two minutes, with an explicit
Copy action. It is removed when the panel closes, pairing completes or its timer expires. It is not
put in a URL, local storage, a command line, a log, a Spec or a file. Clipboard copying is explicit;
Verb does not later overwrite the person's clipboard.

The companion reads the code from stdin (without terminal echo), claims it once, then holds a
separate browser-only key in memory. Both secrets use 32 bytes from `/dev/urandom` and fail closed
if secure randomness is unavailable. There is no timestamp/randomness fallback for authorization.
The browser key cannot authenticate to project, terminal or other general Verb APIs.

The key expires after eight hours, is invalidated by a lost heartbeat (35 seconds), and is revoked
on Disconnect. Pairing and jobs are memory-only; host restart revokes them. Reconnection requires
fresh pairing. No browser control or CDP listener is exposed. The worker uses an authenticated
loopback egress proxy for website access. Remote origins require HTTPS with normal
certificate validation; redirects are refused, and insecure TLS overrides are rejected.

Cloudflare Access is retained. For an Access-protected host, first run the official
`cloudflared access login https://your-verb-host` interactively. The optional `--cloudflare-access`
flag obtains the existing user token through `cloudflared access token -app=...`, captures it without
printing it and supplies it in the `cf-access-token` request header. No Access policy or service token
is added. This user identity credential has the authority of that existing Access login; it is not
itself a browser-scoped credential. Neither it nor any Verb token is forwarded to the browser worker
or websites. Access expiry stops the connection; there is no automatic login or token replay.

References: [Cloudflare CLI authentication](https://developers.cloudflare.com/cloudflare-one/tutorials/cli/),
[agent authentication](https://developers.cloudflare.com/cloudflare-one/access-controls/authenticate-agents/).

## Use

Use a maintained Node.js 22 or 24 release, install the repository's pinned web dependencies and
provide a compatible installed Chromium binary.
From the Verb checkout on the Mac:

```sh
node desktop/web/runtime/browser-companion.mjs \
  --server https://your-verb-host \
  --chromium "/absolute/path/to/chromium" \
  --cloudflare-access
```

In Studio → Browser → Connect Mac browser, create the code and paste it into the runner's prompt.
Choose Check connection, then Start browser. Opening an address explicitly grants that HTTP(S)
origin; dependencies require separate site grants. Stop clears the browser session while leaving
the companion paired. Disconnect or Ctrl-C closes the runner's browser and revokes the connection.
No login item, daemon or background auto-start is installed.

## Delivery, cancellation and cleanup

Jobs have fresh secure IDs, are delivered once and are never redelivered after a missing poll/reply.
Only one job is pending. Stop can replace it; an older reply is discarded without confirming Stop.
Browser sessions rotate on every new start, independent of the longer-lived companion key. A stale
session cannot act on or stop a newer browser; worker control generations guard client takeover.

A failed fetch, timeout, worker crash or lost reply has an unknown outcome; neither side retries the
action. The runner closes its browser and requires pairing again after a broken connection. A remote
revocation rejects new calls/results immediately; the Mac closes its processes when it observes that
revocation, subject to the bounded in-flight command/network waits. It cannot promise instantaneous
remote process termination while the Mac is offline. Normal EOF and signals share one cleanup promise
so concurrent shutdown paths cannot exit ahead of Chromium cleanup. Arbitrary kernel failure and
forced SIGKILL cleanup remain outside the proven contract.

Replies are bounded at 3 MiB; text/errors use the existing redactor. Pixels are not automatically
redacted. Page content and user-entered text traverse the authenticated Verb relay transiently and
are not saved as browser history or evidence. This is application-level isolation, not an OS network
firewall; the existing worker's network containment limits still apply.

## Acceptance

`browser-companion.spec.js` uses an external-mode host with no local browser configuration and a
real outbound runner/Chromium. It checks one-use pairing, rejected credentials, denied general API
access, real navigation/input/inspection, rotated session IDs, stale Stop rejection, owner UI pairing,
revocation and disappearance of the runner's owned browser process tree. `browser-runtime.spec.js`
checks local navigation, grants, input, failures, start/close races, screenshot/input coordination,
viewport synchronization and phone layout. `browser-origin.test.js` guards the public-origin default
port case and local host aliases. Rust relay tests check delivery once, revocation and expiry.

Real Cloudflare login, WAN latency and public host acceptance require a separately recorded run.
