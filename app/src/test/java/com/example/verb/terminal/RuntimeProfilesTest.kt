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

    /**
     * A stock userland has the 3.14 `python` from the main repository and no `python3.13`, so
     * Hermes reports the versioned interpreter as missing -- work an install can do -- rather than
     * as an incompatible version, which no install could ever fix.
     */
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
        assertFalse(report.isReady)
        assertTrue(report.missingPackages.contains("python-cryptography"))
        assertTrue(report.missingCommands.contains("hermes"))
        // The unversioned 3.14 interpreter no longer blocks Hermes, so this stays installable.
        assertFalse(report.isUnsatisfiable)
        assertTrue(report.isInstallable)
    }

    /**
     * A compatible interpreter is necessary but not sufficient. Hermes reported Ready for months on
     * the strength of `python` existing while the agent itself was never installed, which is
     * exactly the "installed does not mean runnable" failure this catalog is meant to prevent.
     */
    @Test
    fun `Hermes is not ready on the strength of the interpreter alone`() {
        val filesDir = temporaryFolder.newFolder("files")
        File(filesDir, "usr/var/lib/dpkg/status").apply {
            parentFile?.mkdirs()
            writeText("""
                Package: python
                Status: install ok installed
                Version: 3.14.6
            """.trimIndent())
        }
        File(filesDir, "usr/bin/python").apply {
            parentFile?.mkdirs()
            createNewFile()
            setExecutable(true)
        }

        val report = RuntimeCapabilityDetector(filesDir).inspect(RuntimeProfiles.forId(RuntimeProfileId.HERMES))

        assertFalse("the agent itself is still missing", report.isReady)
        assertTrue(report.missingCommands.contains("hermes"))
    }

    @Test
    fun `Hermes pulls in native toolchain and cryptography packages`() {
        val hermes = RuntimeProfiles.forId(RuntimeProfileId.HERMES)

        assertEquals(listOf(RuntimeProfileId.NATIVE), hermes.prerequisiteProfiles)
        assertEquals(
            listOf("python", "python-pip", "python-cryptography", "python-psutil", "openssl", "libffi"),
            hermes.packages
        )
        assertEquals(
            listOf(RuntimeProfileId.NATIVE, RuntimeProfileId.HERMES),
            RuntimeProfiles.installPlan(RuntimeProfileId.HERMES) { false }.map { it.id }
        )
        assertTrue(hermes.installCommand.contains("hermes-agent==0.15.2"))
        // AgentWrapperBootstrap owns launchers in Verb's private libexec directory. A profile
        // installer must never overwrite package-manager or user commands in $PREFIX/bin.
        assertFalse(hermes.installCommand.contains("\$PREFIX/bin/"))
        assertFalse(hermes.installCommand.contains("chmod +x"))
    }

    /** The repository index must be refreshed after the key-bearing package lands, or nothing new resolves. */
    @Test
    fun `the extra repository profile updates the index after installing itself`() {
        val command = RuntimeProfiles.forId(RuntimeProfileId.TUR).installCommand

        assertTrue(command.startsWith("apt-get update &&"))
        assertTrue(command.contains("install -y --no-install-recommends tur-repo"))
        assertTrue(command.endsWith("&& apt-get update"))
    }

    @Test
    fun `profile exposes an install plan using catalog packages`() {
        val command = RuntimeProfiles.forId(RuntimeProfileId.PYTHON).installCommand

        assertEquals("apt-get update && apt-get install -y --no-install-recommends python python-pip", command)
    }

    @Test
    fun `agent profiles use their vendor npm installers and require JavaScript`() {
        val codex = RuntimeProfiles.forId(RuntimeProfileId.CODEX)
        val claude = RuntimeProfiles.forId(RuntimeProfileId.CLAUDE_CODE)
        val gemini = RuntimeProfiles.forId(RuntimeProfileId.GEMINI_CLI)

        // Codex's launcher is only half of it: the real binary ships in an optional dependency npm
        // skips here, so the install resolves that too. Asserted in detail by
        // AgentWrapperBootstrapTest.
        assertTrue(codex.installCommand.startsWith("npm install -g @openai/codex"))
        // Claude and OpenCode publish no android build; their musl builds run here once the
        // interpreter exists, so they install the platform package directly. Asserted in detail by
        // MuslAgentSupportTest -- here we only pin that they are still npm-installed agent CLIs.
        assertTrue(claude.installCommand.contains("@anthropic-ai/claude-code-linux-arm64-musl"))
        assertEquals("npm install -g @google/gemini-cli", gemini.installCommand)
        // Codex additionally needs the emulator, because its only aarch64 build is static and
        // proot refuses to exec a static binary.
        assertEquals(
            listOf(RuntimeProfileId.JAVASCRIPT, RuntimeProfileId.AGENT_EMULATOR),
            codex.prerequisiteProfiles
        )
        assertEquals(listOf(RuntimeProfileId.JAVASCRIPT), claude.prerequisiteProfiles)
        assertEquals(listOf(RuntimeProfileId.JAVASCRIPT), gemini.prerequisiteProfiles)

        val opencode = RuntimeProfiles.forId(RuntimeProfileId.OPENCODE)
        assertTrue(opencode.installCommand.contains("opencode-linux-arm64-musl"))
        assertEquals(listOf(RuntimeProfileId.JAVASCRIPT), opencode.prerequisiteProfiles)

        val antigravity = RuntimeProfiles.forId(RuntimeProfileId.ANTIGRAVITY)
        assertEquals("Antigravity", antigravity.displayName)
        assertEquals("agy", antigravity.launchCommand)
        assertEquals(ProfileEnvironment.AGENT_RUNTIME, antigravity.environment)
        assertEquals(ProfileEnvironment.LOCAL_USERLAND, antigravity.installEnvironment)
        assertTrue(antigravity.installCommand.contains("1.1.22-5711547746615296"))
        assertTrue(antigravity.installCommand.contains("sha512sum -c -"))
        assertTrue(antigravity.installCommand.contains("tar -xzf"))
        assertTrue(antigravity.installCommand.contains("trap 'rm -rf"))
        assertFalse(antigravity.installCommand.contains("| bash"))
        assertEquals(listOf(RuntimeRequirement("agy", "", versionProbeArgs = listOf("--version"), probeTimeoutMs = 15_000L)), antigravity.requirements)
        assertTrue(antigravity.signedInMarkers.isEmpty())
        assertEquals(
            listOf(AgentBinaryCandidate("\$HOME/.local/bin/agy", AgentBinaryAbi.DETECT)),
            antigravity.binaryCandidates
        )
        assertTrue(antigravity.isAgent)
        assertEquals(listOf(RuntimeProfileId.ANTIGRAVITY), RuntimeProfiles.installPlan(RuntimeProfileId.ANTIGRAVITY) { false }.map { it.id })
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

    /**
     * The defect this pins: an install command carrying a build-time absolute path under
     * `/data/data/<applicationId>/` is wrong on every variant except the one it was typed for.
     * The debug build type adds `.debug` and the Play flavour adds `.play`, so such a path names a
     * different package's private directory -- which Android refuses to write to.
     */
    @Test
    fun `no install command hardcodes an app-private absolute path`() {
        val offenders = RuntimeProfiles.all
            .filter { it.installCommand.contains("/data/data/") }
            .map { it.displayName }

        assertEquals(emptyList<String>(), offenders)
    }

    /**
     * Antigravity installs into the Agent Runtime home while its downloader runs in the local
     * userland, so its target cannot come from `$HOME`. It must therefore go through the
     * placeholder -- the only mechanism that lets the running app supply its own path.
     */
    @Test
    fun `antigravity install targets the agent runtime home through the placeholder`() {
        val agy = RuntimeProfiles.forId(RuntimeProfileId.ANTIGRAVITY)

        assertTrue(agy.installCommand.contains(RuntimeProfiles.AGENT_RUNTIME_HOME_TOKEN))
        assertEquals(ProfileEnvironment.AGENT_RUNTIME, agy.environment)
        assertEquals(ProfileEnvironment.LOCAL_USERLAND, agy.installEnvironment)
    }

    /**
     * Antigravity is the only admitted agent that runs inside the Agent Runtime.
     *
     * Two pieces of behaviour rest on this and would go wrong quietly if it changed. Resuming a
     * local-userland agent first calls `deactivateAgentRuntime()` and restarts, because a resume is
     * dispatched by typing into whatever the terminal is currently running -- without it,
     * `claude --resume <id>` was observed being typed into Antigravity's chat box on a Vivo I2202.
     * And Verb claims the foreground for Antigravity by hand, because it is the one admitted agent
     * with no session coordinator to do it.
     *
     * A second AGENT_RUNTIME agent, or Claude moving into the Agent Runtime, invalidates both. This
     * is the test that should fail first when that happens.
     */
    @Test
    fun `Antigravity is the only admitted agent that runs in the Agent Runtime`() {
        val admitted = mapOf(
            RuntimeProfileId.CLAUDE_CODE to ProfileEnvironment.LOCAL_USERLAND,
            RuntimeProfileId.CODEX to ProfileEnvironment.LOCAL_USERLAND,
            RuntimeProfileId.OPENCODE to ProfileEnvironment.LOCAL_USERLAND,
            RuntimeProfileId.HERMES to ProfileEnvironment.LOCAL_USERLAND,
            RuntimeProfileId.ANTIGRAVITY to ProfileEnvironment.AGENT_RUNTIME
        )

        admitted.forEach { (id, expected) ->
            val profile = RuntimeProfiles.all.first { it.id == id }
            assertEquals("$id runs in the wrong environment", expected, profile.environment)
        }
    }

    /**
     * Substitution is asserted on the same regex the ViewModel refuses on, so a token added to the
     * catalog without a matching resolver fails here rather than on a user's device.
     */
    @Test
    fun `placeholder detector matches an unresolved token and not a resolved path`() {
        val unresolved = "install -m 0755 x ${RuntimeProfiles.AGENT_RUNTIME_HOME_TOKEN}/.local/bin/agy"
        val resolved = unresolved.replace(
            RuntimeProfiles.AGENT_RUNTIME_HOME_TOKEN,
            "/data/user/0/com.aistudio.verb.app.debug/files/agent-runtime/homes/default"
        )

        assertTrue(RuntimeProfiles.UNRESOLVED_PLACEHOLDER.containsMatchIn(unresolved))
        assertFalse(RuntimeProfiles.UNRESOLVED_PLACEHOLDER.containsMatchIn(resolved))
    }
}
