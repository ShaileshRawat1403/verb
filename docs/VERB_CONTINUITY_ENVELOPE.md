# Verb continuity envelope

Status: **v1 contract and beta implementation, 2026-08-24**

Verb continuity moves structural knowledge between user-owned hosts. It does not move a process,
PTY, terminal transcript, agent transcript, credential, or execution authority.

The rule that preserves the four-state contract is:

> **State is never transported as current truth. Evidence is transported. State is computed by the
> host answering the question.**

An origin may record that it wrote `LIVE` at a time. A destination must display that as dated
history, never as proof that a process is live there. An imported record is read-only and lives in a
separate namespace. It never occupies a local session slot and never enables Resume by itself.

## Beta transport

The beta uses an explicit, manual `.vcont` file. There is no background sync, Verb cloud, account,
LAN listener, discovery service, or daemon. A file can be carried by USB, local sharing, or storage
the user chooses. Import previews by default and changes state only with an explicit `--apply` or
the equivalent Android confirmation.

The file is UTF-8 JSON Lines:

1. one header record;
2. one origin record;
3. zero or more project, session, and event records.

The header contains `envelopeVersion: 1`, the literal `kind: "verb.continuity"`, and
`payloadSha256`, computed over every byte after the header newline. Exact version match is required.
Unknown keys, unknown record types, malformed identifiers, traversal-like relative paths, checksum
mismatch, and oversized input are rejected rather than repaired.

This beta envelope is plaintext because it is evidence-only and must be inspectable. It is written
with owner-only permissions where the host supports them. It can reveal project labels, agent names,
and work times; users should still treat it as private. The credential-bearing `.vbak` format is a
different artifact and must never be used as a continuity file.

## Provenance and identity

The origin record contains:

- `hostId`: opaque random 128-bit install identity; never hardware-derived;
- `hostKind`: `android` or `desktop`;
- `verbVersion`;
- `exportedAt`: origin-clock UTC time.

The host ID is a grouping label, not authentication. Imported evidence is labelled as recorded on
another host and has no authority merely because its label matches an earlier import.

A session ID travels unchanged. Its multi-host key is `(origin.hostId, sessionId)`. It is never
rewritten into a local ID.

Project identity is derived at export time:

1. `git:<normalized remote host/path>` when a credential-free Git remote is available;
2. `unresolved` otherwise.

Paths are never identity. The envelope carries a basename `label` and may carry a normalized,
repository-relative `cwdRelative`; it never carries an absolute path. An unresolved project imports
as evidence, matches nothing automatically, and offers no action.

## Session records

A session record contains only:

- `sessionId`, `projectKey`, `runtimeId`;
- optional validated `agentType` and `resumeIdentityRef`;
- `createdAt`, `lastSeenAt`, optional `lastObservedAt`;
- optional normalized `cwdRelative`;
- `recordedState` and `recordedStateAt`.

`recordedState` accepts the existing four words, including `LIVE`, only as history. Import never
writes it into the local durable session store. A destination with a matched local project may run
its local `AgentAdapter.canResume()`; only local `YES` may produce a local `RECOVERABLE` capability.

## Event records

Event identity is `(origin.hostId, sessionId, seq)`. Sequence is assigned at the origin, per session,
and strictly increases. Within a session, sequence defines ordering; clocks from different hosts are
never interleaved to invent causality. Re-importing the same checksum is a no-op. The same event key
with different content is a conflict and must never silently overwrite earlier evidence.

Allowed event fields are the closed event vocabulary from `VERB_SESSION_SCHEMA.md`, plus `seq`,
`recordedAt`, optional `exitCode`, opaque `commandId`, normalized `cwdRelative`, state, tool **name**,
and `source` (`shell`, `agentRecord`, or `verb`). Tool arguments and results are prohibited.

## Complete prohibited categories

The writer has no field capable of containing raw PTY input/output, terminal or agent transcripts,
prompts, responses, command text, argv, environment variables, credentials, tokens, cookies,
keystores, tool arguments/results, file contents, diffs, patches, PID, process/PTY handles,
`processPresent`, absolute paths, device names/serials, hardware identifiers, hostnames, usernames,
screenshots, shell history, or database rows.

Encryption, signed pairing, automatic transport, merge/conflict resolution, and cross-host process
or transcript resume are not part of v1.
