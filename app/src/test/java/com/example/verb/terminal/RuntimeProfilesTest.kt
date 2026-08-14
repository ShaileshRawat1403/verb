package com.example.verb.terminal

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class RuntimeProfilesTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `reports missing commands and packages`() {
        val filesDir = temporaryFolder.newFolder("files")
        val status = File(filesDir, "usr/var/lib/dpkg/status").apply {
            parentFile?.mkdirs()
            writeText("""
                Package: python
                Status: install ok installed
                Version: 3.14.6-1
            """.trimIndent())
        }
        File(filesDir, "usr/bin/python").apply {
            parentFile?.mkdirs()
            createNewFile()
        }

        val report = RuntimeCapabilityDetector(filesDir).inspect(RuntimeProfiles.forId(RuntimeProfileId.HERMES))

        assertTrue(status.isFile)
        assertTrue(report.isReady.not())
        assertTrue(report.incompatibleCommands.contains("python"))
        assertEquals(emptyList<String>(), report.missingPackages)
        assertEquals(emptyList<String>(), report.missingCommands)
    }

    @Test
    fun `accepts a supported Hermes Python version`() {
        val filesDir = temporaryFolder.newFolder("files")
        File(filesDir, "usr/var/lib/dpkg/status").apply {
            parentFile?.mkdirs()
            writeText("""
                Package: python
                Status: install ok installed
                Version: 3.13.9-1
            """.trimIndent())
        }
        File(filesDir, "usr/bin/python").apply {
            parentFile?.mkdirs()
            createNewFile()
        }

        val report = RuntimeCapabilityDetector(filesDir).inspect(RuntimeProfiles.forId(RuntimeProfileId.HERMES))

        assertTrue(report.isReady)
        assertFalse(report.incompatibleCommands.contains("python"))
    }

    @Test
    fun `profile exposes an install plan using catalog packages`() {
        val command = RuntimeProfiles.forId(RuntimeProfileId.PYTHON).installCommand

        assertEquals("apt-get update && apt-get install -y --no-install-recommends python", command)
    }

    @Test
    fun `agent profiles use their vendor npm installers and require JavaScript`() {
        val codex = RuntimeProfiles.forId(RuntimeProfileId.CODEX)
        val claude = RuntimeProfiles.forId(RuntimeProfileId.CLAUDE_CODE)
        val gemini = RuntimeProfiles.forId(RuntimeProfileId.GEMINI_CLI)

        assertEquals("npm install -g @openai/codex", codex.installCommand)
        assertEquals("npm install -g @anthropic-ai/claude-code", claude.installCommand)
        assertEquals("npm install -g @google/gemini-cli", gemini.installCommand)
        assertEquals(listOf(RuntimeProfileId.JAVASCRIPT), codex.prerequisiteProfiles)
        assertEquals(listOf(RuntimeProfileId.JAVASCRIPT), claude.prerequisiteProfiles)
        assertEquals(listOf(RuntimeProfileId.JAVASCRIPT), gemini.prerequisiteProfiles)
    }
}
