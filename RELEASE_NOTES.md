# Verb 0.1.0-beta.1 — developer preview

This is an evidence-gathering preview, not a claim that Verb is finished. It is intended for people
comfortable using a terminal who want Claude Code, Codex or OpenCode hosted inside a more
understandable and recoverable development environment.

## What is ready to test

- Native desktop PTY workspace with project/Git context, sessions, recovery, structural evidence,
  scrollback/search, mouse control and JSON CLI output.
- Android terminal-first workspace with a searchable Verb task sheet, real Termux-derived userland,
  Claude/Codex/OpenCode adapters and explicit Working World backup/restore.
- One four-state session contract on both hosts: `LIVE`, `INTERRUPTED`, `RECOVERABLE`, `ENDED`.
- Manual checksummed `.vcont` exchange of read-only structural evidence between hosts.
- Optional provider interpretation that can execute nothing and receives no terminal output,
  command text, file contents, transcript, credential or absolute path.

## Privacy correction on upgrade

Earlier Android development builds contained a local database and preference store for command
text, terminal output and assistant messages. Those categories violate Verb's durable-data boundary
and were not needed by the current product. On first launch, this version deletes that legacy local
database/preferences and does not recreate them.

Verb continues to persist only structural session/project records and allowlisted lifecycle events.
Working World archives remain explicit, user-triggered and encrypted because they may contain agent
credentials and the user's guest environment.

## Known limits

- The terminal-first Android workspace, v1 Working World import through the current v2 reader,
  Claude/Codex conversation recovery, and OpenCode launch have physical-device evidence.
- Claude's restored files produced the same conversation identity, but Claude itself reported that
  the saved login was no longer valid. Verb therefore reports only "Saved login found" and leaves
  authentication truth to the agent.
- OpenCode Android recovery is not physically proven; the restored v1 archive contained no
  OpenCode `VerbSession` record.
- Continuity moves evidence, not a running process or an agent transcript. A destination offers
  Resume only when its own local adapter positively confirms it. The complete physical
  Android→desktop→Android picker round-trip remains pending.
- Working World archives do not contain project source trees. Projects must be protected by Git or
  another independent backup.
- Desktop is installed from source; prebuilt desktop binaries and Windows TUI support are pending.
- Android uses the established `com.aistudio.verb.app` application ID but retains an internal
  `com.example` Kotlin namespace. Renaming it is upgrade-sensitive and intentionally deferred.

See `docs/RELEASE_CHECKLIST.md` and `RETURN_HANDOFF.md` for exact verification and remaining gates.
