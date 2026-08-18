package com.example.verb.terminal

import androidx.compose.ui.text.TextRange
import kotlinx.coroutines.flow.StateFlow
import java.io.File

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

    /**
     * Bounded, session-local, non-persistent command lifecycle history built from advisory
     * OSC 7/633 shell-integration markers (see [CommandExecutionTracker]). Empty when shell
     * integration hasn't loaded or produced no events yet -- this is a best-effort supplement to
     * [terminalContextState], never a replacement for it, and never forwarded to any AI provider.
     */
    val commandHistory: StateFlow<List<CommandExecutionRecord>>

    /**
     * True once the one-shot Verb shell-integration handshake (OSC 633;P;Verb=1) has been seen
     * this session -- i.e. whether [commandHistory] can be expected to populate at all. False both
     * before the guest shell has started and if shell integration never loaded; an empty
     * [commandHistory] alone can't distinguish those from "nothing has run yet".
     */
    val shellIntegrationActive: StateFlow<Boolean>

    /**
     * Single-use URL detected from a tap on the terminal canvas. Set when the user taps a line
     * containing a link so the host can launch it in a browser; consumed (reset to null) once
     * collected, keeping it effectively a one-shot event.
     */
    val urlToOpen: StateFlow<String?>

    /** Resets [urlToOpen] back to null after the host has consumed it. */
    fun consumeUrlToOpen()

    /**
     * Single-use confirmation that text was copied to the system clipboard (e.g. from a terminal
     * selection). Set on copy; consumed (reset to null) once collected, so the host can show a
     * brief "Copied to clipboard" confirmation without re-showing it on recomposition.
     */
    val clipboardCopyEvent: StateFlow<String?>

    /** Resets [clipboardCopyEvent] back to null after the host has consumed it. */
    fun consumeClipboardCopyEvent()

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

    /**
     * The HOST directory the current session's process was launched in. A static fact about how
     * the PTY was started -- it does not track the shell and never changes while a session runs.
     *
     * Deliberately named for what it is. It was previously exposed as `currentWorkingDirectory()`,
     * which made every consumer present the launch directory as though it were the shell's live
     * directory; a `cd` was never reflected anywhere. Use [currentWorkingDirectory] for the live
     * value, and never substitute this one for it.
     */
    val launchWorkingDirectory: File

    /**
     * The shell's live working directory, or null when unknown.
     *
     * Sourced only from advisory OSC 7 markers (see [CommandExecutionTracker]), so it is null
     * before the first marker of a session, null again once the session ends, and permanently null
     * wherever Verb's shell integration does not run at all -- notably the Agent Runtime, whose
     * rootfs ships no integration script. Null means unknown and must be shown as unknown; it is
     * never a cue to fall back to [launchWorkingDirectory].
     *
     * Updates once per shell prompt, so during a long-running command it holds the directory the
     * command started in.
     */
    val currentWorkingDirectory: StateFlow<TerminalWorkingDirectory?>

    /** Clears terminal buffer output */
    fun clearBuffer()

    /** Destroys and restarts the active session, allocating a fresh PTY */
    fun restartSession()

    /** Destroys active session and cleans up PTY resources */
    fun destroy()
}
