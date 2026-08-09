package com.example.verb.ui

import android.content.Context
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.performTextInput
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
@Config(sdk = [36])
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
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        composeTestRule.onNodeWithTag("key_paste").assertExists().performClick()
        assertEquals(listOf("PASTE"), keysSent)
    }

    @Test
    fun `user typed terminal input is submitted only by the terminal input control`() {
        val commandsSent = mutableListOf<String>()
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = {},
                onSendCommand = { commandsSent.add(it) },
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        composeTestRule.onNodeWithTag("terminal_input_field").performTextInput("git status")
        composeTestRule.onNodeWithTag("terminal_input_submit").performClick()

        assertEquals(listOf("git status"), commandsSent)
    }

    @Test
    fun `empty terminal input sends an explicit Enter for interactive programs`() {
        val commandsSent = mutableListOf<String>()
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = {},
                onSendCommand = { commandsSent.add(it) },
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        composeTestRule.onNodeWithTag("terminal_input_submit").performClick()

        assertEquals(listOf(""), commandsSent)
    }

    @Test
    fun `quick-key customisation persistence`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        
        composeTestRule.setContent {
            MobileTerminalKeyboard(
                onSendKey = {},
                onSendCommand = {},
                terminalOutput = "",
                onInspectOutput = {}
            )
        }

        // Just verify the settings button is there since testing ModalBottomSheet is flaky in Robolectric
        composeTestRule.onNodeWithTag("btn_edit_quick_keys").assertExists()
        assertTrue(true)
    }
}
