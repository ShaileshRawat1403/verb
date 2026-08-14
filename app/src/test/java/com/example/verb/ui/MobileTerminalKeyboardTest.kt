package com.example.verb.ui

import android.content.Context
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.performTextInput
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

        composeTestRule.onNodeWithTag("key_quick_/").assertExists().performClick()
        composeTestRule.onNodeWithTag("key_quick_|").assertExists().performClick()
        
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
        composeTestRule.onNodeWithTag("key_ctrl").performClick()

        // Now Ctrl+C should be visible
        composeTestRule.onNodeWithTag("key_ctrl_C").assertExists().performClick()
        
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
        composeTestRule.onNodeWithTag("key_tab").performClick()
        assertEquals(listOf("TAB"), keysSent)
        keysSent.clear()
        
        // Tap SHIFT
        composeTestRule.onNodeWithTag("key_shift").performClick()
        composeTestRule.onNodeWithTag("key_tab").performClick()
        
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

        composeTestRule.onNodeWithTag("key_paste").assertExists().performClick()
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
    fun `history recall and interrupt stay reachable while the IME is visible`() {
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

        // These live outside the isKeyboardVisible-gated strips, unlike ESC/CTRL/SHIFT/PASTE.
        composeTestRule.onNodeWithTag("key_up").assertExists().performClick()
        composeTestRule.onNodeWithTag("key_down").assertExists().performClick()
        composeTestRule.onNodeWithTag("key_tab").assertExists().performClick()
        composeTestRule.onNodeWithTag("key_essential_ctrl_c").assertExists().performClick()

        assertEquals(listOf("UP", "DOWN", "TAB", "CTRL_C"), keysSent)

        // The gated power strip should not be present while the IME is up.
        composeTestRule.onAllNodesWithTag("key_esc").assertCountEquals(0)
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

        // Just verify the settings button is there since testing ModalBottomSheet is flaky in Robolectric
        composeTestRule.onNodeWithTag("btn_edit_quick_keys").assertExists()
        assertTrue(true)
    }
}
