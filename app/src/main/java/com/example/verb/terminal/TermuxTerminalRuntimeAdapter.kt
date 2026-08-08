package com.example.verb.terminal

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.view.KeyEvent
import android.view.MotionEvent
import androidx.compose.ui.text.TextRange
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

    fun bindTerminalView(view: TerminalView) {
        terminalView = view
        view.setTerminalViewClient(this)
        session?.let { view.attachSession(it) }
    }

    private val _sessionState = MutableStateFlow<TerminalSessionState>(TerminalSessionState.STARTING)
    override val sessionState: StateFlow<TerminalSessionState> = _sessionState.asStateFlow()

    private val _terminalOutput = MutableStateFlow<String>("")
    override val terminalOutput: StateFlow<String> = _terminalOutput.asStateFlow()

    private val _isSessionActive = MutableStateFlow<Boolean>(false)
    override val isSessionActive: StateFlow<Boolean> = _isSessionActive.asStateFlow()

    private val _activeSelectionText = MutableStateFlow<String>("")
    override val activeSelectionText: StateFlow<String> = _activeSelectionText.asStateFlow()

    private val _activeSelectionRange = MutableStateFlow<TextRange>(TextRange.Zero)
    /**
     * Note: The activeSelectionRange currently represents a range local to the extracted string
     * rather than exact terminal cell coordinates.
     */
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
            "PATH=$sysPath",
            "LANG=en_US.UTF-8"
        )

        try {
            Class.forName("com.termux.terminal.JNI")
            val newSession = TerminalSession(
                shellExecutable,
                workingDir.absolutePath,
                arrayOf("-l"),
                envArray,
                2000,
                this
            )
            
            // Initialize the emulator immediately for headless execution
            newSession.updateSize(80, 24, 0, 0)
            
            if (newSession.isRunning) {
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
        } catch (t: Throwable) {
            _isSessionActive.value = false
            _sessionState.value = TerminalSessionState.FAILED
            appendOutput("\n[FAILED to start Termux PTY session: ${t.message}]\n")
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
        session?.updateSize(cols, rows, 0, 0)
    }

    override fun selectedText(): String {
        return terminalView?.storedSelectedText ?: _activeSelectionText.value
    }

    override fun notifySelectionChanged(selectedRange: TextRange, selectedText: String) {
        // The selection range is documented as local to the extracted string
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
        appendOutput(changedSession.emulator.screen.transcriptText)
    }

    override fun onTitleChanged(changedSession: TerminalSession) {}

    override fun onSessionFinished(finishedSession: TerminalSession) {
        _isSessionActive.value = false
        _sessionState.value = if (finishedSession.exitStatus == 0) TerminalSessionState.EXITED else TerminalSessionState.FAILED
        appendOutput("\n[Session terminated with code ${finishedSession.exitStatus}]\n$ ")
    }

    override fun onCopyTextToClipboard(session: TerminalSession, text: String) {
        terminalView?.context?.let { ctx ->
            val clipboard = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            clipboard.setPrimaryClip(ClipData.newPlainText("Termux Selection", text))
        }
    }

    override fun onPasteTextFromClipboard(session: TerminalSession?) {
        val s = session ?: this.session
        terminalView?.context?.let { ctx ->
            val clipboard = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            val clipData = clipboard.primaryClip
            if (clipData != null && clipData.itemCount > 0) {
                val text = clipData.getItemAt(0).coerceToText(ctx).toString()
                s?.write(text)
            }
        }
    }
    
    override fun onBell(session: TerminalSession) {}
    
    override fun onColorsChanged(session: TerminalSession) {}
    
    override fun onTerminalCursorStateChange(state: Boolean) {}
    
    override fun setTerminalShellPid(session: TerminalSession, pid: Int) {}
    
    override fun getTerminalCursorStyle(): Int = 0

    // TerminalViewClient callbacks
    override fun onScale(scale: Float): Float = scale
    
    override fun onSingleTapUp(e: MotionEvent) {}
    
    override fun onInspectText(text: String) {
        // Notify Semantic Lens about inspected text. The selection range is local to the extracted string.
        notifySelectionChanged(TextRange(0, text.length), text)
    }

    override fun onLongPress(e: MotionEvent): Boolean {
        return false
    }

    override fun shouldBackButtonBeMappedToEscape(): Boolean = false
    override fun shouldEnforceCharBasedInput(): Boolean = false
    override fun shouldUseCtrlSpaceWorkaround(): Boolean = false
    override fun isTerminalViewSelected(): Boolean = true
    override fun copyModeChanged(copyMode: Boolean) {}
    
    override fun onKeyDown(keyCode: Int, e: KeyEvent, session: TerminalSession?): Boolean = false
    override fun onKeyUp(keyCode: Int, e: KeyEvent): Boolean = false
    override fun readControlKey(): Boolean = false
    override fun readAltKey(): Boolean = false
    override fun readShiftKey(): Boolean = false
    override fun readFnKey(): Boolean = false
    override fun onCodePoint(codePoint: Int, ctrlDown: Boolean, session: TerminalSession?): Boolean = false
    
    override fun onEmulatorSet() {}
    
    override fun logError(tag: String, message: String) {}
    override fun logWarn(tag: String, message: String) {}
    override fun logInfo(tag: String, message: String) {}
    override fun logDebug(tag: String, message: String) {}
    override fun logVerbose(tag: String, message: String) {}
    override fun logStackTraceWithMessage(tag: String, message: String, e: Exception) {}
    override fun logStackTrace(tag: String, e: Exception) {}

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
