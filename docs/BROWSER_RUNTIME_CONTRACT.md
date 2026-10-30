# Native browser runtime and Mac companion

Native implementation: 28 October 2026. Companion update: 30 October 2026. Local worker execution remains restricted to native-local hosts. External hosts can use the separately paired companion described below. Deployment acceptance is recorded separately.

## Product behavior

Studio's Browser surface opens real pages beside the work thread. A separate Chromium process runs
website code; Verb displays its rendered pixels and sends explicit browser commands. This does not
relax Verb's CSP or embed external sites in the workspace document. The existing saved single-HTML
app preview remains a separate feature.

Implemented: four tabs, actual title/URL, back/forward/reload, explicit site access, click, text entry,
remote Tab/Enter/arrows, scrolling, 390/768/1100-pixel viewports, bounded readable page text and page
errors, visible client control, stop and fresh-session recovery. Viewport changes preserve page state.
A delayed default-file response must not replace a browser the person has already opened.

This slice is human-controlled, not an agent browser tool. Screenshot interaction is not a complete
accessible browser: readable text and keyboard controls are available, but a semantic page tree and
assistive-technology interaction need further work. Downloads, uploads, popups, dialogs, service workers,
WebSockets and persistent/imported login profiles are not supported in this slice. Pages needing them
may not work. Current snapshots refresh about every 1.8 seconds, not a full-motion stream.

## Explicit configuration and availability

The route requires an owner token and a native-local host: no boundary execution profile, allowed
public origin, Cloudflare Access configuration or non-local deployment mode. It is unavailable on
those hosts even when paths are configured. Nothing starts on a capability read or workspace load.

Three environment variables must name existing absolute files:

- `VERB_BROWSER_NODE`: supported Node executable (the acceptance fixture used Node 20).
- `VERB_BROWSER_WORKER`: this checkout's `desktop/web/runtime/browser-worker.mjs`.
- `VERB_BROWSER_EXECUTABLE`: a compatible Chromium executable.

The worker imports the pinned `playwright-core` 1.63.0 production dependency from `desktop/web`.
It does not download Chromium or discover/import a personal browser profile. The operator owns
browser/Node patching and compatibility. Configuration requires a separately approved host launch
or restart; this implementation does not change the existing host environment.

One ephemeral browser belongs to the running Verb host, not a durable project/browser registry.
The current Studio lifecycle closes it when switching away from its surface. Browser state is not
persisted or attached to a Spec. Durable project association and reconnection across host restarts
remain future work; do not present localhost as the viewing device's localhost. It is this host's.

## Isolation and network boundary

Rust starts a private NDJSON worker with a cleared environment; Verb tokens/provider credentials
are not forwarded. There is no exposed CDP or browser-control HTTP listener. Chromium uses its normal
sandbox and a fresh, nonpersistent context. Pages receive neither Node APIs nor Verb's origin token.

The worker owns a temporary authenticated proxy bound to loopback. Site access grants exact HTTP(S)
origins, explicitly through opening an address or allowing a blocked dependency. Request hooks and
the proxy check destinations. The proxy is needed because request hooks alone did not prevent an
unapproved redirect in the first acceptance run. HTTP forwarding and HTTPS CONNECT targets require
a grant before connecting. Direct loopback bypass is disabled; non-proxied WebRTC UDP is disabled.
WebSockets are refused. Verb's own origin and every destination using its server port are refused,
including loopback aliases. URL userinfo and non-HTTP(S) navigation are refused.

These are application-level controls, not an OS network firewall or an exhaustive hostile-browser
security proof. HTTPS tunnels are opaque after the allowed CONNECT. DNS/IP containment and explicit
policy for other local services are not implemented; an owner may explicitly allow a different local
service. Browser internals/updates, hostile origins and comprehensive network containment require a
separate security review before public enablement. There is no public enablement in this slice.

Grants, cookies and storage disappear on normal stop. There is no per-origin revoke yet: stop the
session to revoke all sites. Proxy credentials stay private to the worker/browser and are discarded.
The browser may contact a site's dependencies only after those origins are explicitly allowed.

## Control, delivery and lifecycle

A browser runtime identity changes on a fresh worker launch. Input also carries the controlling
client identity and generation. A takeover invalidates older input commands; an old browser identity
cannot stop or act on a newer session. Authenticated owner clients may capture the current page
without taking input control. Stop remains reachable during a pending action and cancels its wait.

Actions are serialized and bounded; busy requests are refused rather than queued. Disconnects,
timeouts and crashes never replay actions. A failed action may have occurred: inspect the page or
start fresh rather than silently retrying it. Browser state is not confused with chat delivery state.

Normal stop closes the worker and its Chromium descendants; acceptance checks the owned process
tree disappears. Closing stdin or receiving termination triggers cleanup. A forced worker crash
invalidates its runtime identity; starting again is explicit. Cleanup after arbitrary host/kernel
failure is not proven. Leaving the panel clears pixels/text and requests stop. Closing a browser tab
or losing connectivity is not a reliable server-side stop notification: the runtime can remain alive
until explicit stop or host exit. There is no idle expiry yet.

Page text/errors use the existing redactor, are bounded and are not written as a browsing history.
Pixels can contain sensitive information and are not automatically redacted. Start is explicit and
owner-only; screenshots and readable text stay transient in the client/response, not saved as evidence.
Verb's normal log opt-in does not authorize collecting unrelated browser content. Agent tool access,
recorded screenshots and consequential-action approvals need their own future contract.

## Verification and limits

`desktop/web/e2e/browser-runtime.spec.js` exercises real Chromium through the real Rust host and
throwaway HTTP projects. The site denies iframe embedding. Tests cover input, actual navigation,
blocked dependencies/redirects, own-origin refusal, fresh storage, stale runtime/control commands,
read-only observation without takeover, stop during navigation, forced worker failure, owned-process
cleanup, a delayed file-list response and narrow-screen rendering. No personal account or real agent
is used. HTTPS login flows and arbitrary production sites are not acceptance evidence from this suite.

REA 6.8.0 statically analyzed a temporary copy of the final worker: one file parsed, zero parse
failures, 2,129 visited AST nodes and 15 findings. Semantic coverage remains partial/unknown with
600 reported unknowns. REA is architectural evidence, not a security certification or a substitute
for the runtime tests. See BROWSER_ARCHITECTURE_RESEARCH.md for the app research and its limitations.

Resource samples are single Mac fixture measurements, not a phone-host or production benchmark.
The process tree's summed RSS includes shared pages and must not be called unique memory. Native
phone-hosted Chromium feasibility remains unproven. The separate Android application is untouched.

Latest checks: Rust all-targets with synthetic-boundary: 430 passed, five ignored; web units: 78 passed;
full end-to-end: 68 passed, one existing synthetic-boundary skip. After the last browser-only focus and
announcement refinement, all six browser tests and 78 web units passed again. Two targeted remote/read
access tests include GET/POST browser denials and passed. Build, syntax, Rustfmt, Clippy with warnings
as errors and diff whitespace checks passed. All hosts were disposable fixtures; existing services
were not used or restarted. The conversation suite uses a scripted agent, not real Claude.

Logs: `/private/tmp/verb-browser-rust-final.log`, `/private/tmp/verb-all-e2e-browser-final.log`,
`/private/tmp/verb-browser-polish-final.log`, `/private/tmp/verb-browser-denial-checked.log`,
`/private/tmp/verb-browser-web-unit-polish-final.log`. Final screenshots are in
`/private/tmp/verb-browser-polish-final/` and copied to the chat's persistent visualization directory.

The final single Mac sample: launch 754 ms, page open 418 ms, three captures 201/13/16 ms; summed
process RSS about 864 MiB, including shared pages. No claim of lightweight phone-host operation.

## Local preview and text-entry follow-up

The browser's text-entry draft now clears only after a successful response and only if the person
has not edited it while waiting. Busy/refused/uncertain requests preserve it; no automatic resend.
Enter in the text-entry field invokes the same deliberate Type into page action (it does not also
submit the remote page's form). The separate remote Enter control submits the page when intended.
The rendered image's accessible label explains where to find remote keyboard controls.

All six real-browser tests passed after this follow-up, including a simulated refusal preserving a
draft and a later keyboard-only successful entry; all 78 web units passed. Logs:
`/private/tmp/verb-browser-preview-polish.log`, `/private/tmp/verb-preview-web-unit-final.log`.
A user-requested separate local playground is running; its private launch metadata path is recorded
in the ignored handoff, not in this document. The existing port 4870 service was not restarted.

The misleading background-request notice found in the live preview has now been fixed. The permission
list is populated only by browser-context page request events (including redirects) and route hooks;
Chromium background connections remain denied at the proxy without becoming page permission requests.
The fresh-redirect denial and exact pending-origin list are checked by the real Chromium suite. See
UX_WALKTHROUGH_2026-10-28.md for manual verification and current test evidence.

## 30 October: companion path and reliability fixes

The owner may explicitly pair a separate Mac runtime using the contract in
`BROWSER_COMPANION_CONTRACT.md`. This permits rendered browsing through an external Verb host
without launching Chromium in its execution boundary. The local execution restriction above still
applies to the local worker. Saved HTML previews retain their independent sandbox and permissions.

Address drafts survive background captures. Navigation exceptions and HTTP errors are displayed as
failures, and errors belong to the originating tab. Switching pages clears stale inspected text and
synchronizes viewport controls. One deliberate action can wait for an in-flight screenshot before
its first send; it is cancelled if that refresh fails or the browser session/control changes. Other
busy actions remain refused, never retried. Stop cancels waiting input. Closing while Start is in
flight stops only the session returned by that Start. A delayed availability response cannot enable
Start in a newer, unavailable view. Explicit Select all, Backspace and Previous field controls help
with editing remote inputs. Each fresh browser start rotates its session identity.

A real socket-reset regression was found during repeated acceptance: unauthenticated/aborted
Chromium proxy connections and upstream response streams could emit an unhandled `ECONNRESET`,
ending the worker. Those socket/stream errors now terminate their request, not the process. A
fixture deliberately cuts a response and verifies an explicit navigation failure followed by a
successful page open in the same browser session. The forced-SIGKILL test explicitly cleans only
its recorded, fingerprint-verified descendants; this is fixture teardown, not a claim that arbitrary
worker SIGKILL automatically cleans up every browser process.
