package com.example.verb.session

import android.database.sqlite.SQLiteDatabase
import androidx.test.core.app.ApplicationProvider
import com.example.verb.terminal.FakeTerminalRuntimeAdapter
import com.example.verb.terminal.ShellIntegrationEvent
import kotlinx.coroutines.async
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.rules.TemporaryFolder
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * Robolectric, unlike the Claude and Codex adapter tests, because OpenCode's evidence *is* a SQLite
 * database and the test writes a real one with the schema read off the installed build.
 */
@RunWith(RobolectricTestRunner::class)
@OptIn(ExperimentalCoroutinesApi::class)
class OpenCodeAgentAdapterTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private val scratchDir: File
        get() = ApplicationProvider.getApplicationContext<android.content.Context>().cacheDir

    private fun setUpFilesystem(): Pair<File, File> {
        val filesDir = temporaryFolder.newFolder("files")
        val project = File(filesDir, "projects/alpha").apply { mkdirs() }
        return filesDir to project
    }

    /**
     * The two tables the adapter reads, with the column names and types the real
     * `~/.local/share/opencode/opencode.db` uses.
     */
    private fun openCodeDatabase(filesDir: File): SQLiteDatabase {
        val directory = File(filesDir, "home/.local/share/opencode").apply { mkdirs() }
        val database = SQLiteDatabase.openOrCreateDatabase(File(directory, "opencode.db"), null)
        database.execSQL(
            """
            CREATE TABLE session (
              id text PRIMARY KEY, project_id text NOT NULL, workspace_id text, parent_id text,
              slug text NOT NULL, directory text NOT NULL, title text NOT NULL, version text NOT NULL,
              time_created integer NOT NULL, time_updated integer NOT NULL
            )
            """.trimIndent()
        )
        database.execSQL(
            """
            CREATE TABLE message (
              id text PRIMARY KEY, session_id text NOT NULL, time_created integer NOT NULL,
              time_updated integer NOT NULL, data text NOT NULL
            )
            """.trimIndent()
        )
        return database
    }

    private fun SQLiteDatabase.insertSession(
        id: String,
        directory: String,
        updatedAt: Long = 1L,
        parentId: String? = null
    ) = execSQL(
        "INSERT INTO session (id, project_id, parent_id, slug, directory, title, version," +
            " time_created, time_updated) VALUES (?,?,?,?,?,?,?,?,?)",
        arrayOf<Any?>(id, "project-1", parentId, id, directory, "a session", "1.0.0", 1L, updatedAt)
    )

    private fun SQLiteDatabase.insertUserMessage(sessionId: String, text: String = "hi") = execSQL(
        "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?,?,?,?,?)",
        arrayOf<Any?>("$sessionId-msg", sessionId, 1L, 1L, """{"role":"user","parts":[{"text":"$text"}]}""")
    )

    private fun adapter(filesDir: File, project: File?, fake: FakeTerminalRuntimeAdapter) =
        OpenCodeAgentAdapter(filesDir, project, fake, scratchDir, resumeSettleMs = 200, pollIntervalMs = 20)

    // --- canResume(): opened is not used, and unreadable is not "no" ---

    @Test
    fun `canResume is YES when a session for this project holds a user message`() {
        val (filesDir, project) = setUpFilesystem()
        openCodeDatabase(filesDir).use {
            it.insertSession("session-1", project.absolutePath)
            it.insertUserMessage("session-1")
        }

        val adapter = adapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.YES, adapter.canResume(AgentRef("opencode")))
        assertEquals("session-1", adapter.resumeIdentity(AgentRef("opencode")))
    }

    @Test
    fun `canResume is NO for a session that was only ever opened, never used`() {
        // OpenCode writes the session row at startup; only a message proves a conversation.
        val (filesDir, project) = setUpFilesystem()
        openCodeDatabase(filesDir).use { it.insertSession("session-idle", project.absolutePath) }

        val adapter = adapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.NO, adapter.canResume(AgentRef("opencode")))
        assertNull(adapter.resumeIdentity(AgentRef("opencode")))
    }

    @Test
    fun `canResume is NO when the database holds conversations only for other projects`() {
        val (filesDir, project) = setUpFilesystem()
        openCodeDatabase(filesDir).use {
            it.insertSession("session-elsewhere", "/somewhere/else")
            it.insertUserMessage("session-elsewhere")
        }

        val adapter = adapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.NO, adapter.canResume(AgentRef("opencode")))
    }

    @Test
    fun `canResume is NO when the known resumeIdentity is not in the database`() {
        val (filesDir, project) = setUpFilesystem()
        openCodeDatabase(filesDir).use {
            it.insertSession("session-1", project.absolutePath)
            it.insertUserMessage("session-1")
        }

        val adapter = adapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.NO, adapter.canResume(AgentRef("opencode", "never-happened")))
    }

    @Test
    fun `canResume is UNKNOWN, never NO, when OpenCode has never written a database`() {
        val (filesDir, project) = setUpFilesystem()

        val adapter = adapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.UNKNOWN, adapter.canResume(AgentRef("opencode", "abc")))
    }

    @Test
    fun `canResume is UNKNOWN, never NO, when there is no project to check`() {
        val (filesDir, _) = setUpFilesystem()
        openCodeDatabase(filesDir).close()

        val adapter = adapter(filesDir, project = null, fake = FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.UNKNOWN, adapter.canResume(AgentRef("opencode", "abc")))
    }

    @Test
    fun `canResume is UNKNOWN, never NO, when the database cannot be read`() {
        val (filesDir, project) = setUpFilesystem()
        File(filesDir, "home/.local/share/opencode").apply { mkdirs() }
            .resolve("opencode.db")
            .writeText("this is not a database")

        val adapter = adapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.UNKNOWN, adapter.canResume(AgentRef("opencode")))
    }

    @Test
    fun `a sub-session is never offered as the thing to resume`() {
        // OpenCode records child sessions (sub-agents) in the same table; resuming one instead of
        // the conversation the user was actually having would restore the wrong thing.
        val (filesDir, project) = setUpFilesystem()
        openCodeDatabase(filesDir).use {
            it.insertSession("session-parent", project.absolutePath, updatedAt = 10L)
            it.insertUserMessage("session-parent")
            it.insertSession("session-child", project.absolutePath, updatedAt = 99L, parentId = "session-parent")
            it.insertUserMessage("session-child")
        }

        val adapter = adapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals("session-parent", adapter.resumeIdentity(AgentRef("opencode")))
    }

    @Test
    fun `resumeIdentity returns the most recently updated used session`() {
        val (filesDir, project) = setUpFilesystem()
        openCodeDatabase(filesDir).use {
            it.insertSession("session-older", project.absolutePath, updatedAt = 10L)
            it.insertUserMessage("session-older")
            it.insertSession("session-newer", project.absolutePath, updatedAt = 20L)
            it.insertUserMessage("session-newer")
        }

        val adapter = adapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals("session-newer", adapter.resumeIdentity(AgentRef("opencode")))
    }

    @Test
    fun `a session recorded under the other Android path alias still matches the project`() {
        val filesDir = temporaryFolder.newFolder("data", "user", "0", "com.aistudio.verb.app", "files")
        val project = File(filesDir, "projects/alpha").apply { mkdirs() }
        openCodeDatabase(filesDir).use {
            it.insertSession("session-alias", "/data/data/com.aistudio.verb.app/files/projects/alpha")
            it.insertUserMessage("session-alias")
        }

        val adapter = adapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        // Both sides live under a `files` tree, as they do on the device, so the shared suffix
        // rule resolves the two Android spellings to the same project.
        assertEquals(ResumeVerdict.YES, adapter.canResume(AgentRef("opencode")))
        assertEquals("session-alias", adapter.resumeIdentity(AgentRef("opencode")))
    }

    @Test
    fun `reading the evidence never disturbs OpenCode's own database`() {
        val (filesDir, project) = setUpFilesystem()
        val databaseFile = File(filesDir, "home/.local/share/opencode/opencode.db")
        openCodeDatabase(filesDir).use {
            it.insertSession("session-1", project.absolutePath)
            it.insertUserMessage("session-1")
        }
        val before = databaseFile.readBytes()

        adapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir)).canResume(AgentRef("opencode"))

        assertTrue("the live database must be read through a copy, never opened in place", before.contentEquals(databaseFile.readBytes()))
        assertTrue(
            "the scratch copy must not be left behind",
            scratchDir.listFiles()?.none { it.name.startsWith("opencode-session-read-") } ?: true
        )
    }

    // --- resume(): flags read from `opencode --help` on the installed build ---

    @Test
    fun `resume runs opencode --session with the conversation id and succeeds when nothing settles`() = runTest {
        val (filesDir, project) = setUpFilesystem()
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        val adapter = adapter(filesDir, project, fake)

        val binding = adapter.resume(AgentRef("opencode", "session-1"))

        assertNotNull(binding)
        assertTrue(fake.terminalOutput.value.contains("opencode --session session-1"))
    }

    @Test
    fun `resume falls back to --continue when no id is known`() = runTest {
        val (filesDir, project) = setUpFilesystem()
        val fake = FakeTerminalRuntimeAdapter(filesDir)

        adapter(filesDir, project, fake).resume(AgentRef("opencode", resumeIdentity = null))

        assertTrue(fake.terminalOutput.value.contains("opencode --continue"))
    }

    @Test
    fun `resume fails when a new command settles before the window closes`() = runTest {
        val (filesDir, project) = setUpFilesystem()
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        val adapter = OpenCodeAgentAdapter(
            filesDir, project, fake, scratchDir, resumeSettleMs = 5_000, pollIntervalMs = 50
        )

        val resumeCall = async { adapter.resume(AgentRef("opencode", "session-1")) }
        runCurrent()
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(exitCode = 0))
        advanceUntilIdle()

        assertNull("a settled record appearing at all -- any exit code -- must never read as success", resumeCall.await())
    }
}
