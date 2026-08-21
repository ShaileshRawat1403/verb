package com.example.verb.session

/**
 * Whether an agent's own state can be resumed. A third value, not a boolean: [UNKNOWN] is the
 * absence of an answer, not evidence either way, and must not be collapsed into [YES] or [NO] --
 * that is what [VerbSessionState.INTERRUPTED] exists to represent honestly.
 */
enum class ResumeVerdict { YES, NO, UNKNOWN }

/**
 * Resumability is agent-specific knowledge -- checking Claude's transcripts looks nothing like
 * checking Codex's rollout files -- so [VerbSession] never inspects an [AgentRef] itself. It only
 * ever sees the verdict an [AgentAdapter] returns.
 *
 * An adapter is the *only* place agent-specific runtime truth belongs: how the agent is resumed,
 * what on-disk evidence proves a recoverable conversation exists, and what that conversation's
 * stable identity is. Session lifecycle itself is shared -- see [AgentSessionCoordinator].
 */
interface AgentAdapter {
    fun canResume(agent: AgentRef): ResumeVerdict

    /**
     * The agent's own stable conversation id, when this host exposes one and the agent does not
     * already carry it. Never a PID or a file handle: those do not survive process death, which is
     * exactly the case this identity exists for. `null` means "no id discoverable", which is not a
     * claim that recovery is impossible -- [canResume] answers that separately.
     */
    fun resumeIdentity(agent: AgentRef): String? = null

    /**
     * Attempts to resume [agent]. `null` means the attempt failed -- there is no other failure
     * signal, deliberately: a caller that only ever branches on null-vs-non-null cannot
     * accidentally treat a failure as success, which a thrown exception or a boolean flag alongside
     * a possibly-stale binding both make easy to get wrong.
     */
    suspend fun resume(agent: AgentRef): ProcessBinding?
}
