package com.example.verb.ui

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onChildren
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.example.verb.terminal.CommandExecutionRecord
import com.example.verb.terminal.CommandLifecycleState
import com.example.verb.terminal.SelectionChangeListener
import com.example.verb.terminal.TerminalContextState
import com.example.verb.terminal.TerminalRuntimeAdapter
import com.example.verb.terminal.TerminalSessionState
import com.example.verb.terminal.TerminalWorkingDirectory
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Minimal, test-only [TerminalRuntimeAdapter] that exposes directly-settable [commandHistory] and
 * [shellIntegrationActive] flows. [com.example.verb.terminal.CommandExecutionTracker] only builds
 * records from parsed OSC events with real wall-clock timestamps, so it can't produce the fixed,
 * deterministic records these UI tests need -- this fake bypasses it entirely without touching it.
 */
private class TestTerminalRuntimeAdapter(
    initialHistory: List<CommandExecutionRecord> = emptyList(),
    initialActive: Boolean = false,
    private val onRestartSession: () -> Unit = {}
) : TerminalRuntimeAdapter {
    private val _commandHistory = MutableStateFlow(initialHistory)
    override val commandHistory: StateFlow<List<CommandExecutionRecord>> = _commandHistory.asStateFlow()

    private val _shellIntegrationActive = MutableStateFlow(initialActive)
    override val shellIntegrationActive: StateFlow<Boolean> = _shellIntegrationActive.asStateFlow()

    override val sessionState: StateFlow<TerminalSessionState> = MutableStateFlow(TerminalSessionState.RUNNING)
    override val terminalOutput: StateFlow<String> = MutableStateFlow("")
    override val activeSelectionText: StateFlow<String> = MutableStateFlow("")
    override val activeSelectionRange: StateFlow<TextRange> = MutableStateFlow(TextRange.Zero)
    override val isSessionActive: StateFlow<Boolean> = MutableStateFlow(true)
    override val terminalContextState: StateFlow<TerminalContextState> = MutableStateFlow(TerminalContextState())
    override val urlToOpen: StateFlow<String?> = MutableStateFlow(null)
    override fun consumeUrlToOpen() {}
    override val clipboardCopyEvent: StateFlow<String?> = MutableStateFlow(null)
    override fun consumeClipboardCopyEvent() {}
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
    override val launchWorkingDirectory: File = File("/")
    override val currentWorkingDirectory: StateFlow<TerminalWorkingDirectory?> = MutableStateFlow(null)
    override fun clearBuffer() {}
    override fun restartSession() { onRestartSession() }
    override fun destroy() {}
}

private fun record(
    id: String,
    commandText: String,
    state: CommandLifecycleState,
    startedAtEpochMs: Long = 1_000L,
    endedAtEpochMs: Long? = 1_120L,
    exitCode: Int? = null,
    workingDirectory: String? = null
) = CommandExecutionRecord(
    id = id,
    commandText = commandText,
    workingDirectory = workingDirectory,
    startedAtEpochMs = startedAtEpochMs,
    endedAtEpochMs = endedAtEpochMs,
    exitCode = exitCode,
    state = state
)

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class RunsSheetTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun completedFailedAndAbandonedRecordsRenderTheSpecifiedFormat() {
        val runtime = TestTerminalRuntimeAdapter(
            initialHistory = listOf(
                record("1", "git status", CommandLifecycleState.COMPLETED, 1_000L, 1_120L, exitCode = 0),
                record("2", "ls /missing", CommandLifecycleState.FAILED, 1_000L, 1_040L, exitCode = 2),
                record("3", "npm install", CommandLifecycleState.ABANDONED, 1_000L, 1_500L)
            ),
            initialActive = true
        )

        composeTestRule.setContent { RunsSheet(terminalRuntime = runtime, onDismiss = {}) }

        composeTestRule.onNodeWithText("git status").assertExists()
        composeTestRule.onNodeWithText("120 ms").assertExists()

        composeTestRule.onNodeWithText("ls /missing").assertExists()
        composeTestRule.onNodeWithText("Failed · exit 2 · 40 ms").assertExists()

        composeTestRule.onNodeWithText("npm install").assertExists()
        composeTestRule.onNodeWithText("Interrupted").assertExists()
    }

    @Test
    fun newestRecordIsRenderedFirst() {
        val runtime = TestTerminalRuntimeAdapter(
            initialHistory = listOf(
                record("older", "first command", CommandLifecycleState.COMPLETED, exitCode = 0),
                record("newer", "second command", CommandLifecycleState.COMPLETED, exitCode = 0)
            ),
            initialActive = true
        )

        composeTestRule.setContent { RunsSheet(terminalRuntime = runtime, onDismiss = {}) }

        composeTestRule.onNodeWithTag("runs_list")
            .onChildren()[0]
            .assert(hasTestTag("run_row_newer"))
    }

    @Test
    fun blankCommandTextFallsBackToGenericLabel() {
        val runtime = TestTerminalRuntimeAdapter(
            initialHistory = listOf(record("1", "", CommandLifecycleState.COMPLETED, exitCode = 0)),
            initialActive = true
        )

        composeTestRule.setContent { RunsSheet(terminalRuntime = runtime, onDismiss = {}) }

        composeTestRule.onNodeWithText("Command run").assertExists()
    }

    @Test
    fun emptyHistoryWithActiveIntegrationShowsNoRunsYetMessage() {
        val runtime = TestTerminalRuntimeAdapter(initialHistory = emptyList(), initialActive = true)

        composeTestRule.setContent { RunsSheet(terminalRuntime = runtime, onDismiss = {}) }

        composeTestRule.onNodeWithTag("runs_empty_active").assertExists()
        composeTestRule.onNodeWithText("No commands run yet.").assertExists()
    }

    @Test
    fun emptyHistoryWithInactiveIntegrationShowsWaitingMessage() {
        val runtime = TestTerminalRuntimeAdapter(initialHistory = emptyList(), initialActive = false)

        composeTestRule.setContent { RunsSheet(terminalRuntime = runtime, onDismiss = {}) }

        composeTestRule.onNodeWithTag("runs_empty_inactive").assertExists()
        composeTestRule.onNodeWithText("Runs will appear after the terminal shell is ready.").assertExists()
    }

    @Test
    fun footerDisclosesLocalOnlyPrivacyStance() {
        val runtime = TestTerminalRuntimeAdapter()

        composeTestRule.setContent { RunsSheet(terminalRuntime = runtime, onDismiss = {}) }

        composeTestRule.onNodeWithTag("runs_privacy_footer")
            .assertExists()
        composeTestRule.onNodeWithText("Local terminal activity. Nothing is sent to AI.").assertExists()
    }

    @Test
    fun workingDirectoryIsNeverRenderedIntoTheRow() {
        val runtime = TestTerminalRuntimeAdapter(
            initialHistory = listOf(
                record(
                    "1",
                    "git status",
                    CommandLifecycleState.COMPLETED,
                    exitCode = 0,
                    workingDirectory = "/very/secret/cwd/path"
                )
            ),
            initialActive = true
        )

        composeTestRule.setContent { RunsSheet(terminalRuntime = runtime, onDismiss = {}) }

        composeTestRule.onNodeWithText("/very/secret/cwd/path", substring = true).assertDoesNotExist()
    }

    @Test
    fun closingTheSheetInvokesOnDismiss() {
        var dismissed = false
        val runtime = TestTerminalRuntimeAdapter()

        composeTestRule.setContent { RunsSheet(terminalRuntime = runtime, onDismiss = { dismissed = true }) }

        composeTestRule.onNodeWithTag("btn_close_runs").performClick()

        assertTrue("Expected onDismiss to be invoked by the close button", dismissed)
    }

    @Test
    fun headerExposesAnAccessibleRunsTrigger() {
        // ModalBottomSheet renders into a separate Popup window that this project's
        // Robolectric/compose-ui-test combination doesn't reliably surface to the default
        // semantics tree (see MobileTerminalKeyboardTest's "testing ModalBottomSheet is flaky in
        // Robolectric" precedent) -- so the open/dismiss round trip itself is covered by
        // [closingTheSheetInvokesOnDismiss] against RunsSheet directly, and physically verified on
        // device. This test covers what Robolectric CAN check reliably: the header trigger exists,
        // is accessibly labeled, and clicking it doesn't crash the screen.
        composeTestRule.setContent {
            TerminalScreen(
                terminalOutput = "",
                terminalRuntime = null,
                onSendCommand = {},
                onSendKey = {},
                onSendText = {},
                onClearTerminal = {},
                onInspectText = {},
                onSubmitIntent = {}
            )
        }

        composeTestRule.onNodeWithTag("btn_terminal_runs")
            .assertExists()
            .assertContentDescriptionEquals("Command run history")
            .performClick()
    }

    @Test
    fun headerExposesAnAccessibleOverflowMenuWithTheOtherActions() {
        composeTestRule.setContent {
            TerminalScreen(
                terminalOutput = "",
                terminalRuntime = null,
                onSendCommand = {},
                onSendKey = {},
                onSendText = {},
                onClearTerminal = {},
                onInspectText = {},
                onSubmitIntent = {}
            )
        }

        composeTestRule.onNodeWithTag("btn_terminal_overflow")
            .assertExists()
            .assertContentDescriptionEquals("More terminal actions")
            .performClick()

        composeTestRule.onNodeWithText("Diagnostics").assertExists()
        composeTestRule.onNodeWithText("Browse Files").assertExists()
        composeTestRule.onNodeWithText("Explain evidence with AI").assertExists()
        composeTestRule.onNodeWithText("Restart Session").assertExists()
        composeTestRule.onNodeWithText("Clear Terminal").assertExists()
    }

    @Test
    fun overflowClearTerminalInvokesTheOriginalClearHandler() {
        var clearCount = 0
        composeTestRule.setContent {
            TerminalScreen(
                terminalOutput = "",
                terminalRuntime = null,
                onSendCommand = {},
                onSendKey = {},
                onSendText = {},
                onClearTerminal = { clearCount++ },
                onInspectText = {},
                onSubmitIntent = {}
            )
        }

        composeTestRule.onNodeWithTag("btn_terminal_overflow").performClick()
        composeTestRule.onNodeWithText("Clear Terminal").performClick()

        assertEquals(1, clearCount)
    }

    @Test
    fun overflowExplainWithAiInvokesTheOriginalExplainHandler() {
        var explainCount = 0
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
                onExplainOutput = { explainCount++ }
            )
        }

        composeTestRule.onNodeWithTag("btn_terminal_overflow").performClick()
        composeTestRule.onNodeWithText("Explain evidence with AI").performClick()

        assertEquals(1, explainCount)
    }

    @Test
    fun overflowRestartSessionInvokesTheOriginalRestartHandler() {
        var restartCount = 0
        val runtime = TestTerminalRuntimeAdapter(onRestartSession = { restartCount++ })
        composeTestRule.setContent {
            TerminalScreen(
                terminalOutput = "",
                terminalRuntime = runtime,
                onSendCommand = {},
                onSendKey = {},
                onSendText = {},
                onClearTerminal = {},
                onInspectText = {},
                onSubmitIntent = {}
            )
        }

        composeTestRule.onNodeWithTag("btn_terminal_overflow").performClick()
        composeTestRule.onNodeWithText("Restart Session").performClick()

        assertEquals(1, restartCount)
    }
}
