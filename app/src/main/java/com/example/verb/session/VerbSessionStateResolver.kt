package com.example.verb.session

/**
 * Computes [VerbSessionState] from the two facts that actually determine it: whether a
 * [ProcessBinding] currently exists, and -- when it doesn't -- what the owning [AgentAdapter] says
 * about resumability. This is the mapping table from `docs/VERB_SESSION_CONTRACT.md`, made into
 * code so it can't drift from the doc without a test failing.
 *
 * Deliberately free of any Android/desktop dependency: this is the one piece of the contract that
 * is pure logic, not a platform integration, and should stay that way so it can be shared verbatim
 * later rather than reimplemented per host.
 */
object VerbSessionStateResolver {

    fun resolve(processPresent: Boolean, agent: AgentRef?, resumeVerdict: ResumeVerdict?): VerbSessionState {
        if (processPresent) return VerbSessionState.LIVE

        if (agent == null) return VerbSessionState.ENDED

        return when (resumeVerdict) {
            ResumeVerdict.YES -> VerbSessionState.RECOVERABLE
            ResumeVerdict.NO -> VerbSessionState.ENDED
            ResumeVerdict.UNKNOWN, null -> VerbSessionState.INTERRUPTED
        }
    }
}
