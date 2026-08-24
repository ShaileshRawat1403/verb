# Security Policy

Verb hosts authenticated development agents and terminal runtimes. Treat reports involving command
execution, archive import, resume identities, provider credentials, terminal input ownership, or
durable-data privacy as security-sensitive.

## Supported versions

Until the first stable release, only the latest source revision and latest published prerelease are
supported. Security fixes may require upgrading rather than backporting.

## Reporting a vulnerability

Use the repository's private security-advisory form:

<https://github.com/ShaileshRawat1403/verb/security/advisories/new>

If private reporting is unavailable, open an issue asking the maintainer to establish a private
channel. Do not include exploit details, credentials, terminal output, prompts, transcripts, agent
state, archive contents, or private project paths in a public issue.

Include only the minimum synthetic reproduction needed to establish the problem. Planted markers
are preferred to real secrets. The maintainer will acknowledge the report, establish impact and an
expected update cadence, and coordinate disclosure after a correction is available.

## Security and privacy boundary

Verb may durably retain structural session identity, lifecycle events, timestamps, exit status and
bounded project context. It must never durably retain raw PTY input/output, command text, prompts,
agent transcripts, credentials, PIDs, process handles, or a persisted `processPresent` claim.

A persisted `LIVE` record is history, not current truth. A host may call a session `RECOVERABLE`
only after local, positive agent evidence. Imported continuity records are read-only evidence and
never grant process or resume authority.

See `docs/ARCHITECTURE.md`, `docs/VERB_SESSION_CONTRACT.md`, and
`docs/VERB_CONTINUITY_ENVELOPE.md` for the enforceable boundaries.
