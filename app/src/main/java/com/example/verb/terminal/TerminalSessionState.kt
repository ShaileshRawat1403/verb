package com.example.verb.terminal

/**
 * Explicit session state machine for Termux TTY / PTY terminal lifecycle.
 */
enum class TerminalSessionState {
    STARTING,
    RUNNING,
    EXITED,
    FAILED,
    STOPPING
}
