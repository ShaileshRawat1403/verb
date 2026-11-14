# Hosted Verb: one micro-VM per workspace

Status: proposal, 2026-10-10. Owner chose option C of `docs/SECURITY_MODEL.md` ("always go for
scalable architecture"). Nothing here is built yet; this document is for approval.

## Goals

1. **A real boundary per workspace.** An agent can do anything inside its workspace and nothing
   outside it: not another user's workspace, not the host, not the control plane.
2. **Scale out horizontally.** More users means more hosts, not a bigger server. No component
   holds state that stops it being replaced.
3. **Persistent, resumable workspaces.** Close the tab, come back tomorrow: the project, terminal
   history and agent logins are there. Idle workspaces cost storage, not compute.
4. **The same Verb everywhere.** Hosted, self-hosted on one server, or on a personal device (Node 1)
   run the same workspace software; hosting adds the layers around it.

## Shape

```
 Browser ──TLS──▶ Gateway ──mTLS──▶ Workspace host ──vsock/tap──▶ micro-VM: Verb workspace agent
                    │                     │                         (today's `verb web`, agents,
                    ▼                     ▼                          the project on its own disk)
              Control plane ◀──── host agent (starts, stops,
          (accounts, projects,        snapshots VMs; reports
           tokens, scheduling)        capacity)
                    │
              Postgres · object storage (snapshots, volumes)
```

| Component | Does | State |
| --- | --- | --- |
| **Gateway** | Terminates TLS, authenticates the request with the control plane (session or access token), routes HTTP and WebSocket traffic to the right VM | None (stateless, horizontally scaled) |
| **Control plane** | Users, organisations, projects, access tokens, audit of control actions, scheduling (which host runs which workspace), quotas | Postgres |
| **Host agent** | On each workspace host: boots and stops Firecracker VMs, attaches volumes, snapshots, enforces egress policy, reports capacity | Local, reconstructible |
| **Workspace VM** | One per workspace: a minimal Linux guest running the Verb workspace agent, the agent CLIs and toolchains | Its volume |
| **Storage** | Workspace volumes (block devices), VM snapshots, base images | Object storage |

Firecracker is chosen for the VMs: it boots in about a second, restores from snapshot faster, keeps
memory overhead per VM small, and is what large multi-tenant serverless platforms use for exactly
this boundary. It needs KVM.

**Free-only constraint (owner, 2026-10-10): no paid hosting.** Free cloud VMs generally do not
expose KVM, so the host agent talks to an **isolation driver** with two implementations behind one
interface (start, stop, snapshot, attach volume, egress policy):

| Driver | Needs | Boundary | Where it runs for free |
| --- | --- | --- | --- |
| **Firecracker** | Linux with KVM | Hardware virtualisation (separate kernel) | A Linux machine the owner already has (an old PC or laptop) |
| **gVisor** (`runsc`, open source) | Any Linux; no KVM (its systrap platform) | A user-space kernel between the workspace and the host kernel | Free-tier cloud VMs (e.g. Oracle Cloud Always Free, Arm, up to 4 cores and 24 GB) |

The architecture does not change with the driver; only the strength of the boundary does. A free
deployment can start on gVisor and move hosts to Firecracker when KVM hardware is available,
workspace by workspace.

## What changes in Verb itself

- **Split `verb web` into the workspace agent and the control plane.** The workspace agent keeps
  everything it does today (terminals, specs, agents, Git, the UI) but no longer authenticates
  people: it trusts only the gateway (mutual TLS, plus a signed header naming the user and their
  scope). Owner and access tokens move to the control plane, unchanged in meaning.
- **The UI gains a workspace list** (create, open, stop, delete) in front of today's project view.
- **Agent logins live in the workspace.** Each agent's own login (Claude, Codex, Gemini) is done
  once inside the VM through its device-code flow and stays on the workspace volume. Optional:
  bring-your-own API keys held by a secrets service and injected at boot, never logged.
- **Self-hosting stays first-class**: the same pieces run on one server (gateway, control plane and
  one host agent together), with SQLite instead of Postgres.

## Workspace lifecycle

1. **Create**: the scheduler picks a host with capacity; the host agent restores the base image
   snapshot and attaches a new volume; the project is cloned into it.
2. **Open**: the gateway routes the browser to the VM's workspace agent.
3. **Idle** (no browser and no running agent for a configurable time): snapshot memory and disk,
   stop the VM, free the host.
4. **Resume**: restore the snapshot on any host with capacity; terminals come back with their
   history (live processes come back too when restored from a memory snapshot).
5. **Delete**: volume and snapshots removed; audit entry kept.

## Security boundaries

- **VM per workspace**: separate kernel; a compromised agent stays inside it.
- **No credentials of the platform inside a VM**: the workspace agent holds only its own identity
  for the gateway; it cannot reach the control plane's database or other VMs.
- **Egress policy per workspace**: default allow to the internet (agents need their APIs and package
  registries), deny to the platform's internal networks and cloud metadata endpoints.
- **Audit**: control actions (create, open, delete, token issue) in the control plane; work actions
  (specs, stages, commits) in the project as today.

## Scaling

- Gateway and control plane are stateless processes behind a load balancer.
- Workspace hosts form a pool; the scheduler places by free memory and CPU; the pool grows and
  shrinks with demand.
- A running workspace is pinned to one host; a stopped one can resume anywhere, because its state is
  in object storage.
- Idle stopping keeps compute proportional to active users, not total users.

## Phases

| Phase | Delivers | Proves |
| --- | --- | --- |
| P0 | This design approved; hosting provider chosen | Direction |
| P1 | Workspace agent split from `verb web` (gateway-trusting mode); one Firecracker VM on one KVM host started by hand | Verb runs unchanged inside a micro-VM; agents log in and work there |
| P2 | Control plane MVP: sign-in, projects, create/open/stop workspaces on one host | The product loop end to end |
| P3 | Gateway and scheduler across several hosts; egress policy | Horizontal scale and isolation |
| P4 | Snapshots, idle stop and resume; quotas | Cost proportional to use |
| P5 | Teams, billing, usage limits | Commercial readiness |

## Decisions needed

Constraint: free only. Free components throughout: Firecracker or gVisor, Postgres or SQLite,
MinIO or the host's disk for object storage, Caddy for TLS, Cloudflare Tunnel (free) or Tailscale
(free tier) for reaching hosts behind home networks.

1. **First workspace host**: a Linux machine the owner already has (Firecracker, if its CPU has
   virtualisation enabled), or a free-tier cloud VM (gVisor).
2. **Sign-in**: GitHub or Google sign-in through OIDC (free), or our own accounts. OIDC is
   recommended: no passwords to store.
3. **Free-tier limits accepted**: one or two hosts, a handful of concurrent workspaces; the design
   scales out unchanged when hosts are added.

Until hosted Verb exists, personal nodes like Node 1 keep the single-user model described in
`docs/SECURITY_MODEL.md`.
