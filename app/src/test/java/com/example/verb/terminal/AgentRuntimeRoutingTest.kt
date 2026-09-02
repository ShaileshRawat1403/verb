package com.example.verb.terminal

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class AgentRuntimeRoutingTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `antigravity profile targets AGENT_RUNTIME`() {
        val profile = RuntimeProfiles.forId(RuntimeProfileId.ANTIGRAVITY)
        assertEquals(ProfileEnvironment.AGENT_RUNTIME, profile.environment)
        assertEquals(ProfileEnvironment.LOCAL_USERLAND, profile.installEnvironment)
        assertEquals("agy", profile.launchCommand)
        assertTrue(profile.installCommand.contains("1.1.22-5711547746615296"))
        assertTrue(profile.installCommand.contains("sha512sum -c -"))
        // Through the placeholder, not as a literal absolute path. The install location is still
        // pinned; what changed is that the catalog no longer names one variant's private storage,
        // which is what made this install fail on .debug and .play builds.
        assertTrue(
            profile.installCommand.contains(
                "${RuntimeProfiles.AGENT_RUNTIME_HOME_TOKEN}/.local/bin"
            )
        )
        assertTrue(profile.installCommand.startsWith("(agy_archive="))
        assertFalse(profile.installCommand.contains("| bash"))
    }

    @Test
    fun `canonical Agent Runtime PATH includes home local bin`() {
        val filesDir = temporaryFolder.newFolder("files")
        val rootfs = temporaryFolder.newFolder("rootfs")
        val workspace = temporaryFolder.newFolder("workspace")
        val manifest = AgentRuntimeManifest(
            runtimeVersion = "0.1.0",
            architecture = "aarch64",
            distro = "debian-bookworm-arm64",
            nodeVersion = "24.18.0",
            claudeVersion = "2.1.233",
            openCodeVersion = "1.18.18",
            minimumVerbVersion = "1.0.0",
            createdAt = "2026-08-16T04:28:29Z",
            requiredCommands = listOf("/bin/bash"),
            rootfsSha256 = "0".repeat(64)
        )

        File(filesDir, "usr/bin/proot").apply {
            parentFile?.mkdirs()
            createNewFile()
            setExecutable(true)
        }
        File(filesDir, QemuAgentRuntimeEnvironment.QEMU_RELATIVE_PATH).apply {
            parentFile?.mkdirs()
            createNewFile()
            setExecutable(true)
        }

        val qemuEnv = QemuAgentRuntimeEnvironment(filesDir, workspace, manifest)
        val terminalEnv = qemuEnv.resolveGuestCommand(rootfs, listOf("agy", "--version"))

        val pathArg = terminalEnv.arguments
            .filterIndexed { i, _ -> terminalEnv.arguments.getOrNull(i - 1) == "-E" }
            .firstOrNull { it.startsWith("PATH=") }

        assertEquals("PATH=/home/verb/.local/bin:/usr/local/bin:/usr/bin:/bin", pathArg)
    }

    @Test
    fun `interactive qemu shell preserves the probed PATH instead of sourcing login profile`() {
        val filesDir = temporaryFolder.newFolder("session-files")
        val rootfs = temporaryFolder.newFolder("session-rootfs")
        val workspace = temporaryFolder.newFolder("session-workspace")
        val manifest = AgentRuntimeManifest(
            runtimeVersion = "0.1.0",
            architecture = "aarch64",
            distro = "debian-bookworm-arm64",
            nodeVersion = "24.18.0",
            claudeVersion = "2.1.233",
            openCodeVersion = "1.18.18",
            minimumVerbVersion = "1.0.0",
            createdAt = "2026-08-16T04:28:29Z",
            requiredCommands = listOf("/bin/bash"),
            rootfsSha256 = "0".repeat(64)
        )

        File(filesDir, "usr/bin/proot").apply {
            parentFile?.mkdirs()
            createNewFile()
            setExecutable(true)
        }
        File(filesDir, QemuAgentRuntimeEnvironment.QEMU_RELATIVE_PATH).apply {
            parentFile?.mkdirs()
            createNewFile()
            setExecutable(true)
        }

        val argv = QemuAgentRuntimeEnvironment(filesDir, workspace, manifest)
            .resolve(rootfs)
            .arguments
            .toList()

        assertEquals("/bin/bash", argv.last())
        assertFalse(argv.contains("--login"))
        assertTrue(argv.contains("PATH=/home/verb/.local/bin:/usr/local/bin:/usr/bin:/bin"))
    }

    @Test
    fun `agent sign in detector checks correct home for AGENT_RUNTIME profiles`() {
        val filesDir = temporaryFolder.newFolder("files")
        val detector = AgentSignInDetector(filesDir)

        val localProfile = RuntimeProfile(
            id = RuntimeProfileId.CLAUDE_CODE,
            displayName = "Claude",
            packages = emptyList(),
            requirements = emptyList(),
            signedInMarkers = listOf(".claude.json"),
            environment = ProfileEnvironment.LOCAL_USERLAND
        )

        val agentRuntimeProfile = RuntimeProfile(
            id = RuntimeProfileId.ANTIGRAVITY,
            displayName = "Antigravity",
            packages = emptyList(),
            requirements = emptyList(),
            signedInMarkers = listOf(".gemini/session.json"),
            environment = ProfileEnvironment.AGENT_RUNTIME
        )

        // Local home marker
        File(filesDir, "home/.claude.json").apply { parentFile?.mkdirs(); createNewFile() }
        assertEquals(AgentSignInState.SIGNED_IN, detector.stateFor(localProfile))

        // Agent runtime home marker
        val agentHome = AgentRuntimePaths(filesDir).agentHome("default")
        File(agentHome, ".gemini/session.json").apply { parentFile?.mkdirs(); createNewFile() }
        assertEquals(AgentSignInState.SIGNED_IN, detector.stateFor(agentRuntimeProfile))

        // When markers are empty, UNKNOWN is reported
        val noMarkerProfile = RuntimeProfiles.forId(RuntimeProfileId.ANTIGRAVITY)
        assertEquals(AgentSignInState.UNKNOWN, detector.stateFor(noMarkerProfile))
    }
}
