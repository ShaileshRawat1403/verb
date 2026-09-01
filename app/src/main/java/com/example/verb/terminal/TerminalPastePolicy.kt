package com.example.verb.terminal

/**
 * Chooses how clipboard text crosses the PTY boundary.
 *
 * Interactive authentication widgets do not all consume a complete PTY write correctly. Hermes's
 * device-code field, for example, was reported to accept only the first half of an eight-character
 * code when the clipboard arrived as one burst. Short, single-line ASCII values are therefore
 * delivered with normal typing cadence. Larger, multiline or Unicode payloads remain one write:
 * pacing source files or splitting a surrogate pair would be both slow and incorrect.
 *
 * The policy examines shape only. Clipboard contents are never logged or retained.
 */
internal object TerminalPastePolicy {
    const val CHARACTER_DELAY_MS = 20L
    private const val MAX_PACED_CHARACTERS = 64

    fun chunks(text: String): List<String> =
        if (shouldPace(text)) text.map(Char::toString) else listOf(text)

    private fun shouldPace(text: String): Boolean =
        text.length in 2..MAX_PACED_CHARACTERS && text.all { it.code in 0x20..0x7e }
}
