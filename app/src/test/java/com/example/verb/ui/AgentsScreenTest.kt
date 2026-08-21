package com.example.verb.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.example.verb.terminal.RuntimeProfileId
import com.example.verb.terminal.RuntimeProfileReport
import com.example.verb.terminal.RuntimeProfiles
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class AgentsScreenTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private fun report(
        id: RuntimeProfileId,
        missingCommands: List<String> = emptyList(),
        incompatible: List<String> = emptyList()
    ) = RuntimeProfileReport(
        profile = RuntimeProfiles.forId(id),
        missingPackages = emptyList(),
        missingCommands = missingCommands,
        incompatibleCommands = incompatible
    )

    private fun show(
        reports: List<RuntimeProfileReport>,
        keys: List<AgentKeyStatus> = emptyList(),
        onLaunch: (String) -> Unit = {},
        onEditKeys: () -> Unit = {}
    ) {
        composeTestRule.setContent {
            AgentsScreen(
                reports = reports,
                keyStatus = keys,
                onLaunch = onLaunch,
                onInstall = {},
                onEditKeys = onEditKeys
            )
        }
    }

    /** Toolchains are setup, agents are the product; this surface shows only the latter. */
    @Test
    fun `only agents appear, never toolchains`() {
        show(listOf(report(RuntimeProfileId.CODEX), report(RuntimeProfileId.CORE)))

        composeTestRule.onNodeWithTag("agent_codex").assertIsDisplayed()
        composeTestRule.onNodeWithTag("agent_core").assertDoesNotExist()
    }

    @Test
    fun `a ready agent offers to open and reports the command it will run`() {
        show(listOf(report(RuntimeProfileId.CODEX)))

        composeTestRule.onNodeWithTag("agent_open_codex").assertIsEnabled()
        // The whole command is visible, flags included, so the button is not magic and Verb is not
        // quietly running something other than what the card says.
        composeTestRule.onNodeWithText("codex --disable apps").assertIsDisplayed()
    }

    @Test
    fun `opening an agent hands back the command rather than launching invisibly`() {
        var launched: String? = null
        show(listOf(report(RuntimeProfileId.CODEX)), onLaunch = { launched = it })

        composeTestRule.onNodeWithTag("agent_open_codex").performClick()

        assertEquals("codex --disable apps", launched)
    }

    @Test
    fun `an uninstalled agent offers install, not open`() {
        show(listOf(report(RuntimeProfileId.OPENCODE, missingCommands = listOf("opencode"))))

        composeTestRule.onNodeWithTag("agent_install_opencode").assertIsDisplayed()
        composeTestRule.onNodeWithTag("agent_open_opencode").assertDoesNotExist()
    }

    /** An agent that can never run must not offer an action that cannot succeed. */
    @Test
    fun `an unsatisfiable agent offers neither open nor install`() {
        show(listOf(report(RuntimeProfileId.HERMES, incompatible = listOf("python3.13"))))

        composeTestRule.onNodeWithTag("agent_open_hermes").assertDoesNotExist()
        composeTestRule.onNodeWithTag("agent_install_hermes").assertDoesNotExist()
        composeTestRule.onNodeWithText("Cannot run on this device. No install will resolve this.")
            .assertIsDisplayed()
    }

    /** Presence only: a screenshot of this screen must never be able to leak a key. */
    @Test
    fun `keys show presence and never a value`() {
        show(
            reports = listOf(report(RuntimeProfileId.CODEX)),
            keys = listOf(
                AgentKeyStatus("ANTHROPIC_API_KEY", isSet = true),
                AgentKeyStatus("DEEPSEEK_API_KEY", isSet = false)
            )
        )

        composeTestRule.onNodeWithText("ANTHROPIC_API_KEY").assertIsDisplayed()
        composeTestRule.onNodeWithText("set").assertIsDisplayed()
        composeTestRule.onNodeWithText("not set").assertIsDisplayed()
    }

    @Test
    fun `editing keys is handed to the terminal, not done in the UI`() {
        var edited = false
        show(listOf(report(RuntimeProfileId.CODEX)), onEditKeys = { edited = true })

        composeTestRule.onNodeWithTag("agent_keys_edit").performClick()

        assert(edited)
    }

    @Test
    fun `every catalog agent declares the command it launches`() {
        val agents = RuntimeProfiles.all.filter { it.isAgent }

        assert(agents.isNotEmpty())
        agents.forEach { assert(!it.launchCommand.isNullOrBlank()) }
    }
}
