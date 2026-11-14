# Saved HTML preview loading

The preview is an immutable rendering of a saved file, with a short-lived one-use navigation
capability. The iframe and HTTP CSP retain `sandbox allow-scripts`, with no same-origin access,
external connections, forms, nested frames, or privileged browser APIs. This is separate from
Verb's optional native browser runtime.

## Loading and recovery

Inserting an iframe is not proof that its document has loaded. Studio shows a loading notice
and `aria-busy` until a small response-only Verb readiness script reports DOMContentLoaded.
The parent accepts that message only from the current iframe's WindowProxy and its exact
preview ticket. Messages from other windows or previous previews cannot mark it ready. The
message confirms document loading, not test success or acceptance-criterion evidence.

The readiness script is added after the original doctype where present. The saved file,
source view and revision hash are unchanged. It adds no permissions or network access.
After 12 seconds without confirmation, Studio explains that loading is unconfirmed and offers
its existing explicit Reload and Close controls. Late successful loads can still confirm;
there is no automatic resend. Reload obtains a fresh snapshot/ticket. Closing or changing modes
invalidates pending asynchronous preview requests and removes the current iframe/timer.

Preview responses keep `Cache-Control: no-store` and add `no-transform`. This requests that
intermediaries preserve the isolated document rather than injecting analytics or rewriting HTML.
Cloudflare documents this directive for supported response transformations and analytics injection:
https://developers.cloudflare.com/rules/configuration-rules/response-body-inspection/
https://developers.cloudflare.com/web-analytics/faq/
No Cloudflare Access, WAF, DNS, or host policy setting is changed.

## What the public investigation established

The origin returned non-empty, correct HTML with the expected CSP. A fresh public preview rendered
after Reload and its counter changed from zero to one. The browser console showed blocked
Cloudflare-injected analytics and a sandbox cross-origin access error; these do not prove the
cause of the earlier blank navigation. They are not a reason to relax CSP or grant same-origin
access. The confirmed Verb defect was reporting readiness immediately, with no load confirmation
or stalled-load explanation. Background browser/control interruptions complicated the original
acceptance run. Do not claim a specific network or browser root cause that was not established.

## Regression scope

Rust coverage checks doctype preservation and safely escaped readiness data. Browser acceptance
holds the iframe request, rejects an unrelated window's readiness message, observes the stalled
state, resumes and reloads the real document, and closes during a pending reload. Existing tests
continue checking script interaction, blocked parent/storage/API access, one-use capabilities,
read-scope denial, source revisions and explicit saves. Host-specific rollout and public acceptance
results are recorded privately.
