package com.example.verb.session

import java.time.Instant

/**
 * The one place a [VerbSessionState.RECOVERABLE] session actually becomes
 * [VerbSessionState.LIVE] again. Deliberately separate from [VerbSessionStateResolver]: the
 * resolver answers "given these facts, what state is this"; this answers "the user asked to
 * resume -- what happens now." Different question, same discipline about never claiming LIVE
 * without evidence.
 */
object VerbSessionResumer {

    /**
     * Resumes [session] via [adapter] if [session] is [VerbSessionState.RECOVERABLE] and has an
     * [AgentRef]. Any other input is returned unchanged -- there is nothing to resume.
     *
     * On success: same `id`, `state` becomes `LIVE`, `process` is the new binding. On failure
     * ([AgentAdapter.resume] returns `null`): the session is returned exactly as it was. There is
     * no partial-success state here -- either the resume produced a live process, or nothing about
     * the session changes.
     */
    suspend fun resume(session: VerbSession, adapter: AgentAdapter): VerbSession {
        val agent = session.agent ?: return session
        if (session.state != VerbSessionState.RECOVERABLE) return session

        val binding = adapter.resume(agent)
        return if (binding != null) {
            session.copy(
                state = VerbSessionState.LIVE,
                process = binding,
                lastSeenAt = Instant.now()
            )
        } else {
            session
        }
    }
}
