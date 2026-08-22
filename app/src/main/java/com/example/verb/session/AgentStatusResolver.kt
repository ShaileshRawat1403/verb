package com.example.verb.session

import com.example.verb.terminal.RuntimeProfileReport

/**
 * The one place a displayed agent status is decided.
 *
 * Verb had four sources of truth about an agent and four places that turned them into words:
 * a readiness probe said "Ready", a surviving PTY said "Running", a persisted record said `LIVE`,
 * and the existence of a credentials file said "Signed in". Each surface combined them slightly
 * differently, so every wrong label was a new bug in a new place -- most recently two agents both
 * reporting "Running" while neither process existed.
 *
 * Now the evidence goes in here and a status comes out. Screens render [AgentStatus]; they do not
 * assemble one. If a new piece of evidence appears, it is added to [Evidence] and weighed once.
 */
object AgentStatusResolver {

    /** Everything Verb knows about one agent at one moment. Nothing here is inferred. */
    data class Evidence(
        /** The readiness probe: is the command installed and executable? */
        val report: RuntimeProfileReport,
        /** The tracked session for this agent, when Verb has one. */
        val session: VerbSession?,
        /** Whether an agent-owned credential file exists. Never read, only counted. */
        val signedIn: Boolean?,
        /** True while Verb is installing this agent. */
        val installing: Boolean,
        /** True while another agent's install is running, which blocks this one's button. */
        val otherInstallRunning: Boolean
    )

    /** What a card shows. A screen may style this; it may not decide it. */
    data class AgentStatus(
        val label: String,
        val detail: String?,
        val action: Action,
        /** The evidence behind [label], in a few words, for diagnostics and for tests. */
        val because: String
    )

    enum class Action { NONE, INSTALL, OPEN, RESUME, START_NEW }

    fun resolve(evidence: Evidence): AgentStatus {
        val profile = evidence.report.profile

        // Known-impossible outranks everything: an agent that cannot run here must never show a
        // button, whatever a probe says about it.
        profile.unavailableReason?.let { reason ->
            return AgentStatus(
                label = "Unavailable",
                detail = reason,
                action = Action.NONE,
                because = "the catalog records why this agent cannot run here"
            )
        }

        if (evidence.installing) {
            return AgentStatus(
                label = "Installing",
                detail = null,
                action = Action.NONE,
                because = "Verb is installing it now"
            )
        }

        // A tracked session is the strongest evidence Verb has about an agent, because it was
        // observed rather than probed.
        evidence.session?.let { session ->
            return when (session.state) {
                VerbSessionState.LIVE -> AgentStatus(
                    label = "Running",
                    detail = null,
                    action = Action.NONE,
                    because = "Verb is holding this agent's process binding"
                )

                VerbSessionState.RECOVERABLE -> AgentStatus(
                    label = "Session recoverable",
                    detail = null,
                    action = Action.RESUME,
                    because = "the agent's own evidence says the conversation can be resumed"
                )

                VerbSessionState.INTERRUPTED -> AgentStatus(
                    label = "Session interrupted",
                    detail = "Checking recovery status…",
                    action = Action.START_NEW,
                    because = "the process is gone and recovery is not yet established either way"
                )

                VerbSessionState.ENDED -> AgentStatus(
                    label = "Session ended",
                    detail = null,
                    action = Action.START_NEW,
                    because = "the agent's own evidence says there is nothing to recover"
                )
            }
        }

        if (evidence.report.isUnsatisfiable) {
            return AgentStatus(
                label = "Unavailable",
                detail = "Cannot run on this device. No install will resolve this.",
                action = Action.NONE,
                because = "an installed binary is incompatible with this device"
            )
        }

        if (evidence.report.isReady) {
            return AgentStatus(
                label = "Ready",
                detail = signedInDetail(evidence.signedIn),
                action = Action.OPEN,
                because = "the probe resolved and ran the command"
            )
        }

        return AgentStatus(
            label = "Not installed",
            detail = null,
            action = if (evidence.otherInstallRunning) Action.NONE else Action.INSTALL,
            because = "the probe could not resolve the command"
        )
    }

    /**
     * Sign-in is reported from the existence of a credential file, which is weaker evidence than it
     * looks: it says an agent once wrote credentials, not that they are still valid. So it is a
     * detail line and never a status of its own.
     */
    private fun signedInDetail(signedIn: Boolean?): String? = when (signedIn) {
        true -> "Signed in"
        false -> "Not signed in — run it once to sign in"
        null -> null
    }
}
