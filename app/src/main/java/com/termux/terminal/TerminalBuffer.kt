package com.termux.terminal

/**
 * Grid matrix storing screen rows and scrollback buffer history.
 * Copyright (C) Termux team.
 */
class TerminalBuffer(val columns: Int, val totalRows: Int, val screenRows: Int, defaultStyle: Long) {

    val lines = Array(totalRows) { TerminalRow(columns, defaultStyle) }
    var activeTranscriptRows: Int = 0
    var screenFirstRow: Int = 0

    fun getRow(screenIndex: Int): TerminalRow {
        val actualRow = (screenFirstRow + screenIndex) % totalRows
        return lines[actualRow]
    }

    fun scrollUp(defaultStyle: Long) {
        screenFirstRow = (screenFirstRow + 1) % totalRows
        if (activeTranscriptRows < totalRows - screenRows) {
            activeTranscriptRows++
        }
        lines[(screenFirstRow + screenRows - 1) % totalRows].clear(defaultStyle)
    }

    /**
     * Extracts exact selected text across cell matrix coordinates (x1, y1) -> (x2, y2).
     */
    fun getSelectedText(x1: Int, y1: Int, x2: Int, y2: Int): String {
        val sb = StringBuilder()
        val startY = Math.min(y1, y2)
        val endY = Math.max(y1, y2)

        for (y in startY..endY) {
            val row = getRow(y)
            val startX = if (y == startY) (if (y1 <= y2) x1 else x2) else 0
            val endX = if (y == endY) (if (y1 <= y2) x2 else x1) else columns - 1
            val rowText = row.getSelectedText(startX, endX)
            sb.append(rowText)
            if (y < endY && row.lineWrap.not()) {
                sb.append("\n")
            }
        }
        return sb.toString().trimEnd()
    }

    fun getFullText(): String {
        val sb = StringBuilder()
        for (i in 0 until screenRows) {
            sb.append(getRow(i).getSelectedText(0, columns - 1))
            if (i < screenRows - 1) sb.append("\n")
        }
        return sb.toString()
    }
}
