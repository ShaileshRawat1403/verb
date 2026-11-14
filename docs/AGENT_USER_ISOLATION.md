# Opt-in agent account separation

Linux and rooted Android chroot only. `VERB_AGENT_USER` opts in; bad configuration must refuse
launches rather than falling back. Run Verb as a dedicated non-root host account, with private
home/tokens/state. A fixed root-owned launcher, permitted by a narrow sudo rule, launches PTY,
Talk and project tools as a different non-root agent account. The agent receives a clean
environment and its own home and logins; never copy root's credentials.

The launcher joins a root-controlled memory cgroup, checks its limit and OOM eligibility, sets `no_new_privs`, then
executes the program. The agent can join this leaf but cannot change limits or move into an
unlimited ancestor. This first budget is aggregate across agents and descendants, not per session.
Provision only on an existing memory controller; never mount/unmount Android's global filesystems.
On Android, reset inherited Magisk OOM protection as root in the dedicated service chain before
changing accounts. The launcher requires oom_score_adj 0 and refuses agents that can lower it.
Verify enforcement with a bounded allocator and the kernel OOM counter; a finite limit alone is
insufficient. Use a private root-owned sudo bridge via `VERB_AGENT_SUDO` on nosuid storage, and
expose only the existing memory controller in the private chroot namespace.
If the required controls are unavailable, activation refuses. Node/Bun address-space reservations
make an address-space ulimit an unsuitable substitute.

Keep projects in a shared group outside protected homes. Permit the host to read the agent's
records for recovery/observation, not the reverse. Put shell integration in a separate host-owned,
agent-readable directory. Preserve the session lock across sudo; disable sudo's extra PTY and
input/output logging. Execute project Git hooks and runtime probes as the agent too, so a repository
cannot acquire host privileges via a commit. The host itself must never be root in this mode.

Shared agent UID is an owner-secret boundary, not per-session or multi-tenant containment. Kernel
attacks, network egress and agent-to-agent isolation are out of scope. Agent-side CLI access to
host-private storage is unavailable; use scoped HTTP tokens for automation. Existing root-home
conversation recovery requires an explicit migration, not a silent move. Keep the old deployment
and state for rollback and use separate fresh workspace/state for acceptance.

Prepare and test locally before any live installation, service restart, project copy or reboot.
Acceptance must check UIDs/groups, capability masks, credential denial, memory limit/membership,
agent launch, terminal teardown, exact recovery and boot supervision on the device.

`no_new_privs` prevents exec from granting setuid/file-capability privileges and is inherited;
it does not replace accounts or memory control. See
[Linux kernel documentation](https://www.kernel.org/doc/html/latest/userspace-api/no_new_privs.html).

Canonical launcher: `desktop/scripts/verb-agent-launch`. Disposable Linux acceptance:
`desktop/scripts/test-agent-isolation.sh` after building a Linux release binary. Desktop CI runs
this fixture on Linux with separate accounts, real sudo and a finite Docker memory cgroup; it
requires no private node repository or provider credentials. PocketFabric owns Android mount views,
account provisioning and service installation; its installer consumes this launcher from Verb.

## Recovery with private native records

Claude/Codex may create record files or parent directories the host cannot read. The host now
runs a bounded `__agent-recovery` metadata check through the same launcher under the agent UID.
Only identity and yes/no/unknown verdict return; transcript text and credential files do not.
Discovery filters by project/start time and refuses ambiguous new records. Probing is asynchronous
so it cannot stall terminal byte delivery. Identity is persisted when discovered so a host
interruption does not erase it. Exact-ID verdicts are checked under the agent account too.

This does not enable conversation streaming or observation through unreadable files: those remain
unobserved. AGY's native exact conversation resume has been tested, but automatic Verb recovery
remains unconfirmed because its interactive binary record format has no supported adapter.

Authenticated candidate acceptance: Claude/Codex/AGY responses and native exact marker recall;
real Claude/Codex hosted PTYs, private-record recovery and same-session resume. Claude-created
Python tests passed. Codex command/edit execution failed under its native workspace sandbox on the
phone: the kernel has CONFIG_USER_NS disabled. No sandbox bypass, privilege increase or security
setting change was applied. Authentication and chat success do not prove command capability.


## 2026-10-14 external-boundary candidate checkpoint

Separate manual fixtures on the unchanged OnePlus kernel passed fresh Claude and
Codex sign-in, completed native shell-tool events, exact synthetic file checks
and CLI exit 0. Their temporary homes contained no copied existing logins. The
Codex fixture explicitly used per-process external isolation instead of its
unsupported native sandbox; this was not an automatic compatibility fallback.
The native kernel sandbox limitation remains. These fixtures did not run through
Verb's current launcher, and do not prove integrated resume or controller recovery.

PocketFabric now has a separate candidate control foundation: persistent project
and broker identity reservations, pinned storage handles, bounded typed requests
and connection/message peer authentication. Twenty-three controller checks and
17 network-policy regressions passed on the OnePlus. The owner selected public
HTTPS for daily project terminals, retaining private-address blocking. Existing
strict fixture defaults and live Verb behavior remain unchanged.

A dedicated process scope now passes eight real process lifecycle checks on the
OnePlus, including detached descendant cleanup, true exit status and fail-closed
pidfd checks. Shared Linux setup primitives and bounded DNS/header deadlines also
passed, for 46 additional component regressions plus two disposable namespace,
PTY and unauthenticated HTTPS executions. These use synthetic files and no saved
provider logins. This is not yet an installed supervisor service.

Durable generation-aware leases and exclusive controller ownership now also
passed 51 component checks and a same-boot forced-controller-crash proof on the
OnePlus. A replacement process acquired control and stopped a detached synthetic
actor using its recorded cgroup. A stale stop did not affect a replacement launch.
Shared atomic metadata now serves both the identity registry and lease journal.
No controller daemon or Verb integration was installed by these proofs.

The assembled execution service, resource/storage admission and production
recovery lifecycle, native Verb adapter, persistent-project authentication, candidate endpoint and full
resume/reconnect/stop/restart/rollback acceptance remain open. No production
promotion or automatic unrestricted-host fallback follows from this checkpoint.


## Resource and reply verification, 2026-10-14

PocketFabric commit `29b9bdf4ff9930a67ea79e6231b84fa3c5e91d78` adds private
cpuset bounds, guarded quota admission and reciprocal typed PTY/pipe replies.
Seventy component tests passed on native ARM Linux and the OnePlus. The private
CPU proof stayed on logical CPUs 0–1 despite attempts to widen affinity, refused
priority escalation, and removed its own group and mount. A procfs ESRCH exit race
in supervision/identity scanning was fixed; inaccessible process evidence still
fails closed. Reply tests preserve separate stdout/stderr, reject foreign/stale
session identities and close rejected descriptors.

Persistent storage enforcement remains unaccepted. The phone's native F2FS
project quotas record usage but do not enforce limits. After the owner's approval,
the runtime enforcement-enable call returned ENOSYS; quota policy stayed unchanged.
Admission now rejects accounting-only quotas before mutation. No boot/mount policy,
kernel, live launcher or public routing was changed. All synthetic quota data and
new CPU groups/mounts were cleaned; live Verb health remained HTTP 200.

This is still component verification, not an installed execution supervisor or
Rust integration. The separate candidate and authenticated persistent-project
resume/reconnect/stop/controller-restart/rollback checks remain open. Existing
live Codex launch behavior has not been switched to external isolation.


## 2026-10-15 private native integration

`VERB_EXECUTION_BOUNDARY=monitored-v1` selects the separate private candidate.
It is mutually exclusive with `VERB_AGENT_USER` / `VERB_AGENT_SUDO`, Linux-only,
and requires the dedicated host identity, private host state, a fixed root-owned
unprivileged client and an operator-registered project. Invalid configuration
refuses; no host/root execution fallback is allowed. Existing account separation
and ordinary desktop launches retain their previous behavior.

PocketFabric owns the root controller, per-project actor/broker identities,
pinned mount views, resource admission and durable recovery. Verb retains its
host-side session lock and terminal, while the client relays a separate private
PTY or three distinct pipes. Exact root-issued generations bind completion and
stop. Native histories stay in the project's private home; the bounded helper
returns only recovery identity/verdict metadata. Host shell shims are not sent
into the guest. Unobserved native records remain unobserved.

The candidate runs independently on OnePlus loopback 3007; public port 3005 is
unchanged. Synthetic device/API/browser checks passed native file ownership,
input (including queued startup input), resize, Ctrl+C, hidden input, native exit
status, home persistence, close cleanup and controller crash recovery. Fresh
persistent Claude/Codex sign-ins were completed by the owner. Both providers
then wrote synthetic files through shell tools as UID 62000. Each resumed its
exact native conversation and wrote a remembered marker omitted from the new
prompt, without reading the earlier file. Verb Resume retained the same Verb
session ID and launched that exact native conversation. Fresh output attachments
kept the process alive. A browser reload retained the same shell PID and output.
Closing a terminal stopped its detached synthetic child. The installed runit
controller also recovered after SIGKILL, drained the recorded child and lease,
and admitted a new shell. These tests used only the registered synthetic project.
Claude disables optional startup traffic and its auto-updater in this candidate;
project-configured MCP remains native. Provider output and login contents were
not copied into either repository.

The selected storage control samples native accounting and stops at a budget;
it is **not a hard disk quota**. The private candidate has explicit memory,
cpuset, file, inode, lifetime and public-HTTPS limits. It is owner-only and is not
certified for hostile multi-tenant hosting. Codex's native kernel sandbox remains
unsupported: the selected candidate uses an explicit external boundary, without
changing saved native sandbox settings. The private candidate now runs clean
source revision `5e9884193bf4de3be629092eb67586c1e5af9e8b`, binary SHA256
`e3a4c72429af44440b9b15adf458d5b24337aa0b0cc1de24d460218723dd3f6e`.
Both the previous-package rollback and restoration of this clean package passed,
including native home persistence and exact Claude/Codex Resume after restart.
The installed clean controller repeated the forced-crash proof successfully.
Desktop CI run `38670697911` passed macOS and Linux checks. Public promotion,
reviewed real-project migration, new-boot acceptance and sustained use remain
open. Registration currently creates empty projects through a root operator tool;
AGY/OpenCode full workflows and arbitrary CLI adapters are not certified here.

The responsiveness follow-up installs clean Verb revision
`5c3661ede775549de9017b07f0009e17d661a7c6` with boundary revision
`22b706b05399a46a1a78d233f722bbfd43bd0bda`. Read-only Git runs in bounded
workers, and background pipe clients cannot occupy both execution slots. A short
interactive priority window avoids probe starvation; only explicit pre-lease
capacity rejection is retried for three seconds. No accepted command, uncertain
transport failure or issued generation is retried. Native input and two-session
reload/close passed on the phone. See `DESKTOP_PRODUCTION_READINESS.md` for the
measured latency and current proof scope.

The cold-state follow-up batches Git snapshot reads through the existing
root-owned `verb-recovery` executable, inside one external project session.
Its internal `__git-snapshot` command runs the same guarded Git observer under
the project UID; it does not execute Git on the host. A ten-second guest deadline,
one-second kill grace and bounded metadata response apply. An absent, old or
failed helper reports Git as unavailable, with no host fallback. Deploy the host
binary and its read-only guest helper together. The helper skips ordinary CLI
root discovery to avoid recursion and only maps a confirmed guest workspace root
back to the registered host workspace.

Web state also reuses its already resolved project identity for task and memory
reads. Standalone ledger commands still resolve their own identity. This removes
repeated isolated Git discovery without caching task or memory contents beyond
the existing state-cache lifetime. Device timings must be measured on the new
private artifact before claiming a performance improvement.

Project identity must also remain unavailable when an existing Git checkout
cannot be inspected. During backup recovery the web service could start before
its controller and select a different directory identity for the same checkout.
Verb now refuses that startup before selecting or creating a replacement work
store. Supervision can restart it after the controller is ready. Ordinary folders
remain supported without Git. The regression covers checkout and linked-worktree
markers, preserves registry/shared-memory bytes during a simulated outage, and
recovers the original ID when Git returns. Existing fallback records are retained
for review; this guard does not merge or delete any project data.
