# Verb workspace adapter — local integration preview

An opt-in VS Code-compatible extension connecting existing IDE capabilities to Verb.
This slice provides capability discovery and explicit read-only saved-file snapshots.
It does not delegate editor writes, terminals, debugger or task execution.

## Commands

- **Verb: Connect to Local Workspace** prompts for an explicit loopback HTTP origin
  on the extension host (which may differ from the browser device), then masks the
  existing token input. Prefer a read-scoped token. Tokens stay in memory, never in
  settings, files, URLs or SecretStorage. No service or agent starts automatically.
- **Verb: Refresh Connection Details** rechecks project identity and capability metadata.
  A changed project disconnects; failures clear previous capability observations.
- **Verb: Open Read-Only File Snapshot** asks for one project-relative path and checks
  workspace ID, path, byte count and SHA256 revision. A virtual text document supplies
  syntax display using the IDE's file associations. No file is written or executed.
  Snapshots do not auto-refresh. The output labels the saved revision; reopen explicitly
  for newer bytes. At most eight snapshots are held. Disconnect clears provider content.
- **Verb: Disconnect Workspace** invalidates pending results and drops retained token
  references. JavaScript reference clearing is not secure memory erasure. The IDE may
  retain its own text document buffers; do not claim disconnect erases all IDE memory.

The IDE folder has **not** been proved to be the Verb project. A successful localhost
connection is not filesystem or execution authority. Open snapshots are disclosed to
that IDE and potentially its installed extensions, like other opened files. Use only
an intended, trusted extension host. Nothing is installed into an existing personal IDE.

## Contract

Verb owns project/session identities, capability decisions, revisions, evidence,
approvals and delivery. This client uses only the existing GET workspace/file routes.
It refuses remote origins, DNS hostnames, URL credentials/query/fragment and redirects.
Responses are bounded (16 KiB metadata; 2 MiB escaped JSON for 200 KiB text), deadline
covers the entire body, contract errors fail closed, and raw server prose is not shown.
There are no retries or background polling. Disconnect/project changes invalidate
pending content. Existing server file visibility/access checks remain authoritative.

VS Code commands, input/output and virtual document APIs:
https://code.visualstudio.com/api/references/vscode-api
Use this directory as an extension development path for a deliberate local trial.
Actual candidate IDE UI acceptance is separate from command API mocks.

## Validation

```sh
node --test test/*.test.cjs
VERB_BIN=/absolute/path/to/verb node --test test/*.test.cjs
```

The second form starts a disposable Rust host with a synthetic project, HOME, ledger
and token, verifies actual capabilities/file revisions/auth failures, then stops its
own host and removes its fixture. It does not contact public Verb or existing sessions.
Sixteen checks passed on 29 October 2026, including this native-host test. No claim of
finished workstation readiness, packaged IDE compatibility or mobile acceptance.

## Actual IDE acceptance — 29 October 2026

A separately owned code-server 4.143.1 container, with a synthetic project and a
real ARM64 Verb32a1cf7 host, installed the local VSIX successfully. Through the
real command palette: connected using masked token input, displayed project ID
`f0be6383bcffdeff830d5b292507f5c3`/native-local capabilities, opened `src/greet.cjs`
from Verb, displayed its saved revision `4d378426146c` and JavaScript language mode.
Ordinary typing was refused with “Cannot edit in read-only editor”. Disconnect
replaced the document with “Verb snapshot cleared” and cleared the connection.
No provider or extension marketplace request was made. The loopback UI proxy and
IDE used no host mounts; the IDE network was internal-only.

The generic IDE shell was not accepted as Verb's final UX. At the default narrow
in-app browser viewport its sidebars crowded out the editor; desktop acceptance
used 1440×900. This is not mobile usability acceptance. The existing derivative
image retained an invalid workspace-level extensions.autoUpdate setting, producing
a startup notification; it did not prevent adapter acceptance. The earlier lab
notes already identify that fixture setting problem. Theia acceptance of this
adapter, full editing/language/debug integration and target-phone suitability
remain unproven.

Final installed adapter checks: 16 passed, no skips; Prettier, shell syntax, workflow
YAML parsing and diff whitespace passed. Both acceptance containers were stopped;
the synthetic source file hash was unchanged. Full Desktop CI was not rerun for this
local adapter-only slice, and no remote CI result is claimed.

## Local installation package

`python3 package.py --check` validates an in-memory VSIX. To create an installable
artifact, use `python3 package.py --output /absolute/new/path/verb-workspace-0.0.1.vsix`.
The destination must not exist. Packaging includes only the manifest, runtime source,
README and Apache2 license; no tokens, test fixtures, node_modules or user files.
No marketplace publication or public service restart is needed. Install the VSIX
only in an intended local VS Code-compatible extension host.
