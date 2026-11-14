# Conversation flow: local review candidate

This work lives in `verb-conversation`, branch `claude/conversation-flow`, on the local
Studio baseline `45d1f25`. Git history records the reviewed conversation snapshot; public release remains unobserved. This is not a deployed
release. Android is a separate app and is explicitly deferred.

## Two input channels

Chat uses `POST /api/terminals/{id}/chat`, with a client-generated message ID. It requires
Claude, the project's conversation stream opt-in, an identified conversation, corroborated
readiness, no unresolved delivery, and the sending side's existing terminal input lease.
Raw PTY input retains its existing route and lease. It is not gated by chat readiness, so
interrupting, answering the provider's own permission prompt, and recovery remain possible.
Verb does not approve provider tool permissions or alter provider permission settings.

Readiness requires a timestamped idle status for the exact conversation/process attempt and
a completed turn in the log. A partial line, unread backlog, missing/old timestamp, incomplete
turn, or permission sign closes chat. A new conversation with no completed history starts in
the terminal; Verb says why. Screen text is only a sign that can close chat, never permission
to open it. Real provider behavior still needs acceptance with the installed Claude version.

## Delivery truth

A send is recorded before terminal writing. The same ID is not written twice within an attempt,
even after its visible receipt is evicted. The attempt retains up to 4096 ID tombstones; beyond
that, chat refuses new IDs and directs the person to the terminal. Visible receipts are bounded.
A lease refusal before writing is failed. Any error once writing is attempted is unknown and
blocks another message until explicitly resolved. No message is automatically resent.

Confirmation needs the same provider conversation, a post-send log offset, a nonempty provider
turn UUID, a usable provider timestamp, and a fingerprint of the complete submitted text.
Redacted or truncated display text is never confirmation evidence. Fingerprints are private,
in-memory fields, excluded from serialization and cleared with conversation content.
A missing receipt in a browser snapshot cannot prove that a delayed request was never sent.
Unknown delivery requires checking, explicit resend with a duplicate warning, or discard.
Delivery records are in memory: a process/Verb restart cannot prove an old pending send failed.

## Privacy

Stream opt-in is off by default and per project. Content comes through the canonical redacting
transcript parser. The old `/api/terminals/{id}/stream` content endpoint is removed.

Turning the stream off synchronously clears all mounted panels in the page, including hidden
Sessions and Studio panels, and removes sent-message text from this tab's conversation storage.
Unsent drafts have separate storage and are preserved. A page-wide barrier blocks late responses
while the off request is pending; confirmed settings generations reject older responses.
Hidden mounted panels continue polling only the stream setting, so hiding a panel does not pause
privacy revocation. Disposed panels reject late content responses. Acknowledgements follow the
page-wide clear. Other browsers/devices report their own acknowledgements; an offline client is
never assumed cleared. Provider-owned logs are never modified. Structural readers remain independent
of content opt-in.

The new phone relay protocol is available on the desktop, but the Android client still uses its
old protocol/UI and is not accepted as an implementation of these new delivery/privacy states.

## Review and testing

Regression coverage includes redaction and long-message collisions, uncertain writes, missing
receipts, incomplete turns, stale idle metadata, receipt eviction, hidden-panel clearing, preserved
drafts, and desktop/phone composer containment. Browser journeys use a scripted Claude stand-in,
not a live model. The fixture explicitly writes completion markers to corroborate idle status.

Live acceptance is still needed for bracketed paste plus Enter, metadata during a provider
permission prompt, and conversation identity across a real resume. Do not restart the user's
port-4870 server to perform it without explicit permission: that ends its active sessions.
No deployment, public service operation, or PocketFabric Node modification is authorized.
