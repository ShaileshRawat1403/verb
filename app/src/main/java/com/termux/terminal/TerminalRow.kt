package com.termux.terminal

/**
 * Single line in terminal buffer containing cell code points and style attributes.
 * Copyright (C) Termux team.
 */
class TerminalRow(val columns: Int, defaultStyle: Long) {
    val text = IntArray(columns) { ' '.code }
    val style = LongArray(columns) { defaultStyle }
    var lineWrap: Boolean = false

    fun clear(defaultStyle: Long) {
        text.fill(' '.code)
        style.fill(defaultStyle)
        lineWrap = false
    }

    fun setChar(column: Int, codePoint: Int, charStyle: Long) {
        if (column in 0 until columns) {
            text[column] = codePoint
            style[column] = charStyle
        }
    }

    fun getSelectedText(startCol: Int, endCol: Int): String {
        val sb = StringBuilder()
        val minCol = Math.max(0, startCol)
        val maxCol = Math.min(columns - 1, endCol)
        for (i in minCol..maxCol) {
            val cp = text[i]
            if (cp > 0) {
                sb.appendCodePoint(cp)
            }
        }
        return sb.toString()
    }
}
