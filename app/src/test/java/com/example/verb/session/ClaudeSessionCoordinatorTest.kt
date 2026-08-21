package com.example.verb.session

import com.example.verb.project.VerbProject
import com.example.verb.terminal.FakeTerminalRuntimeAdapter
import com.example.verb.terminal.ShellIntegrationEvent
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

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
        File(filesDir, "home/.claude/projects/${project.directory.absolutePath.replace('/', '-')}")

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
}
