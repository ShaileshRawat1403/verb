package com.example.verb.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.example.verb.project.VerbProject
import com.example.verb.terminal.TerminalSessionState
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Switching terminals from the header, which is where a person actually is when they want to.
 *
 * Reaching the sessions surface to switch was four taps; this is two. The chip is deliberately
 * absent with one terminal -- with nothing to switch between it would be chrome to read past, and
 * `docs/PRODUCT_VISION.md` asks capability to appear when the situation calls for it and recede
 * when it does not.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TerminalSwitcherChipTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private fun screen(
        ids: List<String> = listOf("terminal-1", "terminal-2"),
        active: String = "terminal-1",
        agentIn: (String) -> String? = { null },
        canOpenMore: Boolean = true,
        project: VerbProject? = null,
        state: TerminalSessionState? = null,
        onSwitch: (String) -> Unit = {},
        onOpen: () -> Unit = {}
    ) {
        composeTestRule.setContent {
            TerminalScreen(
                terminalOutput = "",
                terminalRuntime = null,
                onSendCommand = {},
                onSendKey = {},
                onSendText = {},
                onClearTerminal = {},
                onInspectText = {},
                onSubmitIntent = {},
                terminalSessionIds = ids,
                activeTerminalSessionId = active,
                agentInTerminal = agentIn,
                onSwitchTerminalSession = onSwitch,
                onOpenTerminalSession = onOpen,
                canOpenMoreTerminals = canOpenMore,
                selectedProject = project,
                sessionState = state
            )
        }
    }

    @Test
    fun `with one terminal there is nothing to switch and no control to read past`() {
        screen(ids = listOf("terminal-1"))

        composeTestRule.onNodeWithTag("btn_terminal_switcher").assertDoesNotExist()
    }

    @Test
    fun `the chip says which terminal you are in, out of how many`() {
        screen(ids = listOf("a", "b", "c"), active = "b")

        composeTestRule.onNodeWithText("⌄ 2/3").assertIsDisplayed()
    }

    @Test
    fun `switching takes two taps from the terminal itself`() {
        var switched: String? = null
        screen(onSwitch = { switched = it })

        composeTestRule.onNodeWithTag("btn_terminal_switcher").performClick()
        composeTestRule.onNodeWithTag("switch_to_terminal-2").performClick()

        assertEquals("terminal-2", switched)
    }

    /** Switching is a decision, so the menu says what is in each terminal before you commit. */
    @Test
    fun `the menu says what is running in each terminal`() {
        screen(agentIn = { id -> "Codex CLI".takeIf { id == "terminal-2" } })

        composeTestRule.onNodeWithTag("btn_terminal_switcher").performClick()

        composeTestRule.onNodeWithText("Codex CLI is running here").assertIsDisplayed()
        composeTestRule.onNodeWithText("your shell").assertIsDisplayed()
    }

    @Test
    fun `tapping the terminal you are already in does nothing`() {
        var switched: String? = null
        screen(onSwitch = { switched = it })

        composeTestRule.onNodeWithTag("btn_terminal_switcher").performClick()
        composeTestRule.onNodeWithTag("switch_to_terminal-1").performClick()

        assertEquals(null, switched)
    }

    @Test
    fun `a new terminal can be opened without leaving the workspace`() {
        var opened = 0
        screen(onOpen = { opened++ })

        composeTestRule.onNodeWithTag("btn_terminal_switcher").performClick()
        composeTestRule.onNodeWithTag("btn_new_terminal_header").performClick()

        assertEquals(1, opened)
    }

    @Test
    fun `at the ceiling the menu stops offering a new terminal`() {
        screen(canOpenMore = false)

        composeTestRule.onNodeWithTag("btn_terminal_switcher").performClick()

        composeTestRule.onNodeWithTag("btn_new_terminal_header").assertDoesNotExist()
        composeTestRule.onNodeWithTag("switch_to_terminal-1").performClick()

        composeTestRule.onNodeWithTag("btn_terminal_overflow").performClick()
        composeTestRule.onNodeWithTag("btn_new_terminal_overflow").assertDoesNotExist()
    }

    @Test
    fun `the first additional terminal can be opened from the workspace overflow`() {
        var opened = 0
        screen(
            ids = listOf("terminal-1"),
            onOpen = { opened++ }
        )

        composeTestRule.onNodeWithTag("btn_terminal_overflow").performClick()
        composeTestRule.onNodeWithTag("btn_new_terminal_overflow").performClick()

        assertEquals(1, opened)
    }

    @Test
    fun `crowded header keeps session state and a tappable project control`() {
        screen(
            project = VerbProject("mobile-kit-30603ae7", File("/projects/mobile-kit")),
            state = TerminalSessionState.RUNNING
        )

        composeTestRule.onNodeWithText("running").assertIsDisplayed()
        composeTestRule.onNodeWithTag("terminal_project_selector").assertIsDisplayed()
        composeTestRule.onNodeWithText("mobile-kit-30603ae7").assertDoesNotExist()
        val statusBounds = composeTestRule.onNodeWithTag("verb_session_status").getUnclippedBoundsInRoot()
        assertTrue(
            "the status pill must remain horizontal when the terminal switcher is present",
            statusBounds.right - statusBounds.left > statusBounds.bottom - statusBounds.top
        )
    }

    @Test
    fun `runs moves into overflow when the switcher needs the header width`() {
        screen()

        composeTestRule.onNodeWithTag("btn_terminal_runs").assertDoesNotExist()
        composeTestRule.onNodeWithTag("btn_terminal_overflow").performClick()
        composeTestRule.onNodeWithTag("btn_runs_overflow").performClick()

        composeTestRule.onNodeWithTag("terminal_runs_sheet").assertIsDisplayed()
    }
}
