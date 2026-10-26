# Studio workspace bridge v1

Status: native-local bridge and opt-in monitored-boundary adapter implemented, 26 October 2026.
The boundary adapter is not enabled or accepted on the public service yet.
This is the first working edit → saved app preview → Git review slice. It is not a
complete IDE integration, a deployment mechanism, or an arbitrary dev-server proxy.

## Authority and ownership

Verb remains the owner of project identity, Spec selection, evidence, approvals,
Git actions and existing PTY sessions. Studio is another presentation of that state.
No IDE sidecar, provider login, package installation or second terminal store is added.

The native bridge is enabled only when all these are true:

- Execution-profile resolution succeeds and returns **no configured profile**.
  External, account-isolated and invalid configurations fail closed.
- No public allowed origin, Cloudflare issuer or audience is configured.
- Deployment mode is LOCAL.
- The client has drive or owner scope for editing and authorizing application scripts.

Read scope can inspect the native file endpoint, as it already can inspect project
file previews, but cannot save or authorize app previews. Capabilities tell read-scoped
clients that editing/preview are unavailable. All APIs retain the existing token/session,
Host and Origin checks. Node-owned controllers, helpers, recovery records and policy
are unchanged. The monitored-boundary adapter requires explicit `VERB_STUDIO_BOUNDARY=monitored-v1`
plus a valid external execution profile. It never opens or writes project files as
UID995. The existing fixed boundary client admits a bounded pipes command running
a fixed `python3 -I -S -c` adapter inside the registered project's environment.
The adapter source (`desktop/scripts/studio-workspace.py`) is embedded in the host
executable and passed as one literal argument; project text is never executable code.
A fixed base64/zlib bootstrap keeps that argument below 4 KB for the installed client's
transport limit. `studio-workspace.wire` is checked byte-for-byte against the readable
source by the helper contract tests, which execute the actual wire representation.
No controller, launcher, registry, policy, guest image or Node source changes are needed.
No guest helper installation is required. Python uses only standard-library modules,
with isolated mode and site loading disabled. Git visibility uses a fixed system Git
with repository hooks/fsmonitor disabled and bounded literal-path listing output.

Capability discovery verifies the actual project UID, protocol, source revision, size and preview kind from
an actual boundary exchange. The source revision is a correlation value passed by
the host, not independent guest attestation. Release acceptance verifies the clean
host binary hash, which includes the fixed helper source. Saves probe before sending
any mutation and validate the resulting path, length, SHA-256 and submitted text.
Missing/incompatible Python or malformed replies disable capabilities; operation
errors never fall back to host filesystem access.

JSON travels over bounded nonblocking stdin/stdout/stderr, not interpolated shell text
or command-line file contents. The fixed adapter source is the only executable argument. A 15-second guest timeout and 20-second host deadline
bound exchanges. Saves are serialized by the host across helper processes. Every
operation remains subject to the existing controller's admission and cleanup. Capacity
is reported as temporary unavailability. Transport failure after a save has an unknown
outcome: reload and compare before retrying; there is no automatic retry.

Public enablement remains pending installed-boundary acceptance and reviewed rollout.
No PocketFabric Node file or service is changed by this implementation.

## API

| Operation | Contract |
| --- | --- |
| `GET /api/studio/workspace` | Schema version 1, trusted workspace ID, explicit editing/preview availability and limits/reason. Boundary discovery runs only the fixed read-only capability probe; no project code or edits. |
| `GET /api/studio/file?path=…` | Connected-workspace visible UTF-8 file: path, text, byte count and SHA-256 content revision, with workspace ID. |
| `POST /api/studio/file` | Drive scope; workspace ID, path, expected revision, replacement text. Returns the confirmed saved snapshot. Saving neither commits nor publishes. |
| `POST /api/studio/preview` | Drive scope; workspace ID, HTML path, expected saved revision. Returns a scoped one-use preview URL and exact revision. No arbitrary target URL or port. |
| `GET /studio-preview/<ticket>` | Connected-workspace iframe navigation using an unguessable document capability; serves exactly the authorized immutable snapshot once. Cannot authorize another API or file. |

A preview ticket expires after 120 seconds if not opened. At most four pending
snapshots exist per host, each bounded by the 200 KB file limit. Requests are answered
on bounded background workers so file IO and preview delivery do not occupy the PTY
controller loop. Content responses are no-store and set no cookies. The owner's token
never appears in the app document or its URL. A consumed/expired ticket returns 410;
refreshing via Studio creates a new ticket.

## Files and saves

The first adapter edits existing, Git-visible UTF-8 text files up to 200 KB. Ignored
or absent files, invalid paths, binary/non-UTF-8 files, symlinks and hard-linked files
are refused. Directory traversal uses no-follow directory descriptors. Saving also
requires a writable file owned by the native host user. Basic Unix permission bits,
including executable bits, are preserved; this is not an extended-metadata editor.

Writes use a uniquely created owner-only temporary file in the same directory, fsync,
revision/inode/mode checks immediately before atomic rename, and directory sync. Failed
exclusive creation never causes removal of an unowned temporary name. The save endpoint
alone has a bounded JSON limit large enough for escaped 200 KB text; other API body
limits are unchanged. Workspace and detected source-version mismatches return 409.

This is optimistic revision checking, **not a filesystem-wide compare-and-swap**.
Independent writers do not share Verb's in-process save lock. A change completed before
our final check is detected; there remains a narrow check-to-rename race with an
uncooperative concurrent writer. Do not claim universal concurrent-editor safety.
A directory-sync error after rename is reported as an ambiguous save, never retried
automatically. Reload/compare before deciding how to continue.

## Draft and recovery model

- Drafts remain in memory in the current tab, not persistent browser storage.
- Up to five file drafts are retained. Clean entries can be evicted; unsaved or
  recoverable entries are not silently discarded. Closing/reloading a dirty tab
  receives the normal browser unsaved-work prompt.
- Save captures the submitted text. Typing while that request is in flight remains
  a newer unsaved draft when the response arrives.
- A conflict preserves the draft. Load disk version retains the previous dirty draft
  as a recovery copy; Restore previous draft is explicit. Saving remains separate.
- Switching files or phone Canvas/Work focus preserves drafts. Workspace IDs and file
  paths are matched again in responses to avoid applying a reply to another target.
- The confirmed save supersedes older read requests. Git and Spec observations are
  refreshed; a save itself is not acceptance evidence or a deployment receipt.

## Application preview

The supported app is a **self-contained HTML file** with inline CSS and JavaScript.
App preview is an explicit action and uses saved bytes, not an unsaved editor draft.
The UI shows the source path and saved revision. Refresh restarts the app. Stop,
leaving Studio, changing the canvas file or switching out of app mode removes the
iframe and ends that preview execution.

Both the iframe attribute and the HTTP response enforce `sandbox allow-scripts`,
without `allow-same-origin`, popups, forms or top navigation. The native shell has a
specific same-origin frame exception; ordinary responses retain anti-framing headers.
The preview response has its own CSP: no resource loading by default, inline scripts
and styles, data images/fonts, no API connections, no forms, no base URL and no child
frames. Its opaque origin cannot share Verb's DOM or storage. Studio accepts no app
`postMessage` commands. This follows the [HTML sandbox model](https://html.spec.whatwg.org/multipage/iframe-embed-object.html)
and [CSP sandbox behavior](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/sandbox).

The ticket's expiry does not terminate a document already loaded. Browser scripts are
not a server process or a certified CPU/network resource sandbox. This slice does not
supply dependency loading, external assets, fetch APIs, multi-file serving, WebSockets,
hot reload, dev-server discovery or an arbitrary localhost proxy.

## Next adapter

The text editor is a replaceable Studio surface, not a claim of VS Code parity.
Completion, project search/refactoring, debug adapters, compatible extensions and
multi-file app builds still require the full IDE foundation. The local comparison
continues to favor a VS Code-derived baseline first, with Theia retained as a contender.
Any future IDE backend must obey the same workspace/authority ownership rather than
acquiring independent broad host access. Public editing/preview additionally require
a reviewed boundary-side contract; that work is outside the current Node restriction.

## Acceptance

Native browser journeys exercise real file edits and Git review, an interactive app,
opaque-origin parent/storage/API probes, stale revision and workspace rejection,
ignored/link/size/auth cases, single-use tickets, remote capability denial and read
scope denial. They also exercise per-file phone drafts, continued typing during a save,
recoverable reloads, source freshness and preview shutdown on navigation. Rust tests
cover native path/save/ticket invariants; existing HTTP/terminal integration tests
remain required. These are local synthetic-workspace results, not phone-host or public
acceptance, and not universal extension/language compatibility evidence.

## Public release gate

A UI-only release can leave `VERB_STUDIO_BOUNDARY` unset. Full Studio enablement needs:

1. Exact clean candidate committed and reviewed, Linux/macOS CI and ARM64 release build.
2. Fresh read-only public inventory: executable/launcher hashes, fixed guest Python availability, active user work, one ledger writer and no unresolved cleanup. Do not
   start the disabled private writer or touch Node recovery records.
3. Review and approve a concrete Verb-only transaction covering the host executable,
   explicit opt-in, backups of the executable and launcher, and
   preservation of all newer ledger/project data. No guest image or Node artifact may be modified.
4. Before public enablement, run the embedded adapter in an owned disposable file in the existing registered project
   through the installed boundary, proving its actual project UID, conflict preservation,
   blocked paths, large stdin, admission/refusal and cleanup. Synthetic tests are not this proof.
5. Browser acceptance through Access: save the owned fixture only, preview sandbox,
   scoped-token denials, existing terminal reconnect and final cleanup. Roll back only
   Verb artifacts/launcher on failure; never overwrite current ledger/project data.

A successful local or CI run does not authorize interruption of active user terminals.
General dev-server proxies, external assets, WebSockets, LSP/debug/extensions remain
outside this single-file Studio pilot and must not be advertised as delivered.
