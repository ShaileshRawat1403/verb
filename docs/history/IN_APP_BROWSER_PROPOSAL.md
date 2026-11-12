# In-app browser: limited-embed proposal (superseded)

Status: superseded by `BROWSER_ARCHITECTURE_RESEARCH.md`. The user wants a capable browser runtime; this limited iframe design does not meet that goal. It remains recorded for context and is not approved or implemented.

Original status: proposal only. Verb's current saved-HTML preview remains isolated and uses its existing
content-security policy. This proposal has not been implemented, enabled, or deployed.

## User outcome

A Browser surface beside the current work and project previews lets a person enter an HTTP or
HTTPS address, inspect a site, reload it, move between addresses entered in Verb, and open the
requested address in their normal browser. It sends no URL to a server-side proxy and launches no
server process: the viewer's own browser requests the site. On a phone, localhost means the phone;
it does not mean the phone server. URLs and navigation history live only in the active tab.

This is an embedded web view, not a full browser engine. A site's frame policy, cross-origin
restrictions, third-party cookie rules or mixed-content restrictions can prevent display, sign-in or
interaction. The interface must say that the address is the one requested in Verb, not claim to
know where a cross-origin page navigates, and never claim a page loaded successfully based solely
on an iframe load event. The normal-browser action uses `noopener noreferrer`.

## Proposed boundary change

Verb's shell currently restricts frames to its own origin when the Studio workspace is connected,
and its saved-app iframe uses its own stricter sandbox policy. Enabling a general web view would
require allowing external `https:` and `http:` frame destinations in the shell's content-security
policy. That is a real widening of allowed content sources. The saved-app endpoint's sandbox and
CSP must remain unchanged. The proposed web iframe uses `sandbox="allow-scripts allow-forms"`,
without `allow-same-origin`, top navigation or popups, and does not delegate camera, microphone,
geolocation, payment or clipboard permissions. No Verb token, referrer or credential-bearing URL
should be included. Reject non-HTTP(S) URLs, embedded usernames/passwords and Verb's own origin.
Destroy the frame when leaving the Browser surface. Actual browser cookie behavior still depends
on the viewer's browser and site policy; sandboxing does not guarantee a site receives no cookies.

This approach also allows network requests from the viewer's device to the entered destination;
local and private addresses may therefore reach services available to that device. It does not
provide server-side navigation, cross-site browser history, DevTools, screenshots or agent control.
Those require a separately designed browser integration and its own isolation and permission model.

## Acceptance before implementation is considered ready

- Verify the new shell policy and that the saved-app preview's restrictive CSP remains unchanged.
- Test valid/invalid addresses, no automatic navigation, address history, reload and close.
- Verify the external frame cannot access the parent or Verb APIs and does not receive a referrer;
  check permission denial and termination on close/navigation.
- Test embedded-page failure or refusal with honest copy and an external-browser fallback.
- Exercise desktop, phone, keyboard access, dark/light themes and narrow viewports.
- Validate isolated and public deployment modes separately; no deploy is implied by approval.
