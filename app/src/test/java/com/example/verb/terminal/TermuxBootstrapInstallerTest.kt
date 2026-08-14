package com.example.verb.terminal

import java.io.File
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
}
