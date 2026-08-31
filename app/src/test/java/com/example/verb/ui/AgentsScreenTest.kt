package com.example.verb.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.example.verb.terminal.AgentSignInState
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

    @Test
    fun `unverified runtime profiles never become catalog cards`() {
        show(
            listOf(
                report(RuntimeProfileId.GEMINI_CLI),
                report(RuntimeProfileId.DEEPSEEK_HARNESS)
            )
        )

        composeTestRule.onNodeWithTag("agent_gemini_cli").assertDoesNotExist()
        composeTestRule.onNodeWithTag("agent_deepseek_harness").assertDoesNotExist()
    }

    @Test
    fun `Hermes Agent appears as verified agent catalog card`() {
        show(listOf(report(RuntimeProfileId.HERMES)))
        composeTestRule.onNodeWithTag("agent_hermes").assertIsDisplayed()
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
    fun `saved credential material is not presented as verified authentication`() {
        composeTestRule.setContent {
            AgentsScreen(
                reports = listOf(report(RuntimeProfileId.CLAUDE_CODE)),
                keyStatus = emptyList(),
                signInStates = mapOf(RuntimeProfileId.CLAUDE_CODE to AgentSignInState.SIGNED_IN),
                onLaunch = {},
                onInstall = {},
                onEditKeys = {}
            )
        }

        composeTestRule
            .onNodeWithText("Saved login found — the agent verifies it when opened")
            .assertIsDisplayed()
        composeTestRule.onNodeWithText("Signed in").assertDoesNotExist()
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
    fun `every admitted agent declares the command it launches`() {
        val agents = listOf(
            RuntimeProfiles.forId(RuntimeProfileId.CLAUDE_CODE),
            RuntimeProfiles.forId(RuntimeProfileId.CODEX),
            RuntimeProfiles.forId(RuntimeProfileId.OPENCODE),
            RuntimeProfiles.forId(RuntimeProfileId.ANTIGRAVITY)
        )

        assert(agents.isNotEmpty())
        agents.forEach { assert(!it.launchCommand.isNullOrBlank()) }
    }

    @Test
    fun `antigravity appears as an admitted agent card`() {
        show(listOf(report(RuntimeProfileId.ANTIGRAVITY)))
        composeTestRule.onNodeWithTag("agent_antigravity").assertIsDisplayed()
    }
}
