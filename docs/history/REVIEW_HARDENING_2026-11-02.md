Baseline — 2db0ce5 (preserved integration snapshot), review dated 2026-11-02.

# Review follow-up: 2 November 2026

The supplied architecture review mixes reproducible bugs with product gaps and host deployment
constraints. This change fixes the concrete defects below without changing the Android distribution
flavors or provisioning host isolation.

## Fixed

- Mobile Claude chat checks project stream opt-in before reading content and on subsequent polls.
  It shares the browser stream's redacted, 2 MiB tail reader and keeps at most 300 messages.
  Private reasoning, sidechain records and harness wrappers remain excluded. Revocation clears
  the next chat update. This does not revoke separately authorized terminal screen sharing.
- JSONL tails expose only newline-terminated records. A partially written UTF-8/JSON record is
  reread after it is completed, instead of advancing a cursor past it. RFC3339 timestamps preserve
  milliseconds and timezone offsets. Missing or invalid mobile timestamps use 0 for unknown,
  never the time Verb read the log.
- Screen hints may veto prompt input but cannot establish an agent's idle state. An assistant
  reply or an empty transcript is not sufficient evidence that a new prompt can be sent.
- Git config discovery and automatic Git reads use a common subprocess runner with nonblocking
  stdin/stdout/stderr, output bounds and process-group termination on failure. Ordinary Git reads
  and config discovery each have a three-second deadline and 2 MiB output bound; host/runtime
  probes keep their smaller limits. Subprocess failures stay unknown. Filter discovery fails
  closed on errors; an ordinary non-repository directory remains distinguishable from failure.
  State refresh jobs do not queue behind an already busy cache lock.
- The phone client buffers reply reads and reuses pinned TLS connections only when the host
  advertises `keepAlive`. Old clients and one-shot hosts remain compatible. The relay accepts
  bounded frames and at most eight simultaneous connections, validates every operation's token,
  and closes failed/disconnected sessions. Input is never automatically replayed after a lost
  reply. The client discards idle connections before the host's idle timeout.
- Spec commits require explicit file selections; unchecked files, including unrelated staged
  changes, stay out of the commit. Git receives literal paths, including rename sources, rather
  than a blanket `git add --all`. Choosing to record the audit also includes the spec file.
  The dialog lists every changed file and starts with no files selected.

## Still separate work

- OpenCode transcript support and a common delivery-confirmation protocol across all agents are
  new capabilities. Talk's provider print mode has a different purpose and was not replaced.
- Interactive conflict recovery, general branch management and stashing are additional Git UI
  features, not fixes to the selective commit boundary.
- Android executable-storage restrictions and the fullCli/play distribution split remain host
  and packaging constraints. No target SDK or local toolchain capability was changed here.
- Same-user filesystem and process-environment access still require operator-provisioned OS
  isolation. These changes do not create a sandbox around a user's own terminal.
- Subprocess deadlines do not bound arbitrary filesystem syscalls, project locks or kernel-level
  uninterruptible I/O. Killing a process group is best effort; a background reaper avoids holding
  an HTTP permit while awaiting termination. A complete filesystem fault-containment boundary
  requires a broader worker/supervisor design.

## Verification

Regression tests cover opt-in/revocation, redaction, exact timestamps, a torn UTF-8 line, hanging
children and inherited pipes, oversized output, full stdin, selective commits with staged scratch
work, unusual/renamed filenames, persistent phone framing, TLS pin rejection, 100 KB replies,
connection reuse, and avoiding input replay after a lost reply.

Run the desktop Rust suite, web unit/build checks, the selective-commit browser flow, and Android
DesktopBridgeClientTest/DesktopPairingLinkTest. Physical-phone acceptance remains a separate check.
