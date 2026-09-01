package com.example.verb.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.text.TextRange
import com.example.verb.terminal.CommandExecutionRecord
import com.example.verb.terminal.SelectionChangeListener
import com.example.verb.terminal.TerminalContextState
import com.example.verb.terminal.TerminalRuntimeAdapter
import com.example.verb.terminal.TerminalSessionState
import com.example.verb.terminal.TerminalWorkingDirectory
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

/**
 * Test-only adapter with no Termux session behind it, so [TerminalScreen] renders its Compose
 * fallback canvas instead of a real [com.termux.view.TerminalView].
 *
 * [currentWorkingDirectory] stays null throughout, which is exactly what a real Agent Runtime
 * session looks like: that rootfs ships no shell-integration script, so no OSC 7 ever arrives.
 */
private class StatusTestRuntimeAdapter : TerminalRuntimeAdapter {
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

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TerminalScreenSessionStatusTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    /**
     * Regression: the status pill used to read `terminalRuntime.sessionState.value`, which is not a
     * Compose snapshot read. It therefore only refreshed when something *else* recomposed the
     * screen -- in practice a `terminalOutput` change -- so a session that died quietly kept showing
     * a green "running" pill indefinitely. The output string is deliberately held constant here so
     * the only thing that can drive the update is the session state itself.
     */
    @Test
    fun `status pill updates when session state changes with no terminal output change`() {
        var sessionState by mutableStateOf(TerminalSessionState.RUNNING)

        composeTestRule.setContent {
            TerminalScreen(
                terminalOutput = "constant output",
                terminalRuntime = StatusTestRuntimeAdapter(),
                sessionState = sessionState,
                onSendCommand = {},
                onSendKey = {},
                onSendText = {},
                onClearTerminal = {},
                onInspectText = {},
                onSubmitIntent = {}
            )
        }

        composeTestRule.onNodeWithText("running").assertIsDisplayed()

        sessionState = TerminalSessionState.FAILED
        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithText("failed").assertIsDisplayed()

        sessionState = TerminalSessionState.EXITED
        composeTestRule.waitForIdle()

        composeTestRule.onNodeWithText("exited").assertIsDisplayed()
    }

    /**
     * The Agent Runtime has no shell integration, so the live directory is unknown. The diagnostics
     * surface must say so rather than presenting [TerminalRuntimeAdapter.launchWorkingDirectory] as
     * though the shell were there.
     */
    @Test
    fun `an adapter without shell integration reports an unknown current directory`() {
        val adapter = StatusTestRuntimeAdapter()

        assertNull(adapter.currentWorkingDirectory.value)
        assert(!adapter.shellIntegrationActive.value)
    }

    @Test
    fun `slow agent launch remains visibly explained while its terminal is blank`() {
        composeTestRule.setContent {
            TerminalScreen(
                terminalOutput = "",
                terminalRuntime = StatusTestRuntimeAdapter(),
                sessionState = TerminalSessionState.RUNNING,
                terminalLaunchNotice =
                    "Starting Antigravity in compatibility mode — its first screen can take about 30 seconds.",
                onSendCommand = {},
                onSendKey = {},
                onSendText = {},
                onClearTerminal = {},
                onInspectText = {},
                onSubmitIntent = {}
            )
        }

        composeTestRule.onNodeWithText(
            "Starting Antigravity in compatibility mode — its first screen can take about 30 seconds."
        ).assertIsDisplayed()
    }
}
