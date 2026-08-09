package com.example.verb.terminal

import androidx.compose.ui.text.TextRange
import kotlinx.coroutines.flow.StateFlow

/**
 * Runtime abstraction interface decoupling Verb UI and product logic from Termux PTY / TTY components.
 */
interface TerminalRuntimeAdapter {
    /** State flow reflecting current explicit session lifecycle state */
    val sessionState: StateFlow<TerminalSessionState>

    /** State flow containing accumulated terminal buffer text */
    val terminalOutput: StateFlow<String>

    /** State flow for active captured text selection */
    val activeSelectionText: StateFlow<String>

    /** State flow for active captured text selection character range */
    val activeSelectionRange: StateFlow<TextRange>

    /** Backward-compatible boolean state flow indicating active session */
    val isSessionActive: StateFlow<Boolean>

    /**
     * Metadata-only terminal context available without parsing transcript text.
     * Command boundaries, exit codes, and command output are intentionally unavailable.
     */
    val terminalContextState: StateFlow<TerminalContextState>

    /** Starts or attaches a Termux shell session */
    fun startSession()

    /** Attaches to existing active session */
    fun attachSession()

    /** Sends raw text to terminal TTY input */
    fun sendText(text: String)

    /** Sends command with trailing newline to shell TTY */
    fun sendCommand(cmd: String)

    /** Sends terminal control/ASCII key code (ESC, CTRL_C, TAB, ARROWS) */
    fun sendControlKey(key: String)

    /** Resizes terminal window grid dimensions */
    fun resize(rows: Int, cols: Int)

    /** Returns currently selected text in terminal buffer */
    fun selectedText(): String

    /** Updates selection range and notifies SelectionChangeListeners */
    fun notifySelectionChanged(selectedRange: TextRange, selectedText: String)

    /** Registers a SelectionChangeListener */
    fun addSelectionChangeListener(listener: SelectionChangeListener)

    /** Unregisters a SelectionChangeListener */
    fun removeSelectionChangeListener(listener: SelectionChangeListener)

    /** Returns current working directory path */
    fun currentWorkingDirectory(): String

    /** Clears terminal buffer output */
    fun clearBuffer()

    /** Destroys active session and cleans up PTY resources */
    fun destroy()
}
