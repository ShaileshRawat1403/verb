package com.example.verb.session

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test
import java.time.Instant

/**
 * A stub, not [ClaudeAgentAdapter]: these tests are about whether [VerbSessionResumer] ever
 * mutates state incorrectly, not about any one agent's detection logic (that's
 * `ClaudeAgentAdapterTest`).
 */
private class StubAgentAdapter(private val resumeResult: ProcessBinding?) : AgentAdapter {
    override fun canResume(agent: AgentRef): ResumeVerdict = ResumeVerdict.UNKNOWN
    override suspend fun resume(agent: AgentRef): ProcessBinding? = resumeResult
}

private object FakeBinding : ProcessBinding

class VerbSessionResumerTest {

    private val agent = AgentRef(agentType = "claude", resumeIdentity = "session-abc")

    private fun recoverableSession(): VerbSession = VerbSession(
        id = "session-1",
        projectId = "project-1",
        runtime = "claude",
        createdAt = Instant.EPOCH,
        lastSeenAt = Instant.EPOCH,
        state = VerbSessionState.RECOVERABLE,
        agent = agent
    )

    @Test
    fun `resume succeeds - same id, state becomes LIVE, process is the new binding`() = runTest {
        val session = recoverableSession()

        val result = VerbSessionResumer.resume(session, StubAgentAdapter(FakeBinding))

        assertEquals(session.id, result.id)
        assertEquals(VerbSessionState.LIVE, result.state)
        assertSame(FakeBinding, result.process)
    }

    @Test
    fun `resume fails - the session is returned exactly as it was, never LIVE`() = runTest {
        val session = recoverableSession()

        val result = VerbSessionResumer.resume(session, StubAgentAdapter(resumeResult = null))

        assertEquals(session, result)
        assert(result.state != VerbSessionState.LIVE) { "a failed resume must never produce LIVE" }
    }

    @Test
    fun `resuming a session that is not RECOVERABLE is a no-op, even if resume would succeed`() = runTest {
        val interrupted = recoverableSession().copy(state = VerbSessionState.INTERRUPTED)

        val result = VerbSessionResumer.resume(interrupted, StubAgentAdapter(FakeBinding))

        assertEquals(
            "INTERRUPTED must resolve to RECOVERABLE first; resume is not a shortcut around that",
            interrupted,
            result
        )
    }

    @Test
    fun `resuming a session with no agent is a no-op`() = runTest {
        val noAgent = recoverableSession().copy(agent = null)

        val result = VerbSessionResumer.resume(noAgent, StubAgentAdapter(FakeBinding))

        assertEquals(noAgent, result)
    }

    @Test
    fun `an ENDED session is never resumed`() = runTest {
        val ended = recoverableSession().copy(state = VerbSessionState.ENDED)

        val result = VerbSessionResumer.resume(ended, StubAgentAdapter(FakeBinding))

        assertNull(result.process)
        assertEquals(VerbSessionState.ENDED, result.state)
    }
}
