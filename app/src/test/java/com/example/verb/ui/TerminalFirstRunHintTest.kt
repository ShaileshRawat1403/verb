package com.example.verb.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.text.TextRange
import com.example.verb.terminal.CommandExecutionRecord
import com.example.verb.terminal.SelectionChangeListener
import com.example.verb.terminal.TerminalContextState
import com.example.verb.terminal.TerminalRuntimeAdapter
import com.example.verb.terminal.TerminalSessionState
import com.example.verb.terminal.TerminalWorkingDirectory
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

private class HintTestRuntimeAdapter : TerminalRuntimeAdapter {
    override val sessionState: StateFlow<TerminalSessionState> = MutableStateFlow(TerminalSessionState.RUNNING)
    override val terminalOutput: StateFlow<String> = MutableStateFlow("")
    override val activeSelectionText: StateFlow<String> = MutableStateFlow("")
    override val activeSelectionRange: StateFlow<TextRange> = MutableStateFlow(TextRange.Zero)
    override val isSessionActive: StateFlow<Boolean> = MutableStateFlow(true)
    override val terminalContextState: StateFlow<TerminalContextState> = MutableStateFlow(TerminalContextState())
    override val commandHistory: StateFlow<List<CommandExecutionRecord>> = MutableStateFlow(emptyList())
    override val shellIntegrationActive: StateFlow<Boolean> = MutableStateFlow(false)
    override val urlToOpen: StateFlow<String?> = MutableStateFlow(null)
    override fun consumeUrlToOpen() {}
    override val clipboardCopyEvent: StateFlow<String?> = MutableStateFlow(null)
    override fun consumeClipboardCopyEvent() {}
    override val launchWorkingDirectory: File = File("/launch/dir")
    override val currentWorkingDirectory: StateFlow<TerminalWorkingDirectory?> = MutableStateFlow(null)
    override fun startSession() {}
    override fun attachSession() {}
    override fun sendText(text: String) {}
    override fun sendCommand(cmd: String) {}
    override fun sendControlKey(key: String) {}
    override fun resize(rows: Int, cols: Int) {}
    override fun selectedText(): String = ""
    override fun notifySelectionChanged(selectedRange: TextRange, selectedText: String) {}
    override fun addSelectionChangeListener(listener: SelectionChangeListener) {}
    override fun removeSelectionChangeListener(listener: SelectionChangeListener) {}
    override fun clearBuffer() {}
    override fun restartSession() {}
    override fun destroy() {}
}

/**
 * The "nothing has come back yet" hint, and why it must be latched.
 *
 * A full-screen agent repaints by clearing the screen, so the output snapshot goes empty for a
 * moment on every redraw. While this row was gated on `terminalOutput.isBlank()` that emptiness
 * brought 121dp of chrome back above a canvas that takes the remaining height: the terminal lost
 * ~145px, the PTY was resized, the agent answered the SIGWINCH with another full repaint, and the
 * repaint cleared the screen again.
 *
 * Measured on a Vivo I2202 with Antigravity, from a single tap and no further input: the canvas
 * alternated 1360/1215 roughly every 430ms indefinitely, at 41 PTY callbacks per five seconds.
 * After latching: one resize and one callback in 34 seconds. Codex behaved the same way; a shell
 * never blanks its screen, which is why only the two full-screen agents flickered.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TerminalFirstRunHintTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private fun render(output: () -> String) {
        composeTestRule.setContent {
            TerminalScreen(
                terminalOutput = output(),
                terminalRuntime = HintTestRuntimeAdapter(),
                sessionState = TerminalSessionState.RUNNING,
                onSendCommand = {},
                onSendKey = {},
                onSendText = {},
                onClearTerminal = {},
                onInspectText = {},
                onSubmitIntent = {}
            )
        }
    }

    @Test
    fun `the hint is shown while a running session has produced nothing`() {
        render { "" }

        composeTestRule.onNodeWithTag("first_run_terminal_hint").assertIsDisplayed()
    }

    /**
     * The regression. Output arrives, then the snapshot goes blank again because the agent cleared
     * the screen to redraw. The hint must not come back: "has anything arrived yet" is a fact about
     * the session's history, and history does not un-happen because the current frame is mid-redraw.
     */
    @Test
    fun `the hint stays gone once output has been seen, even if the screen is cleared`() {
        var output by mutableStateOf("")
        render { output }

        composeTestRule.onNodeWithTag("first_run_terminal_hint").assertIsDisplayed()

        output = "Antigravity CLI 1.1.25"
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("first_run_terminal_hint").assertDoesNotExist()

        // The agent clears the screen to repaint. This is the frame that used to reintroduce the row.
        output = ""
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("first_run_terminal_hint").assertDoesNotExist()

        // And it must not reappear on any subsequent clear either.
        output = "redrawn"
        composeTestRule.waitForIdle()
        output = ""
        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("first_run_terminal_hint").assertDoesNotExist()
    }
}
