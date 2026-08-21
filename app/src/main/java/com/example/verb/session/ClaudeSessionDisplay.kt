package com.example.verb.session

/**
 * What the Agents screen shows for Claude's card once a [VerbSession] exists for it. A pure
 * function of [VerbSession.state] and nothing else -- requirement 1 of the UI slice this backs:
 * the screen must not independently infer session state from terminal output, process presence,
 * or agent card status.
 */
data class ClaudeSessionDisplay(
    val statusLabel: String,
    val detailLabel: String? = null,
    val showResume: Boolean = false,
    val showStartNew: Boolean = false
)

/** `null` when there is no tracked session yet -- the card falls back to its normal
 *  install/ready display, unchanged, until the user has launched Claude at least once. */
fun claudeSessionDisplay(session: VerbSession?): ClaudeSessionDisplay? {
    val state = session?.state ?: return null
    return when (state) {
        VerbSessionState.LIVE -> ClaudeSessionDisplay(statusLabel = "Running")

        VerbSessionState.INTERRUPTED -> ClaudeSessionDisplay(
            statusLabel = "Session interrupted",
            detailLabel = "Checking recovery status…"
        )

        VerbSessionState.RECOVERABLE -> ClaudeSessionDisplay(
            statusLabel = "Session recoverable",
            showResume = true
        )

        VerbSessionState.ENDED -> ClaudeSessionDisplay(
            statusLabel = "Session ended",
            showStartNew = true
        )
    }
}
