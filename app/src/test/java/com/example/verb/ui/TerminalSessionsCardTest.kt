package com.example.verb.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The switcher: where a person answers *"where was I working?"* — the ORIENT moment
 * `docs/UX_FOUNDATION.md` names and that nothing served while the sessions surface was the agents
 * card under a second name.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TerminalSessionsCardTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private fun card(
        ids: List<String> = listOf("terminal-1", "terminal-2"),
        active: String? = "terminal-2",
        agentIn: (String) -> String? = { null },
        canOpenMore: Boolean = true,
        onOpen: () -> Unit = {},
        onSwitch: (String) -> Unit = {},
        onClose: (String) -> Unit = {}
    ) {
        composeTestRule.setContent {
            TerminalSessionsCard(
                sessionIds = ids,
                activeId = active,
                agentIn = agentIn,
                canOpenMore = canOpenMore,
                onOpen = onOpen,
                onSwitch = onSwitch,
                onClose = onClose
            )
        }
    }

    /** Which one is in front has to be readable without relying on the dot's colour. */
    @Test
    fun `the terminal in front says so in words, not only in colour`() {
        card()

        composeTestRule.onNodeWithText("your shell, in front").assertIsDisplayed()
        composeTestRule.onNodeWithText("your shell").assertIsDisplayed()
    }

    /**
     * The reason a list of terminals is worth looking at: it says where the agent is, and by
     * omission which one is yours to type in.
     */
    @Test
    fun `a terminal running an agent names it`() {
        card(agentIn = { id -> "Claude Code".takeIf { id == "terminal-1" } })

        composeTestRule.onNodeWithText("Claude Code is running here").assertIsDisplayed()
    }

    @Test
    fun `tapping a terminal that is not in front switches to it`() {
        var switched: String? = null
        card(onSwitch = { switched = it })

        composeTestRule.onNodeWithTag("terminal_session_terminal-1").performClick()

        assertEquals("terminal-1", switched)
    }

    /** A workspace with no terminal is a screen with nothing to do, so the last one has no Close. */
    @Test
    fun `the only terminal cannot be closed`() {
        card(ids = listOf("terminal-1"), active = "terminal-1")

        composeTestRule.onNodeWithTag("btn_close_terminal-1").assertDoesNotExist()
    }

    @Test
    fun `closing is offered once there is more than one`() {
        var closed: String? = null
        card(onClose = { closed = it })

        composeTestRule.onNodeWithTag("btn_close_terminal-2").performClick()

        assertEquals("terminal-2", closed)
    }

    /** The ceiling is stated where it is reached, not discovered by the phone running out of memory. */
    @Test
    fun `the ceiling explains itself instead of just disabling the button`() {
        card(canOpenMore = false)

        composeTestRule.onNodeWithText("That is as many as this device should host at once.")
            .assertIsDisplayed()
    }
}
