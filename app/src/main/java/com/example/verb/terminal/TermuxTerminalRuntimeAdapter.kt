package com.example.verb.terminal

import android.view.KeyEvent
import android.view.MotionEvent
import androidx.compose.ui.text.TextRange
import com.termux.terminal.JNI
import com.termux.terminal.TerminalSession
import com.termux.terminal.TerminalSessionClient
import com.termux.view.TerminalView
import com.termux.view.TerminalViewClient
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList

/**
 * Production Termux Terminal Adapter implementing [TerminalRuntimeAdapter].
 * Directly manages authentic Termux [TerminalSession] and [TerminalView] instances.
 * NO silent ProcessBuilder fallbacks are performed in production.
 */
class TermuxTerminalRuntimeAdapter(
    val workingDir: File,
    val shellExecutable: String = "/system/bin/sh"
) : TerminalRuntimeAdapter, TerminalSessionClient, TerminalViewClient {

    private var session: TerminalSession? = null
    var terminalView: TerminalView? = null
        private set

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

    private val selectionListeners = CopyOnWriteArrayList<SelectionChangeListener>()

    init {
        startSession()
    }

    override fun startSession() {
        if (_isSessionActive.value && session != null) return

        _sessionState.value = TerminalSessionState.STARTING
        appendOutput("Verb Termux Session Active (${workingDir.name})\n$ ")

        val sysPath = System.getenv("PATH") ?: "/system/bin:/system/xbin"
        val envArray = arrayOf(
            "TERM=xterm-256color",
            "COLORTERM=truecolor",
            "HOME=${workingDir.absolutePath}",
            "PATH=$sysPath:/data/data/com.termux/files/usr/bin",
            "LANG=en_US.UTF-8"
        )

        val newSession = TerminalSession(
            shellPath = shellExecutable,
            cwd = workingDir.absolutePath,
            args = arrayOf("-l"),
            env = envArray,
            client = this
        )

        if (newSession.isRunning && JNI.isLoaded()) {
            session = newSession
            _isSessionActive.value = true
            _sessionState.value = TerminalSessionState.RUNNING

            terminalView?.attachSession(newSession)
        } else {
            // Truthful failure reporting in production — NO silent fallback
            _isSessionActive.value = false
            _sessionState.value = TerminalSessionState.FAILED
            appendOutput("\n[FAILED to start Termux PTY session: libtermux.so or PTY allocation failed]\n")
        }
    }

    override fun attachSession() {
        if (session != null && _isSessionActive.value) {
            _sessionState.value = TerminalSessionState.RUNNING
        } else {
            startSession()
        }
    }

    override fun sendText(text: String) {
        session?.write(text)
    }

    override fun sendCommand(cmd: String) {
        sendText("$cmd\n")
    }

    override fun sendControlKey(key: String) {
        val s = session ?: return
        when (key) {
            "ESC" -> s.write("\u001b")
            "CTRL_C" -> s.write("\u0003")
            "TAB" -> s.write("\t")
            "UP" -> s.write("\u001b[A")
            "DOWN" -> s.write("\u001b[B")
            "RIGHT" -> s.write("\u001b[C")
            "LEFT" -> s.write("\u001b[D")
            else -> s.write(key)
        }
    }

    override fun resize(rows: Int, cols: Int) {
        session?.updateSize(rows, cols)
    }

    override fun selectedText(): String {
        return terminalView?.getStoredSelectedText() ?: _activeSelectionText.value
    }

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

    override fun currentWorkingDirectory(): String {
        return workingDir.absolutePath
    }

    override fun clearBuffer() {
        _terminalOutput.value = "$ "
    }

    override fun destroy() {
        _sessionState.value = TerminalSessionState.STOPPING
        _isSessionActive.value = false
        session?.finishIfRunning()
        session = null
        selectionListeners.clear()
        _sessionState.value = TerminalSessionState.EXITED
    }

    // TerminalSessionClient callbacks
    override fun onTextChanged(changedSession: TerminalSession) {
        appendOutput(changedSession.emulator.screenBuffer.getFullText())
    }

    override fun onTitleChanged(changedSession: TerminalSession) {}

    override fun onSessionFinished(finishedSession: TerminalSession) {
        _isSessionActive.value = false
        _sessionState.value = if (finishedSession.exitCode == 0) TerminalSessionState.EXITED else TerminalSessionState.FAILED
        appendOutput("\n[Session terminated with code ${finishedSession.exitCode}]\n$ ")
    }

    override fun onClipboardText(session: TerminalSession, text: String) {
        notifySelectionChanged(TextRange(0, text.length), text)
    }

    override fun onBell(session: TerminalSession) {}

    override fun onColorsChanged(session: TerminalSession) {}

    // TerminalViewClient callbacks
    override fun onScale(scale: Float): Float = scale

    override fun onSingleTapUp(e: MotionEvent) {}

    override fun onLongPress(e: MotionEvent): Boolean {
        val selText = terminalView?.getStoredSelectedText() ?: ""
        if (selText.isNotBlank()) {
            notifySelectionChanged(TextRange(0, selText.length), selText)
        }
        return true
    }

    override fun onKeyDown(keyCode: Int, e: KeyEvent, session: TerminalSession?): Boolean = false

    override fun onKeyUp(keyCode: Int, e: KeyEvent): Boolean = false

    override fun onSelectedTextClipboard(selectedText: String) {
        if (selectedText.isNotBlank()) {
            notifySelectionChanged(TextRange(0, selectedText.length), selectedText)
        }
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
}
