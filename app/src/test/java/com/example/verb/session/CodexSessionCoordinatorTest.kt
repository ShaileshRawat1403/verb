package com.example.verb.session

import com.example.verb.project.VerbProject
import com.example.verb.terminal.FakeTerminalRuntimeAdapter
import com.example.verb.terminal.ShellIntegrationEvent
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.time.Instant

/**
 * Codex driving the *same* [AgentSessionCoordinator] Claude drives. These tests exist to prove
 * exactly that: session identity, recovery and resume behave identically for a second agent because
 * the lifecycle is shared, and only [CodexAgentAdapter] differs.
 */
class CodexSessionCoordinatorTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun setUp(): Triple<File, VerbProject, FakeTerminalRuntimeAdapter> {
        val filesDir = temporaryFolder.newFolder("files")
        val projectDir = File(filesDir, "projects/alpha").apply { mkdirs() }
        val project = VerbProject(id = "alpha", directory = projectDir)
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        return Triple(filesDir, project, fake)
    }

    private fun writeRollout(
        filesDir: File,
        project: VerbProject,
        sessionId: String,
        userTurns: Boolean = true
    ) {
        val directory = File(filesDir, "home/.codex/sessions/2026/08/21").apply { mkdirs() }
        val startedAt = "2026-08-21T10:00:00.000Z"
        val header = """{"timestamp":"$startedAt","type":"session_meta","payload":""" +
            """{"id":"$sessionId","timestamp":"$startedAt","cwd":"${project.directory.absolutePath}"}}"""
        val turn = """{"timestamp":"$startedAt","type":"response_item","payload":""" +
            """{"type":"message","role":"user","content":[{"type":"input_text","text":"hi"}]}}"""
        File(directory, "rollout-$startedAt-$sessionId.jsonl")
            .writeText(if (userTurns) "$header\n$turn\n" else "$header\n")
    }

    private fun exitCodex(fake: FakeTerminalRuntimeAdapter, exitCode: Int = 0) {
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(exitCode))
    }

    @Test
    fun `onLaunched sets LIVE with a fresh id and a process bound`() = runTest {
        val (filesDir, project, fake) = setUp()
        val coordinator = CodexSessionCoordinator(filesDir, fake, this)

        coordinator.onLaunched(project, idsBeforeLaunch = emptySet())

        val session = coordinator.session.value!!
        assertEquals(VerbSessionState.LIVE, session.state)
        assertEquals("codex", session.runtime)
        assertEquals("codex", session.agent!!.agentType)
        assertNotNull(session.process)
        coordinator.cancelWatch()
    }

    @Test
    fun `codex exiting after a real conversation resolves to RECOVERABLE and captures its id`() = runTest {
        val (filesDir, project, fake) = setUp()
        writeRollout(filesDir, project, sessionId = "codex-session-1")
        val coordinator = CodexSessionCoordinator(filesDir, fake, this)

        coordinator.onLaunched(project, idsBeforeLaunch = emptySet())
        runCurrent()
        exitCodex(fake)
        advanceUntilIdle()

        val session = coordinator.session.value!!
        assertEquals(VerbSessionState.RECOVERABLE, session.state)
        assertNull("process must be cleared once Codex has exited", session.process)
        assertEquals(
            "the stable Codex conversation id is the resume identity, never a PID or filename",
            "codex-session-1",
            session.agent!!.resumeIdentity
        )
    }

    @Test
    fun `codex exiting after only being opened resolves to ENDED, not RECOVERABLE`() = runTest {
        val (filesDir, project, fake) = setUp()
        writeRollout(filesDir, project, sessionId = "codex-idle", userTurns = false)
        val coordinator = CodexSessionCoordinator(filesDir, fake, this)

        coordinator.onLaunched(project, idsBeforeLaunch = emptySet())
        runCurrent()
        exitCodex(fake)
        advanceUntilIdle()

        assertEquals(VerbSessionState.ENDED, coordinator.session.value!!.state)
    }

    @Test
    fun `codex exiting with no readable sessions tree stays INTERRUPTED, never guessing ENDED`() = runTest {
        val (filesDir, project, fake) = setUp()
        val coordinator = CodexSessionCoordinator(filesDir, fake, this)

        coordinator.onLaunched(project, idsBeforeLaunch = emptySet())
        runCurrent()
        exitCodex(fake)
        advanceUntilIdle()

        assertEquals(VerbSessionState.INTERRUPTED, coordinator.session.value!!.state)
    }

    @Test
    fun `successful resume - same VerbSession id, state becomes LIVE, new process`() = runTest {
        val (filesDir, project, fake) = setUp()
        writeRollout(filesDir, project, sessionId = "codex-session-1")
        val coordinator = CodexSessionCoordinator(filesDir, fake, this)
        coordinator.onLaunched(project, idsBeforeLaunch = emptySet())
        runCurrent()
        exitCodex(fake)
        advanceUntilIdle()
        val recoverableId = coordinator.session.value!!.id

        // Nothing new settles inside CodexAgentAdapter's resume window: Codex presumed still running.
        coordinator.resume()

        val resumed = coordinator.session.value!!
        assertEquals(VerbSessionState.LIVE, resumed.state)
        assertEquals("resume must preserve VerbSession.id, never mint a new one", recoverableId, resumed.id)
        assertNotNull(resumed.process)
        coordinator.cancelWatch()
    }

    @Test
    fun `failed resume never produces LIVE and keeps the session RECOVERABLE`() = runTest {
        val (filesDir, project, fake) = setUp()
        writeRollout(filesDir, project, sessionId = "codex-session-1")
        val coordinator = CodexSessionCoordinator(filesDir, fake, this)
        coordinator.onLaunched(project, idsBeforeLaunch = emptySet())
        runCurrent()
        exitCodex(fake)
        advanceUntilIdle()
        val recoverableId = coordinator.session.value!!.id

        val resumeJob = launch { coordinator.resume() }
        runCurrent()
        exitCodex(fake, exitCode = 1)
        advanceUntilIdle()
        resumeJob.join()

        val afterFailedResume = coordinator.session.value!!
        assertEquals("a failed resume must never report LIVE", VerbSessionState.RECOVERABLE, afterFailedResume.state)
        assertEquals(recoverableId, afterFailedResume.id)
    }

    @Test
    fun `a fresh Android process restores the same id and reconciles persisted LIVE`() = runTest {
        val (filesDir, project, fake) = setUp()
        writeRollout(filesDir, project, sessionId = "codex-session-1")
        val persisted = VerbSession(
            id = "persisted-codex-id",
            projectId = project.id,
            runtime = "codex",
            createdAt = Instant.EPOCH,
            lastSeenAt = Instant.EPOCH,
            state = VerbSessionState.LIVE,
            lastKnownCwd = project.directory.absolutePath,
            agent = AgentRef("codex")
        )

        val coordinator = CodexSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeAdapter = fake,
            coroutineScope = this,
            sessionStore = InMemoryVerbSessionStore(persisted),
            processBindingConfirmed = false
        )

        val restored = coordinator.session.value!!
        assertEquals("persisted-codex-id", restored.id)
        assertEquals(VerbSessionState.RECOVERABLE, restored.state)
        assertNull(restored.process)
        assertEquals("codex-session-1", restored.agent!!.resumeIdentity)
    }

    @Test
    fun `a record belonging to another agent is never adopted as this agent's session`() = runTest {
        // Each agent has its own store, so this should not arise -- but adopting Claude's record as
        // Codex's would mean resuming the wrong conversation, which is worth being unable to do.
        val (filesDir, project, fake) = setUp()
        val claudeRecord = VerbSession(
            id = "claude-id",
            projectId = project.id,
            runtime = "claude",
            createdAt = Instant.EPOCH,
            lastSeenAt = Instant.EPOCH,
            state = VerbSessionState.RECOVERABLE,
            lastKnownCwd = project.directory.absolutePath,
            agent = AgentRef("claude", resumeIdentity = "claude-conversation")
        )

        val coordinator = CodexSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeAdapter = fake,
            coroutineScope = this,
            sessionStore = InMemoryVerbSessionStore(claudeRecord)
        )

        assertNull(coordinator.session.value)
    }
}
