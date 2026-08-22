package com.example.verb.terminal

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class TermuxBootstrapInstallerTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `removes unsigned Verb repository while preserving signed sources`() {
        val filesDir = temporaryFolder.newFolder("files")
        val sources = File(filesDir, "usr/etc/apt/sources.list").apply {
            parentFile?.mkdirs()
            writeText(
                "deb https://repository.su/termux/termux-main/ stable main\n" +
                    "deb [trusted=yes] https://shaileshrawat1403.github.io/verb/apt/ ./\n"
            )
        }

        TermuxBootstrapInstaller.ensureSecureAptSources(filesDir)

        val contents = sources.readText()
        assertTrue(contents.contains("repository.su/termux/termux-main"))
        assertFalse(contents.contains("github.io/verb/apt"))
        assertFalse(contents.contains("trusted=yes"))
    }

    @Test
    fun `creates a bash_profile that sources bashrc when none exists`() {
        val filesDir = temporaryFolder.newFolder("files")

        TermuxBootstrapInstaller.ensureLoginShellSourcesBashrc(filesDir)

        val bashProfile = File(filesDir, "home/.bash_profile")
        assertTrue(bashProfile.isFile)
        assertTrue(bashProfile.readText().contains(".bashrc"))
    }

    @Test
    fun `never overwrites an existing bash_profile`() {
        val filesDir = temporaryFolder.newFolder("files")
        val home = File(filesDir, "home").apply { mkdirs() }
        val bashProfile = File(home, ".bash_profile").apply { writeText("# user's own setup\n") }

        TermuxBootstrapInstaller.ensureLoginShellSourcesBashrc(filesDir)

        assertEquals("# user's own setup\n", bashProfile.readText())
    }

    @Test
    fun `fresh guest startup creates the bash_profile bridge and has nothing to migrate`() {
        val filesDir = temporaryFolder.newFolder("files")

        TermuxBootstrapInstaller.ensureGuestShellStartupCurrent(filesDir)

        assertTrue(File(filesDir, "home/.bash_profile").isFile)
        assertFalse(File(filesDir, "home/.bashrc.pre-verb-identity-migration").exists())
    }

    @Test
    fun `already-installed guest startup migrates an existing legacy bashrc on launch`() {
        // Simulates the real bug found on device: a bootstrap installed before this fix, whose
        // .bashrc a third-party installer already wrote a legacy com.termux path into, reaching
        // app launch through the already-installed path (not TermuxBootstrapInstaller.install()).
        val filesDir = temporaryFolder.newFolder("files")
        val home = File(filesDir, "home").apply { mkdirs() }
        val bashrc = File(home, ".bashrc").apply {
            writeText("export PATH=/data/data/com.termux/files/home/.local/bin:\$PATH\n")
        }

        TermuxBootstrapInstaller.ensureGuestShellStartupCurrent(filesDir)

        assertTrue(File(home, ".bash_profile").isFile)
        assertFalse(bashrc.readText().contains("com.termux"))
        assertTrue(bashrc.readText().contains(VerbGuestPaths.HOME))

        // Idempotent: a second launch (the normal case from then on) changes nothing further.
        val bashrcAfterFirstRun = bashrc.readText()
        val bashProfileAfterFirstRun = File(home, ".bash_profile").readText()
        TermuxBootstrapInstaller.ensureGuestShellStartupCurrent(filesDir)
        assertEquals(bashrcAfterFirstRun, bashrc.readText())
        assertEquals(bashProfileAfterFirstRun, File(home, ".bash_profile").readText())
    }

    @Test
    fun `migrates Codex and OpenCode style hard-coded legacy paths in bashrc`() {
        val filesDir = temporaryFolder.newFolder("files")
        val home = File(filesDir, "home").apply { mkdirs() }
        val bashrc = File(home, ".bashrc").apply {
            writeText(
                "# >>> Codex installer >>>\n" +
                    "export PATH=\"/data/data/com.termux/files/home/.local/bin:\$PATH\"\n" +
                    "# <<< Codex installer <<<\n" +
                    "\n" +
                    "# opencode\n" +
                    "export PATH=/data/data/com.termux/files/home/.opencode/bin:\$PATH\n"
            )
        }

        val result = TermuxBootstrapInstaller.migrateLegacyGuestPaths(filesDir)

        assertTrue(result.migrated)
        val migratedContents = bashrc.readText()
        assertFalse(migratedContents.contains("/data/data/com.termux"))
        assertTrue(migratedContents.contains("${VerbGuestPaths.HOME}/.local/bin"))
        assertTrue(migratedContents.contains("${VerbGuestPaths.HOME}/.opencode/bin"))
        // Comments, blank lines, and the installer's own markers are untouched.
        assertTrue(migratedContents.contains("# >>> Codex installer >>>"))
        assertTrue(migratedContents.contains("# <<< Codex installer <<<"))
        assertTrue(migratedContents.contains("# opencode"))
    }

    @Test
    fun `migrates a legacy PREFIX path distinctly from a legacy HOME path`() {
        val filesDir = temporaryFolder.newFolder("files")
        val home = File(filesDir, "home").apply { mkdirs() }
        val bashrc = File(home, ".bashrc").apply {
            writeText("export PATH=\"/data/data/com.termux/files/usr/bin:\$PATH\"\n")
        }

        TermuxBootstrapInstaller.migrateLegacyGuestPaths(filesDir)

        val migratedContents = bashrc.readText()
        assertEquals("export PATH=\"${VerbGuestPaths.PREFIX}/bin:\$PATH\"\n", migratedContents)
    }

    @Test
    fun `preserves a backup of the pre-migration bashrc, written only once`() {
        val filesDir = temporaryFolder.newFolder("files")
        val home = File(filesDir, "home").apply { mkdirs() }
        val originalContents = "export PATH=/data/data/com.termux/files/home/.local/bin:\$PATH\n"
        File(home, ".bashrc").writeText(originalContents)

        TermuxBootstrapInstaller.migrateLegacyGuestPaths(filesDir)
        val backup = File(home, ".bashrc.pre-verb-identity-migration")
        assertTrue(backup.isFile)
        assertEquals(originalContents, backup.readText())

        // A second migration run (e.g. a later launch) must not clobber the original backup, even
        // if the file has since been further edited by the user.
        File(home, ".bashrc").appendText("echo hello\n")
        TermuxBootstrapInstaller.migrateLegacyGuestPaths(filesDir)
        assertEquals(originalContents, backup.readText())
    }

    @Test
    fun `re-running the migration on already-migrated content is a true no-op`() {
        val filesDir = temporaryFolder.newFolder("files")
        val home = File(filesDir, "home").apply { mkdirs() }
        val bashrc = File(home, ".bashrc").apply {
            writeText("export PATH=/data/data/com.termux/files/home/.local/bin:\$PATH\n")
        }

        val first = TermuxBootstrapInstaller.migrateLegacyGuestPaths(filesDir)
        assertTrue(first.migrated)
        val afterFirstRun = bashrc.readText()

        val second = TermuxBootstrapInstaller.migrateLegacyGuestPaths(filesDir)

        assertFalse(second.migrated)
        assertEquals(afterFirstRun, bashrc.readText())
    }

    @Test
    fun `unrelated user lines survive migration byte for byte`() {
        val filesDir = temporaryFolder.newFolder("files")
        val home = File(filesDir, "home").apply { mkdirs() }
        val unrelatedAlias = "alias ll='ls -la'"
        val unrelatedExport = "export EDITOR=nano"
        val bashrc = File(home, ".bashrc").apply {
            writeText(
                "$unrelatedAlias\n" +
                    "export PATH=/data/data/com.termux/files/usr/bin:\$PATH\n" +
                    "$unrelatedExport\n"
            )
        }

        TermuxBootstrapInstaller.migrateLegacyGuestPaths(filesDir)

        val lines = bashrc.readText().lines()
        assertTrue(lines.contains(unrelatedAlias))
        assertTrue(lines.contains(unrelatedExport))
    }

    @Test
    fun `a bashrc with nothing legacy is left completely alone, no backup written`() {
        val filesDir = temporaryFolder.newFolder("files")
        val home = File(filesDir, "home").apply { mkdirs() }
        val contents = "export EDITOR=nano\n"
        File(home, ".bashrc").writeText(contents)

        val result = TermuxBootstrapInstaller.migrateLegacyGuestPaths(filesDir)

        assertFalse(result.migrated)
        assertEquals(contents, File(home, ".bashrc").readText())
        assertFalse(File(home, ".bashrc.pre-verb-identity-migration").exists())
    }

    // --- Shell integration injection (Shell Awareness P0) ---

    @Test
    fun `writeShellIntegrationScript emits OSC 7 and OSC 633 emitters under PREFIX`() {
        val filesDir = temporaryFolder.newFolder("files")

        TermuxBootstrapInstaller.writeShellIntegrationScript(filesDir)

        val script = File(filesDir, "usr/etc/verb/shell-integration.bash")
        assertTrue(script.isFile)
        val contents = script.readText()
        assertTrue(contents.contains("]7;file://"))
        assertTrue(contents.contains("]633;"))
        assertTrue(contents.contains("P;Verb=1"))
        // No DEBUG trap for v1 -- PS0/PROMPT_COMMAND chaining only.
        assertFalse(contents.contains("trap"))
    }

    @Test
    fun `writeShellIntegrationScript unconditionally overwrites -- it is never user content`() {
        val filesDir = temporaryFolder.newFolder("files")
        val script = File(filesDir, "usr/etc/verb/shell-integration.bash").apply {
            parentFile?.mkdirs()
            writeText("# stale content from a previous Verb version\n")
        }

        TermuxBootstrapInstaller.writeShellIntegrationScript(filesDir)

        assertFalse(script.readText().contains("stale content"))
        assertTrue(script.readText().contains("Verb shell integration"))
    }

    @Test
    fun `a fresh Verb-created bash_profile already sources the integration script after bashrc`() {
        val filesDir = temporaryFolder.newFolder("files")

        TermuxBootstrapInstaller.ensureLoginShellSourcesBashrc(filesDir)
        val sourced = TermuxBootstrapInstaller.ensureShellIntegrationSourced(filesDir)

        val contents = File(filesDir, "home/.bash_profile").readText()
        val bashrcIndex = contents.indexOf(".bashrc")
        val integrationIndex = contents.indexOf("shell-integration.bash")
        assertTrue(bashrcIndex >= 0 && integrationIndex >= 0)
        assertTrue("integration source line must come after the bashrc source line", integrationIndex > bashrcIndex)
        // Already present from creation -- ensureShellIntegrationSourced has nothing to add.
        assertFalse(sourced)
    }

    @Test
    fun `an existing bash_profile predating shell integration gets exactly one marked source line appended, with a backup`() {
        val filesDir = temporaryFolder.newFolder("files")
        val home = File(filesDir, "home").apply { mkdirs() }
        val original = "# my own login shell setup\nexport EDITOR=nano\n"
        File(home, ".bash_profile").writeText(original)

        val sourced = TermuxBootstrapInstaller.ensureShellIntegrationSourced(filesDir)

        assertTrue(sourced)
        val updated = File(home, ".bash_profile").readText()
        assertTrue(updated.startsWith(original))
        assertTrue(updated.contains("shell-integration.bash"))
        val backup = File(home, ".bash_profile.pre-verb-shell-integration")
        assertTrue(backup.isFile)
        assertEquals(original, backup.readText())
    }

    @Test
    fun `ensureShellIntegrationSourced is idempotent -- no duplicate line, no backup clobber`() {
        val filesDir = temporaryFolder.newFolder("files")
        val home = File(filesDir, "home").apply { mkdirs() }
        val original = "# my own login shell setup\n"
        File(home, ".bash_profile").writeText(original)

        val first = TermuxBootstrapInstaller.ensureShellIntegrationSourced(filesDir)
        val afterFirst = File(home, ".bash_profile").readText()
        val second = TermuxBootstrapInstaller.ensureShellIntegrationSourced(filesDir)

        assertTrue(first)
        assertFalse(second)
        assertEquals(afterFirst, File(home, ".bash_profile").readText())
        // The opening marker line is unique per appended block (the closing marker uses "<<<"
        // instead of ">>>", and the source line itself legitimately mentions the script path
        // twice: once in the `-f` test, once in the `.` source command).
        val occurrences = afterFirst.split("# >>> Verb shell integration >>>").size - 1
        assertEquals(1, occurrences)
    }

    @Test
    fun `ensureShellIntegrationSourced does not fabricate a bash_profile that does not exist`() {
        val filesDir = temporaryFolder.newFolder("files")

        val sourced = TermuxBootstrapInstaller.ensureShellIntegrationSourced(filesDir)

        assertFalse(sourced)
        assertFalse(File(filesDir, "home/.bash_profile").exists())
    }

    @Test
    fun `ensureGuestShellStartupCurrent wires the script and the source line together end to end`() {
        val filesDir = temporaryFolder.newFolder("files")

        TermuxBootstrapInstaller.ensureGuestShellStartupCurrent(filesDir)

        assertTrue(File(filesDir, "usr/etc/verb/shell-integration.bash").isFile)
        assertTrue(File(filesDir, "home/.bash_profile").readText().contains("shell-integration.bash"))
    }
}
