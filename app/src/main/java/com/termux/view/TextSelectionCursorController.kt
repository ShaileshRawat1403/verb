package com.termux.view

/**
 * Handles cell range text selection boundaries and touch cursor positioning for TerminalView.
 * Copyright (C) Termux team.
 */
class TextSelectionCursorController(val terminalView: TerminalView) {

    var selX1: Int = -1
    var selY1: Int = -1
    var selX2: Int = -1
    var selY2: Int = -1
    var isSelecting: Boolean = false
        private set

    fun setSelection(x1: Int, y1: Int, x2: Int, y2: Int) {
        selX1 = x1
        selY1 = y1
        selX2 = x2
        selY2 = y2
        isSelecting = true
        terminalView.invalidate()
    }

    fun clearSelection() {
        if (isSelecting) {
            isSelecting = false
            selX1 = -1
            selY1 = -1
            selX2 = -1
            selY2 = -1
            terminalView.invalidate()
        }
    }

    fun getSelectedText(): String {
        if (!isSelecting) return ""
        val session = terminalView.currentSession ?: return ""
        return session.emulator.screenBuffer.getSelectedText(selX1, selY1, selX2, selY2)
    }
}
