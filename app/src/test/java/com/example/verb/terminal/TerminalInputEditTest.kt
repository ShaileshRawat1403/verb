package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * What the input dock sends to the PTY as someone types.
 *
 * The field mirrors its characters onto the line the PTY owns, so every keystroke is a real write
 * to whatever program is running. The cost of getting this wrong is visible: the old
 * implementation erased and retyped the whole line whenever a change was neither a pure append nor
 * a pure truncation, and a predictive keyboard makes that the common case rather than the rare one.
 * Inside Antigravity's composer the text could be watched typing, deleting and retyping itself.
 */
class TerminalInputEditTest {

    private fun edit(old: String, new: String) = terminalInputEdit(old, new)

    @Test
    fun `typing a character sends only that character`() {
        assertEquals(TerminalInputEdit(backspaces = 0, textToSend = "o"), edit("hell", "hello"))
    }

    @Test
    fun `deleting a character sends one backspace and no text`() {
        assertEquals(TerminalInputEdit(backspaces = 1, textToSend = ""), edit("hello", "hell"))
    }

    /**
     * The regression. Gboard rewrites the whole composing word, so `hel` -> `hey` arrives as a
     * wholesale replacement. It used to cost three backspaces and three characters; the shared
     * prefix makes it one and one.
     */
    @Test
    fun `a mid-word correction rewrites only the tail that differs`() {
        assertEquals(TerminalInputEdit(backspaces = 1, textToSend = "y"), edit("hel", "hey"))
    }

    @Test
    fun `replacing the whole word still only sends what differs`() {
        assertEquals(TerminalInputEdit(backspaces = 5, textToSend = "world"), edit("hello", "world"))
    }

    @Test
    fun `clearing the field sends backspaces and nothing else`() {
        assertEquals(TerminalInputEdit(backspaces = 5, textToSend = ""), edit("hello", ""))
    }

    @Test
    fun `typing into an empty field sends the text and no backspaces`() {
        assertEquals(TerminalInputEdit(backspaces = 0, textToSend = "ls"), edit("", "ls"))
    }

    @Test
    fun `no change sends nothing at all`() {
        assertEquals(TerminalInputEdit(backspaces = 0, textToSend = ""), edit("ls", "ls"))
    }

    /**
     * Autocorrect committing a word plus its trailing space is an append, not a rewrite, and must
     * not disturb the line.
     */
    @Test
    fun `accepting a suggestion that extends the word is a pure append`() {
        assertEquals(TerminalInputEdit(backspaces = 0, textToSend = "lo there"), edit("hel", "hello there"))
    }

    /** Pasting over a selection shares no prefix and must not send more than it has to. */
    @Test
    fun `a paste that shares no prefix sends the whole replacement once`() {
        assertEquals(TerminalInputEdit(backspaces = 3, textToSend = "git status"), edit("abc", "git status"))
    }

    /**
     * The property that matters: the edit never sends more characters than the strings differ by.
     * Asserted over the cases above rather than stated in a comment.
     */
    @Test
    fun `an edit never sends more than the difference between the two strings`() {
        listOf(
            "" to "ls",
            "hel" to "hey",
            "hello" to "hell",
            "hello" to "world",
            "abc" to "git status",
            "ls" to "ls"
        ).forEach { (old, new) ->
            val e = edit(old, new)
            val shared = old.commonPrefixWith(new).length
            assertEquals("backspaces for '$old' -> '$new'", old.length - shared, e.backspaces)
            assertEquals("text for '$old' -> '$new'", new.length - shared, e.textToSend.length)
        }
    }
}
