package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * The probe's contract: what it runs, how it is bounded, and how outcomes map onto product state.
 *
 * The command and argv shape are asserted against [AgentRuntimeEnvironment.resolveGuestCommand] --
 * the same construction site the interactive session uses -- so a probe can never end up testing a
 * different environment from the one the user is about to open.
 */
class AgentRuntimeCompatibilityProbeTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun manifest() = AgentRuntimeManifest(
        runtimeVersion = "0.1.0",
        architecture = "aarch64",
        rootfsSha256 = "0".repeat(64),
        distro = "debian-bookworm-arm64",
        nodeVersion = "20.0.0",
        claudeVersion = "1.0.0",
        openCodeVersion = "1.0.0",
        minimumVerbVersion = "0.1.0",
        createdAt = "2026-01-01T00:00:00Z"
    )

    /** A filesDir with an executable stand-in for proot, so argv construction can be inspected. */
    private fun filesDirWithProot(): File {
        val filesDir = temporaryFolder.newFolder("files")
        File(filesDir, "usr/bin").mkdirs()
        File(filesDir, "usr/bin/proot").apply { writeText("#!/bin/sh\n"); setExecutable(true) }
        return filesDir
    }

    @Test
    fun `the probe runs bash --version, never a login or interactive shell`() {
        val filesDir = filesDirWithProot()
        val rootfs = temporaryFolder.newFolder("rootfs")
        val workspace = temporaryFolder.newFolder("workspace")

        val argv = AgentRuntimeEnvironment(filesDir, workspace, manifest())
            .resolveGuestCommand(rootfs, listOf("/bin/bash", "--version"))
            .arguments.toList()

        assertEquals(listOf("/bin/bash", "--version"), argv.takeLast(2))
        // A login shell would source guest startup files; app-management code must never do that.
        assert(!argv.contains("--login")) { "probe must not run a login shell" }
        assert(!argv.contains("-i")) { "probe must not run an interactive shell" }
        // argv-only: nothing is ever handed to a shell for interpretation.
        assert(!argv.contains("-c")) { "probe must not be routed through sh -c" }
    }

    @Test
    fun `the probe environment matches the interactive session's binds and loader isolation`() {
        val filesDir = filesDirWithProot()
        val rootfs = temporaryFolder.newFolder("rootfs")
        val workspace = temporaryFolder.newFolder("workspace")
        val environment = AgentRuntimeEnvironment(filesDir, workspace, manifest())

        val probeArgv = environment.resolveGuestCommand(rootfs, listOf("/bin/bash", "--version")).arguments.toList()
        val sessionArgv = environment.resolve(rootfs).arguments.toList()

        // Everything before the guest command is identical: same rootfs, binds, -w and guest env.
        assertEquals(sessionArgv.dropLast(1), probeArgv.dropLast(2))
        assertEquals(listOf("/bin/bash"), sessionArgv.takeLast(1))
        assert(!sessionArgv.contains("--login")) {
            "interactive shell must preserve the same PATH the probe verified"
        }
        assert(probeArgv.contains("LD_LIBRARY_PATH=")) { "guest loader isolation must be preserved" }
        assert(probeArgv.contains("LD_PRELOAD=")) { "guest preload isolation must be preserved" }
        assert(probeArgv.contains("/workspace")) { "workspace bind must be present" }
    }

    @Test
    fun `the host environment keeps PROOT_TMP_DIR and the bionic loader path for proot itself`() {
        val filesDir = filesDirWithProot()
        val rootfs = temporaryFolder.newFolder("rootfs")
        val workspace = temporaryFolder.newFolder("workspace")

        val variables = AgentRuntimeEnvironment(filesDir, workspace, manifest())
            .resolveGuestCommand(rootfs, listOf("/bin/bash", "--version"))
            .variables.toList()

        assert(variables.any { it.startsWith("PROOT_TMP_DIR=") }) { "PROOT_TMP_DIR must reach proot" }
        assert(variables.any { it == "LD_LIBRARY_PATH=${filesDir.absolutePath}/usr/lib" }) {
            "the bionic proot executable still needs its own host library path"
        }
    }

    @Test
    fun `the probe is bounded to at most five seconds`() {
        assertEquals(5_000L, AgentRuntimeCompatibilityProbe.TIMEOUT_MS)
        assert(AgentRuntimeCompatibilityProbe.TIMEOUT_MS <= 5_000L)
    }

    /**
     * A rootfs that cannot produce a runnable proot invocation must resolve to CHECK_FAILED --
     * "nothing could be concluded" -- rather than silently reading as compatible.
     */
    @Test
    fun `an unusable runtime reports CHECK_FAILED rather than compatible`() {
        val filesDir = temporaryFolder.newFolder("files-no-proot")
        val runtime = AgentRuntimeInstaller.InstalledRuntime(
            manifest = manifest(),
            rootfs = temporaryFolder.newFolder("rootfs-unusable")
        )

        val state = AgentRuntimeCompatibilityProbe(filesDir).check(runtime)

        assertEquals(AgentCompatibilityState.CHECK_FAILED, state)
    }

    @Test
    fun `a probe workspace is app-private and never the user's selected project`() {
        val filesDir = filesDirWithProot()
        val runtime = AgentRuntimeInstaller.InstalledRuntime(
            manifest = manifest(),
            rootfs = temporaryFolder.newFolder("rootfs-ws")
        )

        AgentRuntimeCompatibilityProbe(filesDir).check(runtime)

        val probeWorkspace = File(AgentRuntimePaths(filesDir).root, "compat-probe")
        assert(probeWorkspace.isDirectory) { "probe must use its own app-private workspace" }
    }

    @Test
    fun `shell admission probe can succeed without weakening the agent probe contract`() {
        val filesDir = filesDirWithProot()
        // The fake proot exits successfully; QEMU still has to exist for the real environment
        // resolver to admit the invocation.
        File(filesDir, "usr/bin/proot").apply {
            writeText("#!/bin/sh\nexit 0\n")
            setExecutable(true)
        }
        File(filesDir, QemuAgentRuntimeEnvironment.QEMU_RELATIVE_PATH).apply {
            parentFile?.mkdirs()
            writeText("qemu stand-in")
            setExecutable(true)
        }
        val runtime = AgentRuntimeInstaller.InstalledRuntime(
            manifest = manifest(),
            rootfs = temporaryFolder.newFolder("rootfs-shell-admission")
        )

        assertEquals(
            AgentCompatibilityState.COMPATIBLE,
            AgentRuntimeCompatibilityProbe(filesDir).checkShellForProfileInstallation(runtime)
        )
    }
}
