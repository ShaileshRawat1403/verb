package com.example.verb.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.text.TextRange
import com.example.verb.terminal.CommandExecutionRecord
import com.example.verb.terminal.SelectionChangeListener
import com.example.verb.terminal.TerminalContextState
import com.example.verb.terminal.TerminalRuntimeAdapter
import com.example.verb.terminal.TerminalSessionState
import com.example.verb.terminal.TerminalWorkingDirectory
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.rules.TemporaryFolder
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

/**
 * Test adapter whose live working directory can be driven directly, so these tests can emit a
 * `cd` the same way a real OSC 7 marker would.
 */
private class ExplorerTestRuntimeAdapter(
    override val launchWorkingDirectory: File,
    initialWorkingDirectory: TerminalWorkingDirectory? = null
) : TerminalRuntimeAdapter {
    private val _currentWorkingDirectory = MutableStateFlow(initialWorkingDirectory)
    override val currentWorkingDirectory: StateFlow<TerminalWorkingDirectory?> =
        _currentWorkingDirectory.asStateFlow()

    fun emitWorkingDirectory(value: TerminalWorkingDirectory?) {
        _currentWorkingDirectory.value = value
    }

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
class FileExplorerDrawerTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun path() = composeTestRule.onNodeWithTag("file_explorer_current_path")

    @Test
    fun `opens at the mapped live working directory`() {
        val launch = temporaryFolder.newFolder("launch")
        val live = temporaryFolder.newFolder("live-project")
        val adapter = ExplorerTestRuntimeAdapter(
            launchWorkingDirectory = launch,
            initialWorkingDirectory = TerminalWorkingDirectory("/guest/live-project", live)
        )

        composeTestRule.setContent {
            FileExplorerDrawer(terminalRuntime = adapter, isDark = true, onFileClicked = {})
        }

        path().assertTextEquals(live.absolutePath)
    }

    /**
     * `cd /system` in the guest produces a real guest path with no host mapping. The browser must
     * fall back to the launch directory -- never `File("/system")`, and never `/`.
     */
    @Test
    fun `an unmappable live working directory falls back to the launch directory`() {
        val launch = temporaryFolder.newFolder("launch")
        val adapter = ExplorerTestRuntimeAdapter(
            launchWorkingDirectory = launch,
            initialWorkingDirectory = TerminalWorkingDirectory("/system", hostPath = null)
        )

        composeTestRule.setContent {
            FileExplorerDrawer(terminalRuntime = adapter, isDark = true, onFileClicked = {})
        }

        path().assertTextEquals(launch.absolutePath)
    }

    @Test
    fun `an unknown live working directory falls back to the launch directory`() {
        val launch = temporaryFolder.newFolder("launch")
        val adapter = ExplorerTestRuntimeAdapter(launchWorkingDirectory = launch)

        composeTestRule.setContent {
            FileExplorerDrawer(terminalRuntime = adapter, isDark = true, onFileClicked = {})
        }

        path().assertTextEquals(launch.absolutePath)
    }

    /**
     * The core UX guarantee: the snapshot is taken when the browser opens, and nothing the shell
     * does afterwards moves it. A user who has navigated somewhere must not be yanked away by a
     * `cd` typed in the terminal behind the sheet.
     */
    @Test
    fun `a later working directory emission does not move an open browser`() {
        val launch = temporaryFolder.newFolder("launch")
        val opened = temporaryFolder.newFolder("opened-here")
        val movedTo = temporaryFolder.newFolder("shell-moved-here")
        val adapter = ExplorerTestRuntimeAdapter(
            launchWorkingDirectory = launch,
            initialWorkingDirectory = TerminalWorkingDirectory("/guest/opened-here", opened)
        )

        composeTestRule.setContent {
            FileExplorerDrawer(terminalRuntime = adapter, isDark = true, onFileClicked = {})
        }
        path().assertTextEquals(opened.absolutePath)

        adapter.emitWorkingDirectory(TerminalWorkingDirectory("/guest/shell-moved-here", movedTo))
        composeTestRule.waitForIdle()

        path().assertTextEquals(opened.absolutePath)
    }

    @Test
    fun `user navigation inside the browser survives a later working directory emission`() {
        val launch = temporaryFolder.newFolder("launch")
        val opened = temporaryFolder.newFolder("opened-here")
        val movedTo = temporaryFolder.newFolder("shell-moved-here")
        val adapter = ExplorerTestRuntimeAdapter(
            launchWorkingDirectory = launch,
            initialWorkingDirectory = TerminalWorkingDirectory("/guest/opened-here", opened)
        )

        composeTestRule.setContent {
            FileExplorerDrawer(terminalRuntime = adapter, isDark = true, onFileClicked = {})
        }

        // The user navigates away using the browser's own Root shortcut.
        composeTestRule.onNodeWithText("Root").performClick()
        composeTestRule.waitForIdle()
        path().assertTextEquals("/")

        adapter.emitWorkingDirectory(TerminalWorkingDirectory("/guest/shell-moved-here", movedTo))
        composeTestRule.waitForIdle()

        path().assertTextEquals("/")
    }

    /**
     * Reopening is the supported way to resync, so the sheet's remount must pick up whatever the
     * shell's directory is at that moment -- not the value captured the first time it opened.
     */
    @Test
    fun `reopening the browser takes a fresh snapshot of the working directory`() {
        val launch = temporaryFolder.newFolder("launch")
        val firstOpen = temporaryFolder.newFolder("first-open")
        val secondOpen = temporaryFolder.newFolder("second-open")
        val adapter = ExplorerTestRuntimeAdapter(
            launchWorkingDirectory = launch,
            initialWorkingDirectory = TerminalWorkingDirectory("/guest/first-open", firstOpen)
        )
        var open by mutableStateOf(true)

        composeTestRule.setContent {
            if (open) {
                FileExplorerDrawer(terminalRuntime = adapter, isDark = true, onFileClicked = {})
            }
        }
        path().assertTextEquals(firstOpen.absolutePath)

        // Close the sheet, let the shell move, then reopen.
        open = false
        composeTestRule.waitForIdle()
        adapter.emitWorkingDirectory(TerminalWorkingDirectory("/guest/second-open", secondOpen))
        open = true
        composeTestRule.waitForIdle()

        path().assertTextEquals(secondOpen.absolutePath)
    }

    /**
     * The Shell shortcut is a user-initiated resync, so it reads the live value at tap time -- and
     * still refuses an unmapped guest path rather than navigating to a host path it does not name.
     */
    @Test
    fun `the shell shortcut resyncs to the live directory when tapped`() {
        val launch = temporaryFolder.newFolder("launch")
        val opened = temporaryFolder.newFolder("opened-here")
        val movedTo = temporaryFolder.newFolder("shell-moved-here")
        val adapter = ExplorerTestRuntimeAdapter(
            launchWorkingDirectory = launch,
            initialWorkingDirectory = TerminalWorkingDirectory("/guest/opened-here", opened)
        )

        composeTestRule.setContent {
            FileExplorerDrawer(terminalRuntime = adapter, isDark = true, onFileClicked = {})
        }

        adapter.emitWorkingDirectory(TerminalWorkingDirectory("/guest/shell-moved-here", movedTo))
        composeTestRule.onNodeWithText("Shell").performClick()
        composeTestRule.waitForIdle()

        path().assertTextEquals(movedTo.absolutePath)
    }

    @Test
    fun `the shell shortcut falls back to the launch directory for an unmapped path`() {
        val launch = temporaryFolder.newFolder("launch")
        val opened = temporaryFolder.newFolder("opened-here")
        val adapter = ExplorerTestRuntimeAdapter(
            launchWorkingDirectory = launch,
            initialWorkingDirectory = TerminalWorkingDirectory("/guest/opened-here", opened)
        )

        composeTestRule.setContent {
            FileExplorerDrawer(terminalRuntime = adapter, isDark = true, onFileClicked = {})
        }

        adapter.emitWorkingDirectory(TerminalWorkingDirectory("/system", hostPath = null))
        composeTestRule.onNodeWithText("Shell").performClick()
        composeTestRule.waitForIdle()

        path().assertTextEquals(launch.absolutePath)
    }
}
