package com.example.verb.terminal

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * The whole point of the model: "the artifact is on disk" and "the runtime executes here" are
 * different claims. On the validation device the first was true and the second false, and Verb
 * offered a launch button anyway.
 */
class AgentRuntimeStatusTest {

    private fun testManifest() = AgentRuntimeManifest(
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

    private fun installed(state: AgentCompatibilityState) = AgentRuntimeStatus(
        artifact = AgentArtifactState.INSTALLED,
        compatibility = state,
        runtime = AgentRuntimeInstaller.InstalledRuntime(
            manifest = testManifest(),
            rootfs = File("/tmp/rootfs")
        )
    )

    @Test
    fun `installed does not imply compatible`() {
        val status = installed(AgentCompatibilityState.NOT_CHECKED)

        assertTrue(status.isInstalled)
        assertFalse(status.canOpen)
    }

    @Test
    fun `open is refused for every state except COMPATIBLE`() {
        val refused = listOf(
            AgentCompatibilityState.NOT_CHECKED,
            AgentCompatibilityState.CHECKING,
            AgentCompatibilityState.INCOMPATIBLE,
            AgentCompatibilityState.CHECK_FAILED,
            AgentCompatibilityState.CHECK_TIMED_OUT
        )

        refused.forEach { state ->
            assertFalse("canOpen must be false for $state", installed(state).canOpen)
        }
    }

    @Test
    fun `open is allowed only for COMPATIBLE`() {
        assertTrue(installed(AgentCompatibilityState.COMPATIBLE).canOpen)
    }

    @Test
    fun `a missing artifact can never be opened or checked`() {
        val status = AgentRuntimeStatus()

        assertFalse(status.isInstalled)
        assertFalse(status.canOpen)
        assertFalse(status.canCheck)
    }

    /** Guards against a check being retriggered into a loop while one is already running. */
    @Test
    fun `a check cannot be started while one is already in flight`() {
        assertFalse(installed(AgentCompatibilityState.CHECKING).canCheck)
        assertTrue(installed(AgentCompatibilityState.NOT_CHECKED).canCheck)
        assertTrue(installed(AgentCompatibilityState.INCOMPATIBLE).canCheck)
    }
}
