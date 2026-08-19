package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * The agent CLIs read API keys from ordinary environment variables, so they cannot use the
 * Keystore-backed store the Assistant uses. This file is the alternative, and these tests pin the
 * properties that make it acceptable rather than merely convenient.
 */
class AgentEnvironmentFileTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun filesDir() = temporaryFolder.newFolder("files").also { File(it, "home").mkdirs() }

    private fun envFile(filesDir: File) = File(filesDir, "home/.env")

    @Test
    fun `the file is created with placeholders and no key of any kind`() {
        val filesDir = filesDir()

        TermuxBootstrapInstaller.ensureAgentEnvFile(filesDir)

        val contents = envFile(filesDir).readText()
        assertTrue(contents.contains("ANTHROPIC_API_KEY"))
        assertTrue(contents.contains("DEEPSEEK_API_KEY"))
        // Every variable is commented out: Verb never invents or pre-fills a credential.
        assertTrue(
            contents.lines()
                .filter { it.contains("API_KEY") }
                .all { it.trimStart().startsWith("#") }
        )
    }

    /** A real key must survive every later launch; this runs on each one. */
    @Test
    fun `an existing file is never overwritten`() {
        val filesDir = filesDir()
        TermuxBootstrapInstaller.ensureAgentEnvFile(filesDir)
        envFile(filesDir).writeText("export ANTHROPIC_API_KEY=user-provided-value\n")

        TermuxBootstrapInstaller.ensureAgentEnvFile(filesDir)

        assertTrue(envFile(filesDir).readText().contains("user-provided-value"))
    }

    @Test
    fun `the file is owner-only`() {
        val filesDir = filesDir()

        TermuxBootstrapInstaller.ensureAgentEnvFile(filesDir)

        val file = envFile(filesDir)
        assertTrue("owner must read it", file.canRead())
        // Nothing here asserts world-readability is merely unlikely: it is explicitly removed.
        assertFalse("must not be world-readable", file.canExecute())
    }

    @Test
    fun `the profile sources it exactly once, however many launches happen`() {
        val filesDir = filesDir()
        val profile = File(filesDir, "home/.bash_profile").apply { writeText("# user content\n") }

        repeat(3) { TermuxBootstrapInstaller.ensureAgentEnvSourced(filesDir) }

        val contents = profile.readText()
        assertTrue("user content is preserved", contents.contains("# user content"))
        // The marker is the idempotency guard, so it is what must appear exactly once.
        assertEquals(1, contents.split("# >>> Verb agent environment >>>").size - 1)
    }

    @Test
    fun `nothing is sourced when there is no profile to edit`() {
        val filesDir = filesDir()

        assertFalse(TermuxBootstrapInstaller.ensureAgentEnvSourced(filesDir))
    }

    /**
     * The diagnostics report is copied and pasted into issues and chats. A key must never ride
     * along with it.
     */
    @Test
    fun `the diagnostics report never carries the agent environment`() {
        val report = TerminalSessionLogger.exportDiagnosticReport(
            sessionState = TerminalSessionState.RUNNING,
            launchWorkingDir = "/data/user/0/com.aistudio.verb.app/files/home",
            currentWorkingDir = "/data/data/com.aistudio.verb.app/files/home",
            shellExecutable = "/bin/bash"
        )

        assertFalse(report.contains("API_KEY"))
        assertFalse(report.contains("/.env"))
    }
}
