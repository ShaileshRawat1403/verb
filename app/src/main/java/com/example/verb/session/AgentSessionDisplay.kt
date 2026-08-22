package com.example.verb.session

/**
 * What the Agents screen shows for an agent's card once a [VerbSession] exists for it. A pure
 * function of [VerbSession.state] and nothing else -- requirement 1 of the UI slice this backs:
 * the screen must not independently infer session state from terminal output, process presence,
 * or agent card status.
 *
 * Shared by every agent, deliberately. The labels describe session lifecycle, which is identical
 * across agents by contract; if Codex ever needed different words for the same state, that would be
 * a sign the state means something different for Codex, which the contract does not allow.
 */
data class AgentSessionDisplay(
    val statusLabel: String,
    val detailLabel: String? = null,
    val showResume: Boolean = false,
    val showStartNew: Boolean = false
)

/** `null` when there is no tracked session yet -- the card falls back to its normal
 *  install/ready display, unchanged, until the user has launched that agent at least once. */
fun agentSessionDisplay(session: VerbSession?): AgentSessionDisplay? {
    val state = session?.state ?: return null
    return when (state) {
        VerbSessionState.LIVE -> AgentSessionDisplay(statusLabel = "Running")

        // "Start new" is offered here as well as under ENDED, deliberately. INTERRUPTED means Verb
        // does not know whether recovery is possible, and a card with no action at all leaves the
        // user stuck: unable to resume (nothing has proven that works) and unable to start over.
        // Offering only the action that is always honest -- a fresh session -- claims nothing about
        // the old one, whose evidence stays on disk either way.
        VerbSessionState.INTERRUPTED -> AgentSessionDisplay(
            statusLabel = "Session interrupted",
            detailLabel = "Checking recovery status…",
            showStartNew = true
        )

        VerbSessionState.RECOVERABLE -> AgentSessionDisplay(
            statusLabel = "Session recoverable",
            showResume = true
        )

        VerbSessionState.ENDED -> AgentSessionDisplay(
            statusLabel = "Session ended",
            showStartNew = true
        )
    }
}
