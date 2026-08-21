package com.example.verb.session

/**
 * Whether an agent's own state can be resumed. A third value, not a boolean: [UNKNOWN] is the
 * absence of an answer, not evidence either way, and must not be collapsed into [YES] or [NO] --
 * that is what [VerbSessionState.INTERRUPTED] exists to represent honestly.
 */
enum class ResumeVerdict { YES, NO, UNKNOWN }

/**
 * Resumability is agent-specific knowledge -- checking Claude's transcripts looks nothing like
 * checking Codex's -- so [VerbSession] never inspects an [AgentRef] itself. It only ever sees the
 * verdict an [AgentAdapter] returns.
 */
interface AgentAdapter {
    fun canResume(agent: AgentRef): ResumeVerdict

    /**
     * Attempts to resume [agent]. `null` means the attempt failed -- there is no other failure
     * signal, deliberately: a caller that only ever branches on null-vs-non-null cannot
     * accidentally treat a failure as success, which a thrown exception or a boolean flag alongside
     * a possibly-stale binding both make easy to get wrong.
     */
    suspend fun resume(agent: AgentRef): ProcessBinding?
}
