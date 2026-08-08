package com.example.verb.terminal

import androidx.compose.ui.text.TextRange
import kotlinx.coroutines.flow.StateFlow
import java.io.File

/**
 * Universal Terminal Runtime implementing [TerminalRuntimeAdapter].
 * Automatically selects [TermuxTerminalRuntimeAdapter] in native Android environments or [FakeTerminalRuntimeAdapter] in test/JVM environments.
 */
class TerminalRuntime(
    private val workingDir: File,
    useFakeForTesting: Boolean = false
) : TerminalRuntimeAdapter {

    private val delegate: TerminalRuntimeAdapter = if (useFakeForTesting) {
        FakeTerminalRuntimeAdapter(workingDir)
    } else {
        TermuxTerminalRuntimeAdapter(workingDir)
    }

    override val sessionState: StateFlow<TerminalSessionState> get() = delegate.sessionState
    override val terminalOutput: StateFlow<String> get() = delegate.terminalOutput
    override val activeSelectionText: StateFlow<String> get() = delegate.activeSelectionText
    override val activeSelectionRange: StateFlow<TextRange> get() = delegate.activeSelectionRange
    override val isSessionActive: StateFlow<Boolean> get() = delegate.isSessionActive

    override fun startSession() = delegate.startSession()
    override fun attachSession() = delegate.attachSession()
    override fun sendText(text: String) = delegate.sendText(text)
    override fun sendCommand(cmd: String) = delegate.sendCommand(cmd)
    override fun sendControlKey(key: String) = delegate.sendControlKey(key)
    override fun resize(rows: Int, cols: Int) = delegate.resize(rows, cols)
    override fun selectedText(): String = delegate.selectedText()
    override fun notifySelectionChanged(selectedRange: TextRange, selectedText: String) =
        delegate.notifySelectionChanged(selectedRange, selectedText)

    override fun addSelectionChangeListener(listener: SelectionChangeListener) =
        delegate.addSelectionChangeListener(listener)

    override fun removeSelectionChangeListener(listener: SelectionChangeListener) =
        delegate.removeSelectionChangeListener(listener)

    override fun currentWorkingDirectory(): String = delegate.currentWorkingDirectory()
    override fun clearBuffer() = delegate.clearBuffer()
    override fun destroy() = delegate.destroy()
}
