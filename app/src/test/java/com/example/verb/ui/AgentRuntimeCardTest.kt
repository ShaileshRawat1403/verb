package com.example.verb.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import com.example.verb.terminal.AgentArtifactState
import com.example.verb.terminal.AgentCompatibilityState
import com.example.verb.terminal.AgentRuntimeInstaller
import com.example.verb.terminal.AgentRuntimeManifest
import com.example.verb.terminal.AgentRuntimeStatus
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class AgentRuntimeCardTest {

    @get:Rule
    val composeTestRule = createComposeRule()

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

    private fun installed(state: AgentCompatibilityState) = AgentRuntimeStatus(
        artifact = AgentArtifactState.INSTALLED,
        compatibility = state,
        runtime = AgentRuntimeInstaller.InstalledRuntime(manifest(), File("/tmp/rootfs"))
    )

    private fun show(status: AgentRuntimeStatus, message: String? = null) {
        composeTestRule.setContent {
            AgentRuntimeCard(
                status = status,
                importing = false,
                message = message,
                archiveName = null,
                checksumName = null,
                manifestName = null,
                onPickArchive = {},
                onPickChecksum = {},
                onPickManifest = {},
                onImport = {},
                onOpen = {},
                onCheckCompatibility = {},
                onReturnToVerb = {}
            )
        }
    }

    @Test
    fun `open is disabled when the runtime has not been checked`() {
        show(installed(AgentCompatibilityState.NOT_CHECKED))
        composeTestRule.onNodeWithTag("agent_runtime_open").assertIsNotEnabled()
    }

    @Test
    fun `open is disabled while a check is in flight`() {
        show(installed(AgentCompatibilityState.CHECKING))
        composeTestRule.onNodeWithTag("agent_runtime_open").assertIsNotEnabled()
    }

    @Test
    fun `open is disabled when the runtime is incompatible`() {
        show(installed(AgentCompatibilityState.INCOMPATIBLE))
        composeTestRule.onNodeWithTag("agent_runtime_open").assertIsNotEnabled()
    }

    @Test
    fun `open is disabled when the check failed`() {
        show(installed(AgentCompatibilityState.CHECK_FAILED))
        composeTestRule.onNodeWithTag("agent_runtime_open").assertIsNotEnabled()
    }

    @Test
    fun `open is disabled when the check timed out`() {
        show(installed(AgentCompatibilityState.CHECK_TIMED_OUT))
        composeTestRule.onNodeWithTag("agent_runtime_open").assertIsNotEnabled()
    }

    @Test
    fun `open is enabled only for COMPATIBLE`() {
        show(installed(AgentCompatibilityState.COMPATIBLE))

        composeTestRule.onNodeWithTag("agent_runtime_open").assertIsEnabled()
    }

    @Test
    fun `an incompatible runtime still reports as installed and keeps the normal terminal available`() {
        show(installed(AgentCompatibilityState.INCOMPATIBLE))

        composeTestRule.onNodeWithTag("agent_runtime_artifact_state").assertIsDisplayed()
        composeTestRule.onNodeWithText("Compatibility: cannot run on this device").assertIsDisplayed()
        // The rootfs is never removed and the normal terminal is unaffected, so this stays usable.
        composeTestRule.onNodeWithTag("agent_runtime_return").assertIsEnabled()
    }

    @Test
    fun `retry check is offered whenever a check is not already running`() {
        show(installed(AgentCompatibilityState.INCOMPATIBLE))
        composeTestRule.onNodeWithTag("agent_runtime_check").assertIsEnabled()
    }

    @Test
    fun `retry check is disabled while a check is in flight`() {
        show(installed(AgentCompatibilityState.CHECKING))

        composeTestRule.onNodeWithTag("agent_runtime_check").assertIsNotEnabled()
    }

    @Test
    fun `a not-installed runtime offers neither open nor check`() {
        show(AgentRuntimeStatus())

        composeTestRule.onNodeWithText("Not installed").assertIsDisplayed()
        composeTestRule.onNodeWithTag("agent_runtime_open").assertDoesNotExist()
        composeTestRule.onNodeWithTag("agent_runtime_check").assertDoesNotExist()
    }
}
