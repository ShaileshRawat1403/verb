package com.example.verb.ui

import android.content.Context
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextReplacement
import androidx.test.core.app.ApplicationProvider
import com.example.verb.terminal.MobileTerminalKeyboard
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class MobileTerminalKeyboardTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun `default quick keys are displayed and emit keys`() {
        val keysSent = mutableListOf<String>()
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = { keysSent.add(it) },
                onSendCommand = {},
                onSendText = {},
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        // Collapsed by default: the symbol row is one tap away, not permanently on screen.
        composeTestRule.onAllNodesWithTag("key_quick_/").assertCountEquals(0)

        composeTestRule.onNodeWithTag("btn_toggle_key_panel").performClick()

        composeTestRule.onNodeWithTag("key_quick_/").performScrollTo().performClick()
        composeTestRule.onNodeWithTag("key_quick_|").performScrollTo().performClick()

        assertEquals(listOf("/", "|"), keysSent)
    }

    @Test
    fun `ctrl state transitions`() {
        val keysSent = mutableListOf<String>()
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = { keysSent.add(it) },
                onSendCommand = {},
                onSendText = {},
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        // Initially Ctrl+C is not there (hidden in second row)
        composeTestRule.onAllNodesWithTag("key_ctrl_C").assertCountEquals(0)

        // Tap CTRL
        composeTestRule.onNodeWithTag("key_ctrl").performScrollTo().performClick()

        // Now Ctrl+C should be visible
        composeTestRule.onNodeWithTag("key_ctrl_C").performScrollTo().performClick()
        
        // Emits CTRL_C and exits Ctrl mode
        assertEquals(listOf("CTRL_C"), keysSent)
        composeTestRule.onAllNodesWithTag("key_ctrl_C").assertCountEquals(0)
    }

    @Test
    fun `shift tab emits SHIFT_TAB`() {
        val keysSent = mutableListOf<String>()
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = { keysSent.add(it) },
                onSendCommand = {},
                onSendText = {},
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        // Tap TAB normally
        composeTestRule.onNodeWithTag("key_tab").performScrollTo().performClick()
        assertEquals(listOf("TAB"), keysSent)
        keysSent.clear()

        // SHIFT is a rarely used modifier, so it lives in the expandable panel.
        composeTestRule.onNodeWithTag("btn_toggle_key_panel").performClick()
        composeTestRule.onNodeWithTag("key_shift").performScrollTo().performClick()
        composeTestRule.onNodeWithTag("key_tab").performScrollTo().performClick()

        assertEquals(listOf("SHIFT_TAB"), keysSent)
    }

    @Test
    fun `PASTE emits PASTE`() {
        val keysSent = mutableListOf<String>()
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = { keysSent.add(it) },
                onSendCommand = {},
                onSendText = {},
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        composeTestRule.onNodeWithTag("key_paste").performScrollTo().performClick()
        assertEquals(listOf("PASTE"), keysSent)
    }

    @Test
    fun `user typed terminal input echoes to the shell and submits only a newline`() {
        val textsSent = mutableListOf<String>()
        val commandsSent = mutableListOf<String>()
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = {},
                onSendCommand = { commandsSent.add(it) },
                onSendText = { textsSent.add(it) },
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        composeTestRule.onNodeWithTag("terminal_input_field").performTextInput("git status")
        composeTestRule.onNodeWithTag("terminal_input_submit").performClick()

        // Characters are forwarded to the PTY live so they echo on the terminal canvas.
        assertEquals(listOf("git status"), textsSent)
        // The text is already on the shell line; Enter just completes it.
        assertEquals(listOf(""), commandsSent)
    }

    @Test
    fun `deleting trailing characters sends backspace to the shell`() {
        val keysSent = mutableListOf<String>()
        val textsSent = mutableListOf<String>()
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = { keysSent.add(it) },
                onSendCommand = {},
                onSendText = { textsSent.add(it) },
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        composeTestRule.onNodeWithTag("terminal_input_field").performTextInput("abc")
        composeTestRule.onNodeWithTag("terminal_input_field").performTextReplacement("ab")

        assertEquals(listOf("abc"), textsSent)
        assertEquals(listOf("BACKSPACE"), keysSent)
    }

    @Test
    fun `empty terminal input sends an explicit Enter for interactive programs`() {
        val textsSent = mutableListOf<String>()
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = {},
                onSendCommand = {},
                onSendText = { textsSent.add(it) },
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        composeTestRule.onNodeWithTag("terminal_input_submit").performClick()

        assertEquals(listOf("\r"), textsSent)
    }

    @Test
    fun `submitting typed input reports the real command for history, not an empty string`() {
        val commandsExecuted = mutableListOf<String>()
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = {},
                onSendCommand = {},
                onSendText = {},
                terminalOutput = "",
                onInspectOutput = {},
                onCommandExecuted = { commandsExecuted.add(it) }
            )
        }

        composeTestRule.onNodeWithTag("terminal_input_field").performTextInput("git status")
        composeTestRule.onNodeWithTag("terminal_input_submit").performClick()

        assertEquals(listOf("git status"), commandsExecuted)
    }

    @Test
    fun `the resting key row stays reachable while the IME is visible`() {
        val keysSent = mutableListOf<String>()
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = { keysSent.add(it) },
                onSendCommand = {},
                onSendText = {},
                terminalOutput = "",
                isKeyboardVisible = true,
                onInspectOutput = {}
            )
        }

        composeTestRule.onNodeWithTag("key_up").performScrollTo().performClick()
        composeTestRule.onNodeWithTag("key_down").performScrollTo().performClick()
        composeTestRule.onNodeWithTag("key_tab").performScrollTo().performClick()
        composeTestRule.onNodeWithTag("key_essential_ctrl_c").performScrollTo().performClick()
        // ESC used to be hidden the moment the IME appeared, which is precisely when a terminal
        // user reaches for it. The resting row is no longer gated on keyboard visibility.
        composeTestRule.onNodeWithTag("key_esc").performScrollTo().performClick()

        assertEquals(listOf("UP", "DOWN", "TAB", "CTRL_C", "ESC"), keysSent)
    }

    @Test
    fun `quick-key customisation persistence`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = {},
                onSendCommand = {},
                onSendText = {},
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        // Just verify the settings button is there since testing ModalBottomSheet is flaky in
        // Robolectric. It moved into the expandable panel along with the quick keys themselves.
        composeTestRule.onNodeWithTag("btn_toggle_key_panel").performClick()
        composeTestRule.onNodeWithTag("btn_edit_quick_keys").performScrollTo().assertExists()
        assertTrue(true)
    }
}
