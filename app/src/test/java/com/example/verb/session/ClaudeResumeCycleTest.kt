package com.example.verb.session

import com.example.verb.terminal.FakeTerminalRuntimeAdapter
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.time.Instant

/**
 * The complete transition the whole slice exists to prove:
 *
 * ```
 * LIVE -> (process dies) -> INTERRUPTED -> (ClaudeAdapter finds the transcript) -> RECOVERABLE
 *      -> (user chooses Resume) -> LIVE, same VerbSession.id, new process
 * ```
 *
 * Every step uses the real [VerbSessionStateResolver] and real [ClaudeAgentAdapter], not stubs --
 * this is the first test in the codebase where the `VerbSession` abstraction produces an outcome a
 * user would actually see, rather than only proving its own internal bookkeeping.
 */
class ClaudeResumeCycleTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `LIVE to INTERRUPTED to RECOVERABLE to LIVE, same id throughout`() = runTest {
        val filesDir = temporaryFolder.newFolder("files")
        val project = File(filesDir, "projects/alpha").apply { mkdirs() }
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        val adapter = ClaudeAgentAdapter(filesDir, project, fake, resumeSettleMs = 200, pollIntervalMs = 20)
        val agent = AgentRef(agentType = "claude", resumeIdentity = "session-abc")

        var session = VerbSession(
            id = "session-1",
            projectId = "project-1",
            runtime = "claude",
            createdAt = Instant.EPOCH,
            lastSeenAt = Instant.EPOCH,
            state = VerbSessionState.LIVE,
            agent = agent,
            process = object : ProcessBinding {}
        )
        assertEquals(VerbSessionState.LIVE, session.state)

        // The process ends. Nothing has checked the transcript yet, and there is no transcript
        // directory on disk at all -- ClaudeAgentAdapter cannot determine anything.
        val verdictBeforeTranscriptExists = adapter.canResume(agent)
        session = session.copy(
            process = null,
            state = VerbSessionStateResolver.resolve(processPresent = false, agent, verdictBeforeTranscriptExists)
        )
        assertEquals(
            "no transcript directory at all is UNKNOWN, which resolves to INTERRUPTED, not RECOVERABLE",
            VerbSessionState.INTERRUPTED,
            session.state
        )

        // Claude's transcript shows up on disk -- the real fact that makes resume possible.
        val transcriptDir = File(filesDir, "home/.claude/projects/${project.absolutePath.replace('/', '-')}")
        transcriptDir.mkdirs()
        File(transcriptDir, "session-abc.jsonl").createNewFile()

        // Whoever owns the INTERRUPTED -> * transition re-checks, per the contract: INTERRUPTED is
        // a waiting state, not a resting one.
        val verdictAfterTranscriptExists = adapter.canResume(agent)
        session = session.copy(
            state = VerbSessionStateResolver.resolve(processPresent = false, agent, verdictAfterTranscriptExists)
        )
        assertEquals(VerbSessionState.RECOVERABLE, session.state)

        // The user chooses Resume.
        session = VerbSessionResumer.resume(session, adapter)

        assertEquals(VerbSessionState.LIVE, session.state)
        assertEquals("session-1", session.id)
    }
}
