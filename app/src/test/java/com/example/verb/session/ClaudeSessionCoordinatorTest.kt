package com.example.verb.session

import com.example.verb.project.VerbProject
import com.example.verb.terminal.FakeTerminalRuntimeAdapter
import com.example.verb.terminal.ShellIntegrationEvent
import kotlinx.coroutines.launch
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.time.Instant

@OptIn(ExperimentalCoroutinesApi::class)
class ClaudeSessionCoordinatorTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun setUp(): Triple<File, VerbProject, FakeTerminalRuntimeAdapter> {
        val filesDir = temporaryFolder.newFolder("files")
        val projectDir = File(filesDir, "projects/alpha").apply { mkdirs() }
        val project = VerbProject(id = "alpha", directory = projectDir)
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        return Triple(filesDir, project, fake)
    }

    private fun transcriptDir(filesDir: File, project: VerbProject): File =
        File(filesDir, "home/.claude/projects/${ClaudeProjectDirectory.encode(project.directory.absolutePath)}")

    @Test
    fun `onLaunched sets LIVE with a fresh id and a process bound`() = runTest {
        val (filesDir, project, fake) = setUp()
        val coordinator = ClaudeSessionCoordinator(filesDir, fake, this)

        coordinator.onLaunched(project, idsBeforeLaunch = emptySet())

        val session = coordinator.session.value
        assertNotNull(session)
        assertEquals(VerbSessionState.LIVE, session!!.state)
        assertNotNull(session.process)
        assertEquals(project.id, session.projectId)
        coordinator.cancelWatch() // nothing ever settles in this test; don't poll past it
    }

    @Test
    fun `starting a new session after one already exists mints a different id`() = runTest {
        val (filesDir, project, fake) = setUp()
        val coordinator = ClaudeSessionCoordinator(filesDir, fake, this)

        coordinator.onLaunched(project, idsBeforeLaunch = emptySet())
        val firstId = coordinator.session.value!!.id

        coordinator.onLaunched(project, idsBeforeLaunch = emptySet())
        val secondId = coordinator.session.value!!.id

        assertNotEquals("Start new must never reuse the previous VerbSession.id", firstId, secondId)
        coordinator.cancelWatch() // nothing ever settles in this test; don't poll past it
    }

    @Test
    fun `claude exiting with no project resolves to INTERRUPTED, and stays there`() = runTest {
        val (filesDir, _, fake) = setUp()
        val coordinator = ClaudeSessionCoordinator(filesDir, fake, this)
        val idsBefore = fake.commandHistory.value.mapTo(mutableSetOf()) { it.id }

        coordinator.onLaunched(project = null, idsBeforeLaunch = idsBefore)
        runCurrent()
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(0))
        advanceUntilIdle()

        assertEquals(VerbSessionState.INTERRUPTED, coordinator.session.value!!.state)
    }

    @Test
    fun `claude exiting with a transcript already on disk resolves to RECOVERABLE`() = runTest {
        val (filesDir, project, fake) = setUp()
        transcriptDir(filesDir, project).apply { mkdirs() }
            .let { File(it, "session-x.jsonl").createNewFile() }
        val coordinator = ClaudeSessionCoordinator(filesDir, fake, this)
        val idsBefore = fake.commandHistory.value.mapTo(mutableSetOf()) { it.id }

        coordinator.onLaunched(project, idsBeforeLaunch = idsBefore)
        runCurrent()
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(1))
        advanceUntilIdle()

        val session = coordinator.session.value!!
        assertEquals(VerbSessionState.RECOVERABLE, session.state)
        assertNull("process must be cleared once Claude has exited", session.process)
    }

    @Test
    fun `successful resume - same VerbSession id, state becomes LIVE, new process`() = runTest {
        val (filesDir, project, fake) = setUp()
        transcriptDir(filesDir, project).apply { mkdirs() }
            .let { File(it, "session-x.jsonl").createNewFile() }
        val coordinator = ClaudeSessionCoordinator(filesDir, fake, this)
        val idsBefore = fake.commandHistory.value.mapTo(mutableSetOf()) { it.id }
        coordinator.onLaunched(project, idsBeforeLaunch = idsBefore)
        runCurrent()
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(0))
        advanceUntilIdle()
        val recoverableId = coordinator.session.value!!.id
        assertEquals(VerbSessionState.RECOVERABLE, coordinator.session.value!!.state)

        // Resume: nothing new settles within ClaudeAgentAdapter's resume window -- Claude presumed
        // still running.
        coordinator.resume()

        val resumed = coordinator.session.value!!
        assertEquals(VerbSessionState.LIVE, resumed.state)
        assertEquals("resume must preserve VerbSession.id, never mint a new one", recoverableId, resumed.id)
        assertNotNull(resumed.process)
        coordinator.cancelWatch() // the new watch for the resumed session never settles in this test
    }

    @Test
    fun `failed resume never produces LIVE and keeps the session RECOVERABLE`() = runTest {
        val (filesDir, project, fake) = setUp()
        transcriptDir(filesDir, project).apply { mkdirs() }
            .let { File(it, "session-x.jsonl").createNewFile() }
        val coordinator = ClaudeSessionCoordinator(filesDir, fake, this)
        val idsBefore = fake.commandHistory.value.mapTo(mutableSetOf()) { it.id }
        coordinator.onLaunched(project, idsBeforeLaunch = idsBefore)
        runCurrent()
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(0))
        advanceUntilIdle()
        val recoverableId = coordinator.session.value!!.id

        // A background job runs resume(); once it has issued the resume command, simulate that
        // attempt failing before the settle window closes.
        val resumeJob = launch { coordinator.resume() }
        runCurrent()
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(1))
        advanceUntilIdle()
        resumeJob.join()

        val afterFailedResume = coordinator.session.value!!
        assertEquals(
            "a failed resume must never report LIVE",
            VerbSessionState.RECOVERABLE,
            afterFailedResume.state
        )
        assertEquals(recoverableId, afterFailedResume.id)
    }

    @Test
    fun `fresh process restores the same id and reconciles LIVE to RECOVERABLE`() = runTest {
        val (filesDir, project, fake) = setUp()
        transcriptDir(filesDir, project).apply { mkdirs() }
            .let { File(it, "session-x.jsonl").createNewFile() }
        val persisted = VerbSession(
            id = "persisted-id",
            projectId = project.id,
            runtime = "claude",
            createdAt = Instant.EPOCH,
            lastSeenAt = Instant.EPOCH,
            state = VerbSessionState.LIVE,
            lastKnownCwd = project.directory.absolutePath,
            agent = AgentRef("claude")
        )
        val store = InMemoryVerbSessionStore(persisted)

        val coordinator = ClaudeSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeAdapter = fake,
            coroutineScope = this,
            sessionStore = store,
            processBindingConfirmed = false
        )

        assertEquals("persisted-id", coordinator.session.value!!.id)
        assertEquals(VerbSessionState.RECOVERABLE, coordinator.session.value!!.state)
        assertNull(coordinator.session.value!!.process)
    }

    @Test
    fun `existing process binding is the only reason persisted LIVE stays LIVE`() = runTest {
        val (filesDir, project, fake) = setUp()
        VerbTerminalSessionHolder.resetForTests()
        // A foreground claim belongs to a terminal, so there has to be one. The coordinator reads
        // its own adapter for session state and the holder only for *which agent holds the front*,
        // so this session and `fake` are deliberately different objects.
        VerbTerminalSessionHolder.getOrCreateActive {
            com.example.verb.terminal.TerminalRuntime(workingDir = filesDir, useFakeForTesting = true)
        }
        assertTrue(
            "the claim must land on a session, not vanish",
            VerbTerminalSessionHolder.claimForeground("claude", emptySet())
        )
        val persisted = VerbSession(
            id = "persisted-id",
            projectId = project.id,
            runtime = "claude",
            createdAt = Instant.EPOCH,
            lastSeenAt = Instant.EPOCH,
            state = VerbSessionState.LIVE,
            lastKnownCwd = project.directory.absolutePath,
            agent = AgentRef("claude")
        )
        val coordinator = ClaudeSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeAdapter = fake,
            coroutineScope = this,
            sessionStore = InMemoryVerbSessionStore(persisted),
            processBindingConfirmed = true
        )

        assertEquals("persisted-id", coordinator.session.value!!.id)
        assertEquals(VerbSessionState.LIVE, coordinator.session.value!!.state)
        assertNotNull(coordinator.session.value!!.process)
        coordinator.cancelWatch()
        VerbTerminalSessionHolder.resetForTests()
    }

    @Test
    fun `Activity recreation observes an agent exit that happened before reattachment`() = runTest {
        val (filesDir, project, fake) = setUp()
        transcriptDir(filesDir, project).apply { mkdirs() }
            .let { File(it, "session-x.jsonl").createNewFile() }
        VerbTerminalSessionHolder.resetForTests()
        val store = InMemoryVerbSessionStore()
        val first = ClaudeSessionCoordinator(filesDir, fake, this, sessionStore = store)

        first.onLaunched(project, idsBeforeLaunch = emptySet())
        runCurrent()
        first.cancelWatch() // the old ViewModel and its coroutine scope disappear
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(0))

        val reattached = ClaudeSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeAdapter = fake,
            coroutineScope = this,
            sessionStore = store,
            processBindingConfirmed = true
        )
        advanceUntilIdle()

        assertEquals(VerbSessionState.RECOVERABLE, reattached.session.value!!.state)
        assertNull(reattached.session.value!!.process)
        assertNull(VerbTerminalSessionHolder.foregroundAgent())
        VerbTerminalSessionHolder.resetForTests()
    }
}
