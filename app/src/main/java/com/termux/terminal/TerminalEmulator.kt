package com.termux.terminal

import android.util.Log

/**
 * Authentic Termux Terminal Emulator state machine and VT100/ANSI escape code parser.
 * Copyright (C) Termux team.
 */
class TerminalEmulator(
    var columns: Int,
    var rows: Int,
    var transcriptRows: Int,
    val session: TerminalSession
) {

    val screenBuffer = TerminalBuffer(columns, rows + transcriptRows, rows, 0L)
    var cursorX = 0
    var cursorY = 0
    private var defaultStyle = TextStyle.encode(TextStyle.COLOR_INDEX_FOREGROUND, TextStyle.COLOR_INDEX_BACKGROUND, 0)

    fun updateSize(newCols: Int, newRows: Int) {
        if (newCols > 0 && newRows > 0) {
            this.columns = newCols
            this.rows = newRows
        }
    }

    /**
     * Appends and parses VT100/ANSI stream data into cell buffer.
     */
    fun append(buffer: ByteArray, length: Int) {
        var i = 0
        while (i < length) {
            val b = buffer[i].toInt() and 0xFF
            when (b) {
                0x07 -> session.client?.onBell(session) // BELL
                0x08 -> { // Backspace
                    if (cursorX > 0) cursorX--
                }
                0x09 -> { // TAB
                    cursorX = (cursorX + 8) and 7.inv()
                    if (cursorX >= columns) cursorX = columns - 1
                }
                0x0A, 0x0B, 0x0C -> { // Line Feed / Newline
                    cursorY++
                    if (cursorY >= rows) {
                        cursorY = rows - 1
                        screenBuffer.scrollUp(defaultStyle)
                    }
                }
                0x0D -> { // Carriage Return
                    cursorX = 0
                }
                0x1B -> { // ESC Escape sequence
                    if (i + 1 < length && buffer[i + 1] == '['.code.toByte()) {
                        i += 2
                        // Parse CSI parameters
                        val csiStart = i
                        while (i < length && (buffer[i].toInt() in 0x30..0x3F)) {
                            i++
                        }
                        if (i < length) {
                            val cmd = buffer[i].toInt().toChar()
                            handleCsiCommand(cmd, String(buffer, csiStart, i - csiStart))
                        }
                    }
                }
                else -> { // Standard printable character or UTF-8 byte
                    if (b >= 32) {
                        val row = screenBuffer.getRow(cursorY)
                        row.setChar(cursorX, b, defaultStyle)
                        cursorX++
                        if (cursorX >= columns) {
                            cursorX = 0
                            row.lineWrap = true
                            cursorY++
                            if (cursorY >= rows) {
                                cursorY = rows - 1
                                screenBuffer.scrollUp(defaultStyle)
                            }
                        }
                    }
                }
            }
            i++
        }
        session.client?.onTextChanged(session)
    }

    private fun handleCsiCommand(cmd: Char, paramsStr: String) {
        val params = paramsStr.split(";").mapNotNull { it.toIntOrNull() }
        when (cmd) {
            'H', 'f' -> { // Cursor position
                val r = (params.getOrNull(0) ?: 1) - 1
                val c = (params.getOrNull(1) ?: 1) - 1
                cursorY = Math.max(0, Math.min(rows - 1, r))
                cursorX = Math.max(0, Math.min(columns - 1, c))
            }
            'J' -> { // Erase in display
                val mode = params.getOrNull(0) ?: 0
                if (mode == 2 || mode == 3) {
                    for (r in 0 until rows) {
                        screenBuffer.getRow(r).clear(defaultStyle)
                    }
                    cursorX = 0
                    cursorY = 0
                }
            }
            'm' -> { // Character Attributes (SGR)
                if (params.isEmpty() || params.contains(0)) {
                    defaultStyle = TextStyle.encode(TextStyle.COLOR_INDEX_FOREGROUND, TextStyle.COLOR_INDEX_BACKGROUND, 0)
                } else {
                    var flags = TextStyle.decodeFlags(defaultStyle)
                    var fg = TextStyle.decodeForeground(defaultStyle)
                    var bg = TextStyle.decodeBackground(defaultStyle)
                    for (p in params) {
                        when (p) {
                            1 -> flags = flags or TextStyle.CHARACTER_ATTRIBUTE_BOLD
                            4 -> flags = flags or TextStyle.CHARACTER_ATTRIBUTE_UNDERLINE
                            7 -> flags = flags or TextStyle.CHARACTER_ATTRIBUTE_INVERSE
                            in 30..37 -> fg = p - 30
                            39 -> fg = TextStyle.COLOR_INDEX_FOREGROUND
                            in 40..47 -> bg = p - 40
                            49 -> bg = TextStyle.COLOR_INDEX_BACKGROUND
                        }
                    }
                    defaultStyle = TextStyle.encode(fg, bg, flags)
                }
            }
        }
    }
}
