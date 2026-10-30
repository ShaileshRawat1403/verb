# MCP connections v1

Implementation contract, 30 October 2026. Acceptance and deployment must be recorded separately.

Verb is an MCP host/client. A connection belongs to the currently served project and is explicitly
created by its owner. Agent-managed MCP configuration remains independent: Verb neither imports
credentials nor claims to control calls inside a separately running CLI agent.

The first slice targets remote Streamable HTTP servers through the official SDK. HTTPS, normal
certificate validation, public-address validation, no redirects, bounded requests and no ambient
proxy/cookie/credential forwarding are required. Loopback, private networks, metadata endpoints,
URL credentials and Verb itself are refused. A separately supplied service credential stays in
memory; the API never returns it and the browser never persists it. Verb owner/access tokens
must never be forwarded to a connector. OAuth discovery and local stdio execution are separate
follow-ups, visibly unsupported rather than approximated.

Connections exposes server identity, runtime location, tools/resources, state and Disconnect.
Nothing runs just because a server describes itself as safe or read-only. Each tool call has a
one-use, expiring review record bound to the connection generation, exact tool and arguments.
Changing arguments, disconnecting, reconnecting or changing the catalog invalidates review.
Consuming a review is atomic and precedes dispatch. Transport failures after dispatch have an
unknown outcome; writes are never automatically replayed. Repeated clicks cannot duplicate a call.

Connector content is external and untrusted. Results are bounded and rendered as text, not HTML.
Use in Studio puts an attributed result into a draft for the person to inspect; it does not send
that content to an agent, create an approval, execute code or change a Spec stage automatically.
Secrets, arguments and result bodies must not enter persistent audit records by default.

Acceptance requires real SDK peers for discovery, resources and a reviewed tool call, plus tests
for revoked/expired review, duplicate dispatch, disconnect during a call, oversized/unreadable
responses, unsupported authentication, private-target rejection, token-scope denial and keyboard/
phone recovery. CI must pass the exact source revision before a public release.

## Supported surface and limits

More → Connections accepts anonymous or service-specific Bearer-token Streamable HTTP endpoints.
Simple string/number/integer/boolean/enum inputs have named fields; complex inputs use explicit JSON.
Unsafe JavaScript integers are refused before review rather than silently rounded. Text results are
readable by default, with raw protocol data available separately. Owner setup does not automatically
give a CLI agent access to these connections. The explicit Studio action attaches external content
to a work draft, preserving an existing draft/context; it neither sends chat nor creates evidence.

Project scoping determines where a connection is registered. It does not narrow the remote token's
permissions; those remain whatever its issuer granted. No credential is read from another app,
provider configuration, project file, environment or personal browser. Restarting the host clears
connections/reviews and requires reconnecting. Disconnect does not undo a remote action already
received by the service. Missing responses remain unknown, with no automatic request replay.

There are at most four connections, four active DNS lookups, one operation per connection and
sixteen pending 90-second reviews. Requests have a 25-second deadline and 256 KB raw response limit.
Catalog pagination is bounded to four pages and 200 items per kind, 512 KB combined; repeated IDs
or cursors are refused. Displayed results are limited to 64 KB and Studio attachments to 3,800
characters plus an explicit truncation notice. These are bounded metadata/content reads initiated
by the owner, not background monitoring. Owner-only status includes active-request, pending-connect
and unexpired-review counts so release operations can preserve ongoing work.

Only initial discovery/version negotiation can fall back to older MCP revisions. This uses the
SDK lifecycle, including legacy middleware errors, and never replays a tool invocation. Unsolicited
subscriptions, elicitation, sampling, tasks polling, rich MCP Apps, OAuth sign-in, persistent
credential storage and stdio execution are not implemented in this slice. A returned input request
or remote task is shown as such; receipt is not presented as completed work.

## Implementation and verification

The Rust runtime uses the [official rmcp SDK](https://docs.rs/rmcp/3.8.0/rmcp/) with a bounded HTTP
adapter and the existing ring TLS provider. The TypeScript server SDK is a development-only peer
for real UI → owner API → MCP transport → server → result acceptance. The release binary cannot
use the explicitly configured loopback HTTP fixture: that path is behind the existing synthetic
boundary feature, which refuses release compilation. Production still pins public DNS addresses,
requires HTTPS and certificate verification, refuses redirects/proxies and never forwards Verb
credentials. [MCP annotations are hints](https://blog.modelcontextprotocol.io/posts/2026-03-16-tool-annotations/),
not an authorization policy.

Local validation includes exact/repeated/expired approval, legacy negotiation, JSON/SSE, resource
reads, authentication failure, bounded responses, disconnect races, phone/keyboard access, named
and JSON input preservation, and a normal-build HTTPS read against DeepWiki's public MCP endpoint.
No user project content or credentials were supplied for that live check. REA inspected bounded
selected SDK/UI source; its explicit coverage limits remain, so those findings supplement rather
than replace execution tests. Public rollout and browser acceptance are recorded independently.
