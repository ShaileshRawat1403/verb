package com.example.verb.session

import android.database.sqlite.SQLiteDatabase
import androidx.test.core.app.ApplicationProvider
import com.example.verb.project.VerbProject
import com.example.verb.terminal.FakeTerminalRuntimeAdapter
import com.example.verb.terminal.ShellIntegrationEvent
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File
import java.time.Instant

/**
 * OpenCode driving the same [AgentSessionCoordinator] Claude and Codex drive -- a third agent, and
 * still one state machine. Only the evidence lookup differs, and here it is a SQLite database.
 */
@RunWith(RobolectricTestRunner::class)
class OpenCodeSessionCoordinatorTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private val scratchDir: File
        get() = ApplicationProvider.getApplicationContext<android.content.Context>().cacheDir

    private fun setUp(): Triple<File, VerbProject, FakeTerminalRuntimeAdapter> {
        val filesDir = temporaryFolder.newFolder("files")
        val projectDir = File(filesDir, "projects/alpha").apply { mkdirs() }
        return Triple(filesDir, VerbProject(id = "alpha", directory = projectDir), FakeTerminalRuntimeAdapter(filesDir))
    }

    private fun writeConversation(filesDir: File, project: VerbProject, sessionId: String, used: Boolean = true) {
        val directory = File(filesDir, "home/.local/share/opencode").apply { mkdirs() }
        SQLiteDatabase.openOrCreateDatabase(File(directory, "opencode.db"), null).use { database ->
            database.execSQL(
                "CREATE TABLE IF NOT EXISTS session (id text PRIMARY KEY, project_id text NOT NULL," +
                    " parent_id text, slug text NOT NULL, directory text NOT NULL, title text NOT NULL," +
                    " version text NOT NULL, time_created integer NOT NULL, time_updated integer NOT NULL)"
            )
            database.execSQL(
                "CREATE TABLE IF NOT EXISTS message (id text PRIMARY KEY, session_id text NOT NULL," +
                    " time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL)"
            )
            database.execSQL(
                "INSERT INTO session (id, project_id, parent_id, slug, directory, title, version," +
                    " time_created, time_updated) VALUES (?,?,?,?,?,?,?,?,?)",
                arrayOf(sessionId, "p", null, sessionId, project.directory.absolutePath, "t", "1", 1L, 1L)
            )
            if (used) {
                database.execSQL(
                    "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?,?,?,?,?)",
                    arrayOf("$sessionId-m", sessionId, 1L, 1L, """{"role":"user","parts":[]}""")
                )
            }
        }
    }

    private fun coordinator(
        filesDir: File,
        fake: FakeTerminalRuntimeAdapter,
        scope: kotlinx.coroutines.CoroutineScope,
        store: VerbSessionStore = InMemoryVerbSessionStore()
    ) = OpenCodeSessionCoordinator(
        filesDir = filesDir,
        scratchDir = scratchDir,
        terminalRuntimeAdapter = fake,
        coroutineScope = scope,
        sessionStore = store
    )

    private fun exitOpenCode(fake: FakeTerminalRuntimeAdapter, exitCode: Int = 0) {
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(exitCode))
    }

    @Test
    fun `opencode exiting after a real conversation resolves to RECOVERABLE and captures its id`() = runTest {
        val (filesDir, project, fake) = setUp()
        writeConversation(filesDir, project, "opencode-session-1")
        val coordinator = coordinator(filesDir, fake, this)

        coordinator.onLaunched(project, idsBeforeLaunch = emptySet())
        runCurrent()
        exitOpenCode(fake)
        advanceUntilIdle()

        val session = coordinator.session.value!!
        assertEquals(VerbSessionState.RECOVERABLE, session.state)
        assertEquals("opencode", session.runtime)
        assertNull(session.process)
        assertEquals("opencode-session-1", session.agent!!.resumeIdentity)
    }

    @Test
    fun `opencode exiting after only being opened resolves to ENDED, not RECOVERABLE`() = runTest {
        val (filesDir, project, fake) = setUp()
        writeConversation(filesDir, project, "opencode-idle", used = false)
        val coordinator = coordinator(filesDir, fake, this)

        coordinator.onLaunched(project, idsBeforeLaunch = emptySet())
        runCurrent()
        exitOpenCode(fake)
        advanceUntilIdle()

        assertEquals(VerbSessionState.ENDED, coordinator.session.value!!.state)
    }

    @Test
    fun `a fresh Android process restores the same id and resumes to LIVE`() = runTest {
        val (filesDir, project, fake) = setUp()
        writeConversation(filesDir, project, "opencode-session-1")
        val persisted = VerbSession(
            id = "persisted-opencode-id",
            projectId = project.id,
            runtime = "opencode",
            createdAt = Instant.EPOCH,
            lastSeenAt = Instant.EPOCH,
            state = VerbSessionState.LIVE,
            lastKnownCwd = project.directory.absolutePath,
            agent = AgentRef("opencode")
        )

        val coordinator = coordinator(filesDir, fake, this, InMemoryVerbSessionStore(persisted))

        val restored = coordinator.session.value!!
        assertEquals("persisted-opencode-id", restored.id)
        assertEquals(VerbSessionState.RECOVERABLE, restored.state)
        assertEquals("opencode-session-1", restored.agent!!.resumeIdentity)

        // Nothing settles inside the resume window, so OpenCode is presumed still running.
        coordinator.resume()

        val resumed = coordinator.session.value!!
        assertEquals(VerbSessionState.LIVE, resumed.state)
        assertEquals("resume must preserve VerbSession.id", "persisted-opencode-id", resumed.id)
        assertNotNull(resumed.process)
        coordinator.cancelWatch()
    }
}
