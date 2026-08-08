package com.example.verb.terminal

import androidx.compose.ui.text.TextRange
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList

/**
 * High-performance Termux Terminal Runtime implementing [TerminalRuntimeAdapter].
 * Manages Termux TTY sessions, PTY bindings, explicit session states, and text selection observers.
 */
class TerminalRuntime(private val workingDir: File) : TerminalRuntimeAdapter {

    private var currentSession: TermuxTerminalSession? = null

    private val _sessionState = MutableStateFlow<TerminalSessionState>(TerminalSessionState.STARTING)
    override val sessionState: StateFlow<TerminalSessionState> = _sessionState.asStateFlow()

    private val _terminalOutput = MutableStateFlow<String>("")
    override val terminalOutput: StateFlow<String> = _terminalOutput.asStateFlow()

    private val _isSessionActive = MutableStateFlow<Boolean>(false)
    override val isSessionActive: StateFlow<Boolean> = _isSessionActive.asStateFlow()

    private val _activeSelectionText = MutableStateFlow<String>("")
    override val activeSelectionText: StateFlow<String> = _activeSelectionText.asStateFlow()

    private val _activeSelectionRange = MutableStateFlow<TextRange>(TextRange.Zero)
    override val activeSelectionRange: StateFlow<TextRange> = _activeSelectionRange.asStateFlow()

    private val selectionChangeListeners = CopyOnWriteArrayList<SelectionChangeListener>()

    init {
        startSession()
    }

    override fun addSelectionChangeListener(listener: SelectionChangeListener) {
        if (!selectionChangeListeners.contains(listener)) {
            selectionChangeListeners.add(listener)
        }
    }

    override fun removeSelectionChangeListener(listener: SelectionChangeListener) {
        selectionChangeListeners.remove(listener)
    }

    override fun notifySelectionChanged(selectedRange: TextRange, selectedText: String) {
        _activeSelectionRange.value = selectedRange
        _activeSelectionText.value = selectedText

        for (listener in selectionChangeListeners) {
            listener.onSelectionChanged(selectedRange, selectedText)
        }
    }

    override fun selectedText(): String {
        return _activeSelectionText.value
    }

    override fun startSession() {
        if (_isSessionActive.value && currentSession != null) return

        _sessionState.value = TerminalSessionState.STARTING
        appendOutput("Verb Terminal Session Active (${workingDir.name}).\n$ ")

        val session = TermuxTerminalSession(
            workingDir = workingDir,
            shellExecutable = "/system/bin/sh",
            rows = 24,
            cols = 80,
            onOutputReceived = { text ->
                appendOutput(text)
            },
            onSessionTerminated = { exitCode ->
                _isSessionActive.value = false
                _sessionState.value = if (exitCode == 0) TerminalSessionState.EXITED else TerminalSessionState.FAILED
                appendOutput("\n[Session terminated with code $exitCode]\n$ ")
            }
        )

        currentSession = session
        _isSessionActive.value = true
        _sessionState.value = TerminalSessionState.RUNNING
        session.startSession()
    }

    override fun attachSession() {
        if (currentSession != null && _isSessionActive.value) {
            _sessionState.value = TerminalSessionState.RUNNING
        } else {
            startSession()
        }
    }

    override fun sendText(text: String) {
        currentSession?.writeInput(text)
    }

    override fun sendCommand(cmd: String) {
        sendText("$cmd\n")
    }

    override fun sendControlKey(key: String) {
        currentSession?.sendControlKey(key)
    }

    override fun resize(rows: Int, cols: Int) {
        currentSession?.updateWindowSize(rows, cols)
    }

    override fun currentWorkingDirectory(): String {
        return workingDir.absolutePath
    }

    override fun clearBuffer() {
        _terminalOutput.value = "$ "
    }

    private fun appendOutput(text: String) {
        val current = _terminalOutput.value
        val updated = if (current.length > 50_000) {
            current.takeLast(25_000) + text
        } else {
            current + text
        }
        _terminalOutput.value = updated
    }

    override fun destroy() {
        _sessionState.value = TerminalSessionState.STOPPING
        _isSessionActive.value = false
        currentSession?.destroySession()
        currentSession = null
        selectionChangeListeners.clear()
        _sessionState.value = TerminalSessionState.EXITED
    }
}
