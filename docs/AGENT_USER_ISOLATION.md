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

The execution supervisor, resource/storage admission and leases, native Verb
adapter, persistent-project authentication, candidate endpoint and full
resume/reconnect/stop/restart/rollback acceptance remain open. No production
promotion or automatic unrestricted-host fallback follows from this checkpoint.
