package com.termux.terminal

/**
 * Client callback interface for Termux TerminalSession event listeners.
 * Copyright (C) Termux team.
 */
interface TerminalSessionClient {
    fun onTextChanged(changedSession: TerminalSession)
    fun onTitleChanged(changedSession: TerminalSession)
    fun onSessionFinished(finishedSession: TerminalSession)
    fun onClipboardText(session: TerminalSession, text: String)
    fun onBell(session: TerminalSession)
    fun onColorsChanged(session: TerminalSession)
}
