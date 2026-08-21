package com.example.verb.session

import org.junit.Assert.assertEquals
import org.junit.Test

/** One test per row of the mapping table in docs/VERB_SESSION_CONTRACT.md -- these should never
 *  need to change without that document changing first. */
class VerbSessionStateResolverTest {

    private val claude = AgentRef(agentType = "claude", resumeIdentity = "session-uuid")

    @Test
    fun `process present is always LIVE, regardless of agent`() {
        assertEquals(
            VerbSessionState.LIVE,
            VerbSessionStateResolver.resolve(processPresent = true, agent = null, resumeVerdict = null)
        )
        assertEquals(
            VerbSessionState.LIVE,
            VerbSessionStateResolver.resolve(processPresent = true, agent = claude, resumeVerdict = ResumeVerdict.NO)
        )
    }

    @Test
    fun `process absent with no agent at all is ENDED`() {
        assertEquals(
            VerbSessionState.ENDED,
            VerbSessionStateResolver.resolve(processPresent = false, agent = null, resumeVerdict = null)
        )
    }

    @Test
    fun `process absent, canResume YES is RECOVERABLE`() {
        assertEquals(
            VerbSessionState.RECOVERABLE,
            VerbSessionStateResolver.resolve(processPresent = false, agent = claude, resumeVerdict = ResumeVerdict.YES)
        )
    }

    @Test
    fun `process absent, canResume NO is ENDED`() {
        assertEquals(
            VerbSessionState.ENDED,
            VerbSessionStateResolver.resolve(processPresent = false, agent = claude, resumeVerdict = ResumeVerdict.NO)
        )
    }

    @Test
    fun `process absent, canResume UNKNOWN is INTERRUPTED, never RECOVERABLE`() {
        assertEquals(
            VerbSessionState.INTERRUPTED,
            VerbSessionStateResolver.resolve(processPresent = false, agent = claude, resumeVerdict = ResumeVerdict.UNKNOWN)
        )
    }

    @Test
    fun `process absent, agent present but verdict not yet checked (null) is INTERRUPTED`() {
        // Distinct from the UNKNOWN case at the type level, but the same honest answer: no evidence
        // yet either way, so this must not silently read as RECOVERABLE.
        assertEquals(
            VerbSessionState.INTERRUPTED,
            VerbSessionStateResolver.resolve(processPresent = false, agent = claude, resumeVerdict = null)
        )
    }
}
