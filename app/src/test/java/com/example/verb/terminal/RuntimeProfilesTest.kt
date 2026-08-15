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
            setExecutable(true)
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
            setExecutable(true)
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

    @Test
    fun `a resolved but non-executable command is not ready`() {
        val filesDir = temporaryFolder.newFolder("files")
        File(filesDir, "usr/bin/git").apply {
            parentFile?.mkdirs()
            createNewFile()
            setExecutable(false)
        }

        val report = RuntimeCapabilityDetector(filesDir).inspect(RuntimeProfiles.forId(RuntimeProfileId.CORE))

        assertFalse(report.isReady)
        assertTrue(report.nonExecutableCommands.contains("git"))
        assertFalse(report.missingCommands.contains("git"))
    }

    // Probe execution mechanics (timeout, bounded output, exit-code interpretation, refusal of
    // unregistered probes) moved to GuestCommandRunnerTest.kt, which covers GuestCommandRunner
    // directly. These tests cover the wiring: RuntimeCapabilityDetector.inspect() only reports a
    // probed profile (CODEX/CLAUDE_CODE/GEMINI_CLI) ready when GuestCommandRunner actually says so.

    private fun setUpGuestBootstrap(filesDir: File) {
        File(filesDir, "usr/bin/login").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(filesDir, "usr/lib/libtermux-exec.so").apply { parentFile?.mkdirs(); createNewFile() }
        File(filesDir, "home").mkdirs()
        File(filesDir, "usr/bin/proot").apply {
            parentFile?.mkdirs()
            writeText("#!/bin/sh\nwhile [ \"\$#\" -gt 2 ]; do shift; done\nexec \"\$1\" \"\$2\"\n")
            setExecutable(true)
        }
    }

    @Test
    fun `a probed profile is not ready without an installed guest userland`() {
        val filesDir = temporaryFolder.newFolder("files")
        // No bootstrap at all -- GuestCommandRunner reports GUEST_UNAVAILABLE for codex.

        val report = RuntimeCapabilityDetector(filesDir).inspect(RuntimeProfiles.forId(RuntimeProfileId.CODEX))

        assertFalse(report.isReady)
        assertTrue(report.missingCommands.contains("codex"))
    }

    @Test
    fun `a probed profile is ready only once the guest probe actually exits 0`() {
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        File(filesDir, "usr/bin/codex").apply {
            parentFile?.mkdirs()
            writeText("#!/bin/sh\necho \"codex 1.0.0\"\nexit 0\n")
            setExecutable(true)
        }

        val report = RuntimeCapabilityDetector(filesDir).inspect(RuntimeProfiles.forId(RuntimeProfileId.CODEX))

        assertTrue(report.isReady)
    }

    @Test
    fun `a probed profile is not ready when the guest probe fails`() {
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        File(filesDir, "usr/bin/codex").apply {
            parentFile?.mkdirs()
            writeText("#!/bin/sh\necho boom\nexit 1\n")
            setExecutable(true)
        }

        val report = RuntimeCapabilityDetector(filesDir).inspect(RuntimeProfiles.forId(RuntimeProfileId.CODEX))

        assertFalse(report.isReady)
        assertTrue(report.unverifiedCommands.contains("codex"))
    }

    @Test
    fun `a probed profile that times out is reported distinctly from a failed probe`() {
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        File(filesDir, "usr/bin/codex").apply {
            parentFile?.mkdirs()
            writeText("#!/bin/sh\nread _unused\necho done\n")
            setExecutable(true)
        }
        val requirement = RuntimeRequirement("codex", "", versionProbeArgs = listOf("--version"))
        val guestCommandRunner = GuestCommandRunner(filesDir)

        val result = guestCommandRunner.probe(requirement, timeoutMs = 300)

        assertEquals(GuestCommandRunner.Outcome.TIMEOUT, result.outcome)
    }

    @Test
    fun `stageFor maps each command to its readiness stage`() {
        val filesDir = temporaryFolder.newFolder("files")
        // curl's package is recorded installed, but its command file is never written -> INSTALLED_UNRESOLVED.
        File(filesDir, "usr/var/lib/dpkg/status").apply {
            parentFile?.mkdirs()
            writeText(
                """
                Package: curl
                Status: install ok installed
                Version: 8.9.1-1
                """.trimIndent()
            )
        }
        // bash's command file exists but lacks the executable bit -> RESOLVED_NOT_EXECUTABLE.
        File(filesDir, "usr/bin/bash").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(false) }
        // git's package is never recorded as installed at all -> MISSING.

        val core = RuntimeProfiles.forId(RuntimeProfileId.CORE)
        val report = RuntimeCapabilityDetector(filesDir).inspect(core)

        assertEquals(ReadinessStage.MISSING, report.stageFor(core.requirements.first { it.command == "git" }))
        assertEquals(
            ReadinessStage.INSTALLED_UNRESOLVED,
            report.stageFor(core.requirements.first { it.command == "curl" })
        )
        assertEquals(
            ReadinessStage.RESOLVED_NOT_EXECUTABLE,
            report.stageFor(core.requirements.first { it.command == "bash" })
        )
    }
}
