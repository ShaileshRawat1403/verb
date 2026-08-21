package com.example.verb.session

import java.time.Instant

/**
 * The four states a [VerbSession] can be in. See `docs/VERB_SESSION_CONTRACT.md` for the full
 * contract this type implements -- this file should not drift from that document without it being
 * updated first.
 */
enum class VerbSessionState {
    /** A [ProcessBinding] exists and is running (or starting). */
    LIVE,

    /**
     * The process is gone and whether recovery is possible is not yet known. A waiting state, not a
     * resting one -- whoever owns the transition keeps re-checking [AgentAdapter.canResume] until it
     * resolves into [RECOVERABLE] or [ENDED].
     */
    INTERRUPTED,

    /** The process is gone, but [AgentAdapter.canResume] has positively established recovery works. */
    RECOVERABLE,

    /** Explicitly closed, or [AgentAdapter.canResume] has positively established recovery is impossible. */
    ENDED
}

/**
 * Opaque and host-specific: Android's proot/PTY handle, or on desktop, a native PTY handle. Shared
 * logic never looks inside this -- only at whether one is present.
 */
interface ProcessBinding

/**
 * A reference to an agent's own resumable state. [resumeIdentity] is opaque to [VerbSession]; only
 * the owning [AgentAdapter] interprets it (e.g. Claude's own session uuid for `--resume`). It can
 * outlive the [VerbSession] itself -- the transcript is on disk independent of any session
 * bookkeeping, per `docs/DURABLE_SESSION.md`.
 */
data class AgentRef(
    val agentType: String,
    val resumeIdentity: String? = null
)

/**
 * A session's identity, stable across the [ProcessBinding] it holds being created, dying, and being
 * replaced. `VerbSession` is not the PTY -- see `docs/VERB_SESSION_CONTRACT.md`.
 *
 * [projectId] and [runtime] describe the execution context that actually launched this session and
 * are fixed at creation. The UI's currently-selected project/runtime is a separate, independent
 * concept -- switching it must never mutate an existing session's recorded context.
 */
data class VerbSession(
    val id: String,
    val projectId: String?,
    val runtime: String?,
    val createdAt: Instant,
    val lastSeenAt: Instant,
    val state: VerbSessionState,
    val lastKnownCwd: String? = null,
    val lastObservedAt: Instant? = null,
    val process: ProcessBinding? = null,
    val agent: AgentRef? = null
)
