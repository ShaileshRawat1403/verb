package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * The terminal session must stop belonging to the screen.
 *
 * `refreshEnvironment()` used to be a destroy-and-restart, and it was the only thing it did -- so
 * changing the launch directory and killing the terminal were the same operation. Project
 * selection, Agent Runtime switching and the ViewModel's own startup bootstrap check all went
 * through it, which meant selecting a project ended whatever was running (agent sessions included)
 * and every fresh process tore the PTY down while starting up.
 *
 * These tests pin the new rule: changing metadata never ends a healthy session. A change that needs
 * a new shell is reported, and takes effect only when the user asks for a restart.
 */
class SessionLifecycleOwnershipTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private lateinit var filesDir: File

    private fun runtime(project: File? = null): TerminalRuntime {
        filesDir = temporaryFolder.newFolder("files")
        File(filesDir, "projects").mkdirs()
        return TerminalRuntime(
            workingDir = filesDir,
            useFakeForTesting = true,
            initialProjectDirectory = project
        )
    }

    /** A real project directory, which is what the resolver requires to change the launch dir. */
    private fun project(name: String): File =
        File(filesDir, "projects/$name").apply { mkdirs() }

    /** The common startup case: nothing changed, so nothing may be torn down. */
    @Test
    fun `refreshing an unchanged environment leaves a live session alone`() {
        val runtime = runtime()
        runtime.startSession()
        assertTrue(runtime.isSessionActive.value)

        runtime.refreshEnvironment()

        assertTrue("a no-op refresh must not end the session", runtime.isSessionActive.value)
        assertFalse(runtime.pendingEnvironmentChange.value)
    }

    /**
     * The regression that matters most. Selecting a project used to destroy the PTY, which silently
     * ended a running agent.
     */
    @Test
    fun `selecting a project does not end a live session`() {
        val runtime = runtime()
        runtime.startSession()
        val project = project("alpha")

        runtime.selectProject(project)

        assertTrue("selecting a project must not end the session", runtime.isSessionActive.value)
    }

    /** A change that cannot take effect without a new shell is reported, not performed. */
    @Test
    fun `a project change under a live session is reported as pending`() {
        val runtime = runtime()
        runtime.startSession()

        runtime.selectProject(project("beta"))

        assertTrue(runtime.pendingEnvironmentChange.value)
    }

    /** With nothing to preserve there is no reason to make the user ask. */
    @Test
    fun `a project change applies immediately once the session has exited`() {
        val runtime = runtime()
        runtime.destroy()
        assertFalse(runtime.isSessionActive.value)

        runtime.selectProject(project("gamma"))

        assertFalse("nothing was running, so nothing is pending", runtime.pendingEnvironmentChange.value)
    }

    /** The restart the user asked for is exactly the new shell the pending change needed. */
    @Test
    fun `an explicit restart applies the pending change and clears it`() {
        val runtime = runtime()
        runtime.startSession()
        runtime.selectProject(project("delta"))
        assertTrue(runtime.pendingEnvironmentChange.value)

        runtime.restartSession()

        assertFalse(runtime.pendingEnvironmentChange.value)
    }

    /**
     * Switching back to the configuration already running is not a change. Without this, selecting
     * a project and selecting it back would leave a restart prompt on screen forever.
     */
    @Test
    fun `returning to the running configuration clears the pending change`() {
        val runtime = runtime()
        runtime.startSession()
        runtime.selectProject(project("epsilon"))
        assertTrue(runtime.pendingEnvironmentChange.value)

        runtime.selectProject(null)

        assertFalse(runtime.pendingEnvironmentChange.value)
    }

    /**
     * `environment` describes the session that exists. Answering with a queued configuration would
     * make diagnostics and guest-path translation describe a session that is not running.
     */
    @Test
    fun `the reported environment stays the one the live session was launched with`() {
        val runtime = runtime()
        runtime.startSession()
        val before = runtime.environment

        runtime.selectProject(project("zeta"))

        assertEquals("a pending change must not be reported as the live environment", before, runtime.environment)
    }

    /** Deactivating an Agent Runtime that was never active changes nothing, so it may not restart. */
    @Test
    fun `deactivating an inactive agent runtime leaves the session alone`() {
        val runtime = runtime()
        runtime.startSession()

        runtime.deactivateAgentRuntime()

        assertTrue(runtime.isSessionActive.value)
        assertFalse(runtime.pendingEnvironmentChange.value)
    }
}
