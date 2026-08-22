package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * Verb sets a base PATH with its own agent launchers first, but a base PATH is only a starting
 * point: `$HOME/.bashrc` runs afterwards. The Codex installer writes
 * `export PATH="$HOME/.local/bin:$PATH"` there, which put the vendor launchers back in front --
 * and `$HOME/.local/bin/claude` is the one that fails with `has unexpected e_type: 2`.
 *
 * That is how an installed, authenticated Claude Code stayed unreachable from the terminal while
 * Verb's readiness probe reported it Ready: [GuestCommandRunner] never sources user startup files,
 * so the probe and the real shell disagreed about what `claude` even resolves to. These tests pin
 * the shell-side half of the fix.
 */
class AgentPathGuardTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    /** The exact line the Codex installer wrote on the validation device. */
    private val codexInstallerLine =
        "\n# >>> Codex installer >>>\n" +
            "export PATH=\"${VerbGuestPaths.HOME}/.local/bin:\$PATH\"\n" +
            "# <<< Codex installer <<<\n"

    private fun filesDir() = temporaryFolder.newFolder("files").also { File(it, "home").mkdirs() }

    private fun bashrc(filesDir: File) = File(filesDir, "home/.bashrc")

    @Test
    fun `the guard is appended to a bashrc that reorders PATH`() {
        val filesDir = filesDir()
        bashrc(filesDir).writeText(codexInstallerLine)

        assertTrue(TermuxBootstrapInstaller.ensureAgentPathGuardLast(filesDir))

        val contents = bashrc(filesDir).readText()
        assertTrue(contents.contains(AgentWrapperBootstrap.GUEST_BIN_DIR))
        // The installer's own line is left completely alone.
        assertTrue(contents.contains("export PATH=\"${VerbGuestPaths.HOME}/.local/bin:\$PATH\""))
    }

    /**
     * Position is the entire point. A guard that runs before the installer's line would be undone
     * by it, which is exactly the bug.
     */
    @Test
    fun `the guard runs after the line that reorders PATH`() {
        val filesDir = filesDir()
        bashrc(filesDir).writeText(codexInstallerLine)

        TermuxBootstrapInstaller.ensureAgentPathGuardLast(filesDir)

        val contents = bashrc(filesDir).readText()
        assertTrue(
            "Verb's guard must come last",
            contents.indexOf(AgentWrapperBootstrap.GUEST_BIN_DIR) > contents.indexOf(".local/bin:\$PATH")
        )
    }

    /**
     * The failure mode this protects against long-term: another installer appends to `.bashrc`
     * after Verb did. Verb's block has to move back to the end, or the bug returns exactly as it
     * arrived the first time.
     */
    @Test
    fun `a guard that has been outranked is moved back to the end`() {
        val filesDir = filesDir()
        bashrc(filesDir).writeText(codexInstallerLine)
        TermuxBootstrapInstaller.ensureAgentPathGuardLast(filesDir)
        bashrc(filesDir).appendText("\nexport PATH=\"${VerbGuestPaths.HOME}/.other/bin:\$PATH\"\n")

        assertTrue(TermuxBootstrapInstaller.ensureAgentPathGuardLast(filesDir))

        val contents = bashrc(filesDir).readText()
        assertTrue(
            "Verb's guard must be moved back after the newer installer",
            contents.indexOf(AgentWrapperBootstrap.GUEST_BIN_DIR) > contents.indexOf(".other/bin:\$PATH")
        )
        // Moved, not duplicated -- otherwise every launch would grow the file.
        assertEquals(1, contents.split("# >>> Verb agent PATH >>>").size - 1)
    }

    @Test
    fun `re-running on an already correct bashrc changes nothing`() {
        val filesDir = filesDir()
        bashrc(filesDir).writeText(codexInstallerLine)
        TermuxBootstrapInstaller.ensureAgentPathGuardLast(filesDir)
        val afterFirst = bashrc(filesDir).readText()

        assertFalse(TermuxBootstrapInstaller.ensureAgentPathGuardLast(filesDir))
        assertEquals(afterFirst, bashrc(filesDir).readText())
    }

    /** Verb rewrote a file the user may have edited, so the original is preserved once. */
    @Test
    fun `the pre-existing bashrc is backed up once`() {
        val filesDir = filesDir()
        bashrc(filesDir).writeText(codexInstallerLine)

        TermuxBootstrapInstaller.ensureAgentPathGuardLast(filesDir)
        val backup = File(filesDir, "home/.bashrc.pre-verb-agent-path")
        assertEquals(codexInstallerLine, backup.readText())

        bashrc(filesDir).appendText("\nexport PATH=\"/late:\$PATH\"\n")
        TermuxBootstrapInstaller.ensureAgentPathGuardLast(filesDir)
        assertEquals("the backup must never be overwritten", codexInstallerLine, backup.readText())
    }

    /** Re-sourcing a startup file must not grow PATH without bound. */
    @Test
    fun `the guard is a no-op when the directory already leads PATH`() {
        val filesDir = filesDir()
        TermuxBootstrapInstaller.ensureAgentPathGuardLast(filesDir)

        val guard = bashrc(filesDir).readText()
        assertTrue(guard.contains("case \"\${PATH}\" in"))
        assertTrue(guard.contains("\"${AgentWrapperBootstrap.GUEST_BIN_DIR}:\"*) ;;"))
    }

    /**
     * A login shell reads `.bash_profile`, not `.bashrc`, and the shell-integration script is what
     * it sources afterwards -- so the same correction has to live there too, ahead of both of that
     * script's early returns so it also applies to non-interactive and re-sourced shells.
     */
    @Test
    fun `the shell integration script fixes PATH before it can return early`() {
        val filesDir = filesDir()
        TermuxBootstrapInstaller.writeShellIntegrationScript(filesDir)

        val script = File(filesDir, "usr/etc/verb/shell-integration.bash").readText()
        val fixCall = script.indexOf("\n__verb_fix_agent_path\n")
        val loadedGuard = script.indexOf("VERB_SHELL_INTEGRATION_LOADED:-")
        val interactiveGuard = script.indexOf("case \"\$-\" in")

        assertTrue("the PATH fix must be called", fixCall > 0)
        assertTrue("it must run before the loaded guard", fixCall < loadedGuard)
        assertTrue("it must run before the interactive guard", fixCall < interactiveGuard)
        assertTrue(script.contains(AgentWrapperBootstrap.GUEST_BIN_DIR))
    }

    /** Provisioning runs on every launch, so the guard must be reachable from the one entry point. */
    @Test
    fun `keeping guest startup current also places the guard`() {
        val filesDir = filesDir()
        bashrc(filesDir).writeText(codexInstallerLine)

        TermuxBootstrapInstaller.ensureGuestShellStartupCurrent(filesDir)

        assertTrue(bashrc(filesDir).readText().contains(AgentWrapperBootstrap.GUEST_BIN_DIR))
    }
}
