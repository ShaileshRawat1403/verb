package com.termux.terminal

/**
 * Packed integer representation of terminal text styling and colors.
 * Copyright (C) Termux team.
 */
object TextStyle {
    const val CHARACTER_ATTRIBUTE_BOLD = 1
    const val CHARACTER_ATTRIBUTE_ITALIC = 1 shl 1
    const val CHARACTER_ATTRIBUTE_UNDERLINE = 1 shl 2
    const val CHARACTER_ATTRIBUTE_BLINK = 1 shl 3
    const val CHARACTER_ATTRIBUTE_INVERSE = 1 shl 4
    const val CHARACTER_ATTRIBUTE_DIM = 1 shl 5

    const val COLOR_INDEX_FOREGROUND = 256
    const val COLOR_INDEX_BACKGROUND = 257
    const val COLOR_INDEX_CURSOR = 258

    fun encode(fg: Int, bg: Int, flags: Int): Long {
        return (fg.toLong() and 0xFFFFFFL) or ((bg.toLong() and 0xFFFFFFL) shl 24) or ((flags.toLong() and 0xFFFFL) shl 48)
    }

    fun decodeForeground(style: Long): Int = (style and 0xFFFFFFL).toInt()
    fun decodeBackground(style: Long): Int = ((style shr 24) and 0xFFFFFFL).toInt()
    fun decodeFlags(style: Long): Int = ((style shr 48) and 0xFFFFL).toInt()
}
