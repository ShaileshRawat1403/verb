package com.example.verb.terminal

import androidx.compose.ui.text.TextRange
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList

/**
 * Explicit test/headless implementation of [TerminalRuntimeAdapter].
 * Used for fast JVM unit testing, verification, and mock terminal interactions.
 */
class FakeTerminalRuntimeAdapter(
    val workingDir: File
) : TerminalRuntimeAdapter {

    private val _sessionState = MutableStateFlow<TerminalSessionState>(TerminalSessionState.STARTING)
    override val sessionState: StateFlow<TerminalSessionState> = _sessionState.asStateFlow()

    private val _terminalOutput = MutableStateFlow<String>("")
    override val terminalOutput: StateFlow<String> = _terminalOutput.asStateFlow()

    private val _isSessionActive = MutableStateFlow<Boolean>(false)
    override val isSessionActive: StateFlow<Boolean> = _isSessionActive.asStateFlow()

    private val _terminalContextState = MutableStateFlow(TerminalContextState())
    override val terminalContextState: StateFlow<TerminalContextState> = _terminalContextState.asStateFlow()

    private val commandTracker = CommandExecutionTracker()
    override val commandHistory: StateFlow<List<CommandExecutionRecord>> = commandTracker.history
    override val shellIntegrationActive: StateFlow<Boolean> = commandTracker.shellIntegrationActive

    override val launchWorkingDirectory: File get() = workingDir

    /**
     * Always null: the fake never runs a shell, so it never receives an OSC 7 marker. It reports
     * the live directory as unknown rather than echoing [launchWorkingDirectory], so headless and
     * unit-test surfaces exercise the same "unknown" path a real Agent Runtime session takes.
     */
    private val _currentWorkingDirectory = MutableStateFlow<TerminalWorkingDirectory?>(null)
    override val currentWorkingDirectory: StateFlow<TerminalWorkingDirectory?> =
        _currentWorkingDirectory.asStateFlow()

    private val _urlToOpen = MutableStateFlow<String?>(null)
    override val urlToOpen: StateFlow<String?> = _urlToOpen.asStateFlow()
    override fun consumeUrlToOpen() {
        _urlToOpen.value = null
    }

    private val _clipboardCopyEvent = MutableStateFlow<String?>(null)
    override val clipboardCopyEvent: StateFlow<String?> = _clipboardCopyEvent.asStateFlow()
    override fun consumeClipboardCopyEvent() {
        _clipboardCopyEvent.value = null
    }

    private val _activeSelectionText = MutableStateFlow<String>("")
    override val activeSelectionText: StateFlow<String> = _activeSelectionText.asStateFlow()

    private val _activeSelectionRange = MutableStateFlow<TextRange>(TextRange.Zero)
    override val activeSelectionRange: StateFlow<TextRange> = _activeSelectionRange.asStateFlow()

    private val selectionListeners = CopyOnWriteArrayList<SelectionChangeListener>()

    init {
        startSession()
    }

    override fun startSession() {
        if (_isSessionActive.value) return
        _sessionState.value = TerminalSessionState.STARTING
        _terminalOutput.value = "Verb Terminal Session Active (${workingDir.name}).\n$ "
        _isSessionActive.value = true
        _sessionState.value = TerminalSessionState.RUNNING
        _terminalContextState.value = TerminalContextState(
            capability = TerminalContextCapability.SESSION_ONLY,
            sessionId = "fake-session",
            alternateScreenState = AlternateScreenState.UNKNOWN
        )
    }

    override fun attachSession() {
        if (_isSessionActive.value) {
            _sessionState.value = TerminalSessionState.RUNNING
        } else {
            startSession()
        }
    }

    override fun sendText(text: String) {
        if (_isSessionActive.value) {
            _terminalOutput.value += text
        }
    }

    override fun sendCommand(cmd: String) {
        sendText("$cmd\n$ ")
    }

    override fun sendControlKey(key: String) {
        sendText("^$key\n$ ")
    }

    override fun resize(rows: Int, cols: Int) {}

    override fun selectedText(): String = _activeSelectionText.value

    override fun notifySelectionChanged(selectedRange: TextRange, selectedText: String) {
        _activeSelectionRange.value = selectedRange
        _activeSelectionText.value = selectedText

        for (listener in selectionListeners) {
            listener.onSelectionChanged(selectedRange, selectedText)
        }
    }

    override fun addSelectionChangeListener(listener: SelectionChangeListener) {
        if (!selectionListeners.contains(listener)) {
            selectionListeners.add(listener)
        }
    }

    override fun removeSelectionChangeListener(listener: SelectionChangeListener) {
        selectionListeners.remove(listener)
    }

    override fun clearBuffer() {
        _terminalOutput.value = "$ "
    }

    override fun restartSession() {
        destroy()
        startSession()
    }

    override fun destroy() {
        _sessionState.value = TerminalSessionState.STOPPING
        _isSessionActive.value = false
        selectionListeners.clear()
        _sessionState.value = TerminalSessionState.EXITED
        _terminalContextState.value = TerminalContextState()
    }
}
