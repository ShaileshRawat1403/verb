package com.termux.terminal

/**
 * East Asian character width calculator for terminal cell alignment.
 * Copyright (C) Termux team.
 */
object WcWidth {
    fun width(ucs: Int): Int {
        if (ucs == 0) return 0
        if (ucs < 32 || (ucs in 0x7f..0x9f)) return 0
        if (ucs in 0x0300..0x036F) return 0 // Combining diacritical marks
        if (ucs in 0x1100..0x115F ||
            ucs == 0x2329 || ucs == 0x232A ||
            (ucs in 0x2E80..0xA4CF && ucs != 0x303F) ||
            ucs in 0xAC00..0xD7A3 ||
            ucs in 0xF900..0xFAFF ||
            ucs in 0xFE10..0xFE19 ||
            ucs in 0xFE30..0xFE6F ||
            ucs in 0xFF00..0xFF60 ||
            ucs in 0xFFE0..0xFFE6 ||
            ucs in 0x20000..0x2FFFD ||
            ucs in 0x30000..0x3FFFD
        ) {
            return 2
        }
        return 1
    }
}
