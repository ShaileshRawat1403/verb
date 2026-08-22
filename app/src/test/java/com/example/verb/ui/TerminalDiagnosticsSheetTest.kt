package com.example.verb.ui

import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.text.TextRange
import com.example.verb.terminal.CommandExecutionRecord
import com.example.verb.terminal.LogCategory
import com.example.verb.terminal.SelectionChangeListener
import com.example.verb.terminal.TerminalContextState
import com.example.verb.terminal.TerminalRuntimeAdapter
import com.example.verb.terminal.TerminalSessionLogger
import com.example.verb.terminal.TerminalSessionState
import com.example.verb.terminal.TerminalWorkingDirectory
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

private class DiagnosticsTestRuntimeAdapter(
    override val launchWorkingDirectory: File = File("/data/user/0/com.aistudio.verb.app/files/projects/demo"),
    workingDirectory: TerminalWorkingDirectory? = null
) : TerminalRuntimeAdapter {
    override val currentWorkingDirectory: StateFlow<TerminalWorkingDirectory?> =
        MutableStateFlow(workingDirectory)
    override val sessionState: StateFlow<TerminalSessionState> = MutableStateFlow(TerminalSessionState.RUNNING)
    override val terminalOutput: StateFlow<String> = MutableStateFlow("")
    override val activeSelectionText: StateFlow<String> = MutableStateFlow("")
    override val activeSelectionRange: StateFlow<TextRange> = MutableStateFlow(TextRange.Zero)
    override val isSessionActive: StateFlow<Boolean> = MutableStateFlow(true)
    override val terminalContextState: StateFlow<TerminalContextState> = MutableStateFlow(TerminalContextState())
    override val commandHistory: StateFlow<List<CommandExecutionRecord>> = MutableStateFlow(emptyList())
    override val shellIntegrationActive: StateFlow<Boolean> = MutableStateFlow(true)
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
    override fun clearBuffer() {}
    override fun restartSession() {}
    override fun destroy() {}
}

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TerminalDiagnosticsSheetTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @Before
    fun clearLogs() {
        TerminalSessionLogger.clear()
    }

    private fun show(adapter: TerminalRuntimeAdapter) {
        composeTestRule.setContent {
            TerminalDiagnosticsSheet(terminalRuntime = adapter, onDismiss = {})
        }
    }

    /**
     * Regression for the Vivo I2202 report: the action row was laid out past the bottom of the
     * sheet and could not be scrolled into view, so Copy Report was simply unreachable.
     */
    @Test
    fun `copy report action is displayed and clickable`() {
        show(DiagnosticsTestRuntimeAdapter())

        composeTestRule.onNodeWithText("Copy Report").assertIsDisplayed().assertHasClickAction()
    }

    @Test
    fun `copy report stays reachable with a long log list`() {
        repeat(300) { i ->
            TerminalSessionLogger.info(LogCategory.DIAGNOSTIC, "filler log entry number $i for layout regression")
        }

        show(DiagnosticsTestRuntimeAdapter())

        composeTestRule.onNodeWithText("Copy Report").assertIsDisplayed().assertHasClickAction()
        composeTestRule.onNodeWithText("Clear Logs").assertIsDisplayed().assertHasClickAction()
    }

    @Test
    fun `copy report action fires and confirms`() {
        show(DiagnosticsTestRuntimeAdapter())

        composeTestRule.onNodeWithText("Copy Report").performClick()
        composeTestRule.waitForIdle()

        // The button relabels itself as the user-visible confirmation that the copy happened.
        composeTestRule.onNodeWithText("Report Copied!").assertIsDisplayed()
    }

    /**
     * The two directory lines name their namespace, so a reader is not left wondering why one says
     * `/data/user/0/...` and the other `/data/data/...`.
     */
    @Test
    fun `both directories render with namespace-qualified labels`() {
        show(
            DiagnosticsTestRuntimeAdapter(
                workingDirectory = TerminalWorkingDirectory(
                    guestPath = "/data/data/com.aistudio.verb.app/files/home",
                    hostPath = File("/data/user/0/com.aistudio.verb.app/files/home")
                )
            )
        )

        composeTestRule
            .onNodeWithText("Launch directory (device path): /data/user/0/com.aistudio.verb.app/files/projects/demo")
            .assertIsDisplayed()
        composeTestRule
            .onNodeWithText("Current directory (terminal path): /data/data/com.aistudio.verb.app/files/home")
            .assertIsDisplayed()
    }

    @Test
    fun `an unknown current directory renders truthfully rather than repeating the launch directory`() {
        show(DiagnosticsTestRuntimeAdapter(workingDirectory = null))

        composeTestRule
            .onNodeWithText("Current directory (terminal path): Unknown — shell integration unavailable")
            .assertIsDisplayed()
        composeTestRule
            .onNodeWithText("Launch directory (device path): /data/user/0/com.aistudio.verb.app/files/projects/demo")
            .assertIsDisplayed()
    }

    /**
     * Enlarged-font layout regression: the action row must survive a font scale that no longer lets
     * both buttons sit on one line. FlowRow wraps them instead of clipping.
     */
    @Test
    fun `actions stay reachable at an enlarged font scale`() {
        repeat(120) { i -> TerminalSessionLogger.info(LogCategory.DIAGNOSTIC, "filler $i") }
        val adapter = DiagnosticsTestRuntimeAdapter()

        composeTestRule.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(density = base.density, fontScale = 2f)
            ) {
                TerminalDiagnosticsSheet(terminalRuntime = adapter, onDismiss = {})
            }
        }

        composeTestRule.onNodeWithText("Copy Report").assertIsDisplayed().assertHasClickAction()
        composeTestRule.onNodeWithText("Clear Logs").assertIsDisplayed().assertHasClickAction()
    }
}
