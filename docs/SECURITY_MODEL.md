# Verb security model

Status: 2026-10-10. What Verb protects, from whom, what is in place, and the containment decision
that is still open.

## What Verb is, from a security view

`verb web` is a local HTTP and WebSocket server that hosts real terminals (PTYs) and AI agent CLIs
in a project folder, and shows them in a browser. Whoever can drive Verb can run commands as the
operating-system user Verb runs as. Everything below follows from that.

## Assets

1. **The machine account Verb runs as**: its files, credentials (SSH keys, cloud credentials, agent
   logins), and anything reachable from it.
2. **The owner token**, which can do everything Verb can.
3. **Project content**: code, specs, audit trails, agent conversations.
4. **The audit trail's honesty**: who did what, recorded truthfully.

## Actors

| Actor | Trusted with | Reaches Verb through |
| --- | --- | --- |
| Owner | everything | the URL `verb web` prints, or Cloudflare Access when published |
| An agent or script the owner authorises | what its access token allows | an access token |
| An AI agent running *inside* a Verb terminal | the owner's shell (that is what a terminal is) | the PTY |
| Content the agent reads (repos, web pages, terminal output) | nothing; it may try to give instructions | the agent |
| Anyone else on the network | nothing | must not reach Verb at all |

## In place

- **Local by default.** The server binds to `127.0.0.1`; it checks `Host` and `Origin` headers and
  rejects cross-origin requests. Publishing it (Node 1) goes through Cloudflare Access with a JWT
  check, an allowed email, and session cookies that are `HttpOnly; Secure; SameSite=Strict`.
- **The owner token** is random (256 bits) unless set by `VERB_TOKEN`, compared in constant time,
  and never passed to hosted terminals.
- **Access tokens** (`verb token`): named, scoped (`read` or `drive`), expiring (at most 30 days),
  revocable at once, stored as hashes in a `0600` file, recorded by name in audit trails, and shown
  in a banner in the browser. No access token reaches owner-only settings or can mint another
  token. See `desktop/README.md`.
- **Hosted terminals do not inherit the launcher's secrets**: `VERB_TOKEN` and the session markers
  and messaging token of an agent that launched Verb are removed from every child's environment.
- **Privacy defaults.** The agent stream (reading agent conversations) and the observer are off
  until the owner turns them on per project; the observer reports a possible secret by kind, never
  by value. The file tree never lists or opens Git-ignored files.
- **No permission bypass.** Verb never starts an agent with flags that skip its permission prompts
  and never writes allow-rules into an agent's settings. When an agent asks, Verb shows "Needs you".
- **Honest audit.** Stage moves, proofs (with evidence), commits, agent starts and handoffs are
  appended to the spec file, which goes into Git history with the spec's commits; moving to Ship
  warns about uncommitted work and an unmerged branch.

## Residual risks (known, not yet closed)

1. **No containment of terminals.** An agent in a Verb terminal can do anything the Verb account
   can: read SSH keys, push with the owner's Git credentials, read a token file on disk (Node 1
   keeps the owner token in a file, readable by its terminals). A `drive` access token therefore
   buys identity, expiry and revocation, not a boundary.
2. **Prompt injection** through content agents read. Verb cannot stop an agent being persuaded;
   it can only keep the agent's own permission prompts in front of the owner and limit the blast
   radius (see containment).
3. **Shared checkout.** All sessions of a project share one working tree, so one agent can
   overwrite another's work, and Verb cannot attribute an edit to a session.
4. **Linux `/proc/<pid>/environ`** shows the environment Verb started with, so `VERB_TOKEN` set in
   the environment is readable by processes of the same user. Prefer letting Verb generate the
   token, or a token file the terminals cannot read (which needs containment).

## The open decision: containment

Containment is what turns "an agent can do what you can" into "an agent can do what this session
needs". The obvious shortcut, wrapping each terminal in the macOS sandbox (`sandbox-exec`) or
similar, conflicts with the agents' own sandboxes: Codex and Claude Code sandbox their commands the
same way, and a sandbox cannot be applied inside another. Wrapping would trade the agents' safety
for Verb's. The real options:

| Option | Boundary | Works on | Cost |
| --- | --- | --- | --- |
| **A. A separate OS user for agent sessions** | Unix permissions: the owner's home, keys and Verb's tokens are unreadable to agents | macOS, Linux (not Android/proot, which is single-user) | One setup step (create the user, a narrow helper to start PTYs as it); agents need their own logins under that user |
| **B. A container per project** (Docker/Podman, dev-container style) | Namespaces; only the project is mounted | Linux natively; macOS through a VM (Docker Desktop, Colima) | A container runtime; images per toolchain; slower start |
| **C. A VM per workspace** (hosted) | Hardware virtualisation (e.g. Firecracker micro-VMs) | Hosted service | Infrastructure; the standard for a commercial hosted product |

Recommendation:

- **Self-hosted, now:** A as an opt-in "agent user" mode, then the default once it has been used
  for a while. It keeps the agents' own sandboxes working and closes the worst risk (agents reading
  the owner's credentials and Verb's tokens).
- **Commercial hosted:** C, one micro-VM per workspace, with B for people who self-host on a server.
- **Node 1 (Android, single user):** accept the single-user model; reach it only through
  Cloudflare Access and access tokens, keep throwaway projects there, and treat it as a personal
  device rather than a multi-tenant host.

Until a containment option ships: give `drive` tokens only to agents you would let use your shell,
keep real credentials off machines that host Verb for others, and keep the owner in the loop for
pushes, releases and deletes.
