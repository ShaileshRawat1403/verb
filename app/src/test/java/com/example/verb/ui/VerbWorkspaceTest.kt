package com.example.verb.ui

import androidx.compose.material3.Text
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.example.verb.terminal.TerminalSessionState
import com.example.verb.terminal.TermuxBootstrapInstaller
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The workspace, after the five-tab navigation was removed.
 *
 * `terminalRuntime` is deliberately null: with no Termux session behind it the screen renders its
 * Compose fallback canvas, which is enough to assert everything about Verb's own chrome and keeps
 * these tests off a real PTY.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class VerbWorkspaceTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private fun setWorkspace(
        sessionState: TerminalSessionState? = TerminalSessionState.RUNNING,
        bootstrapState: TermuxBootstrapInstaller.State = TermuxBootstrapInstaller.State.Ready,
        onOpenVerb: () -> Unit = {},
        verbSurfaceOpen: Boolean = false,
        firstAction: (@androidx.compose.runtime.Composable () -> Unit)? = null
    ) {
        composeTestRule.setContent {
            TerminalScreen(
                terminalOutput = "",
                terminalRuntime = null,
                sessionState = sessionState,
                bootstrapState = bootstrapState,
                onSendCommand = {},
                onSendKey = {},
                onSendText = {},
                onClearTerminal = {},
                onInspectText = {},
                onSubmitIntent = {},
                onOpenVerb = onOpenVerb,
                verbSurfaceOpen = verbSurfaceOpen,
                verbFirstAction = firstAction
            )
        }
    }

    /**
     * The regression this whole phase exists to prevent coming back. A permanent bottom navigation
     * organised by subsystem is what `docs/UX_FOUNDATION.md` prohibits, and the tag it used to carry
     * is asserted absent so a well-meaning re-add fails a test rather than shipping.
     */
    @Test
    fun `the workspace draws no permanent bottom navigation`() {
        setWorkspace()
        composeTestRule.onNodeWithTag("verb_bottom_navigation").assertDoesNotExist()
        listOf("tab_agents", "tab_ask", "tab_assistant", "tab_system", "tab_terminal").forEach { tag ->
            composeTestRule.onNodeWithTag(tag).assertDoesNotExist()
        }
    }

    @Test
    fun `the workspace offers one discoverable way into everything Verb can do`() {
        var opened = 0
        setWorkspace(onOpenVerb = { opened++ })

        composeTestRule.onNodeWithTag("verb_sheet_trigger").assertIsDisplayed()
        composeTestRule.onNodeWithTag("verb_sheet_trigger").performClick()

        assertEquals(1, opened)
    }

    /**
     * The header chip used to open a second natural-language surface that neither the Ask tab nor the
     * Assistant tab knew about. That sheet is retired; the chip is now the way in to named tasks.
     */
    @Test
    fun `the retired natural-language sheet is no longer reachable from the workspace`() {
        setWorkspace()
        composeTestRule.onNodeWithTag("verb_nl_trigger_top").assertDoesNotExist()
        composeTestRule.onNodeWithTag("verb_natural_language_sheet").assertDoesNotExist()
    }

    @Test
    fun `the mounted terminal stops accepting text while a Verb surface owns input`() {
        setWorkspace(verbSurfaceOpen = true)

        composeTestRule.onNodeWithTag("terminal_input_field").assertIsNotEnabled()
    }

    // ---- status: glyph and word, never colour alone ---------------------------------------------

    @Test
    fun `session status carries a glyph and a word`() {
        setWorkspace(sessionState = TerminalSessionState.RUNNING)

        composeTestRule.onNodeWithText("running").assertIsDisplayed()
        composeTestRule
            .onNodeWithTag("verb_session_status_glyph", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Test
    fun `session status is readable by a screen reader without seeing the colour`() {
        setWorkspace(sessionState = TerminalSessionState.FAILED)

        composeTestRule
            .onNodeWithTag("verb_session_status")
            .assertContentDescriptionEquals(
                "Terminal session failed. Activate to start a new session."
            )
    }

    @Test
    fun `an unreported session state says so rather than showing nothing`() {
        setWorkspace(sessionState = null)
        composeTestRule.onNodeWithText("not ready").assertIsDisplayed()
    }

    // ---- the first action ----------------------------------------------------------------------

    @Test
    fun `the first action is drawn when the workspace has one to offer`() {
        setWorkspace(firstAction = { Text("Start Claude Code") })
        composeTestRule.onNodeWithText("Start Claude Code").assertIsDisplayed()
    }

    /**
     * While the userland is still installing, "start an agent" is not yet true. A suggestion that
     * cannot work is worse than no suggestion, so the offer waits for setup to finish.
     */
    @Test
    fun `the first action is withheld while the runtime is still installing`() {
        setWorkspace(
            bootstrapState = TermuxBootstrapInstaller.State.Extracting,
            firstAction = { Text("Start Claude Code") }
        )
        composeTestRule.onNodeWithText("Start Claude Code").assertDoesNotExist()
    }
}
