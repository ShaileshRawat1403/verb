package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Test

class TerminalPastePolicyTest {

    @Test
    fun `eight character authentication code is delivered completely and in order`() {
        val chunks = TerminalPastePolicy.chunks("12345678")

        assertEquals(listOf("1", "2", "3", "4", "5", "6", "7", "8"), chunks)
        assertEquals("12345678", chunks.joinToString(separator = ""))
    }

    @Test
    fun `short formatted authentication code is paced without modification`() {
        val chunks = TerminalPastePolicy.chunks("ABCD-EFGH")

        assertEquals("ABCD-EFGH", chunks.joinToString(separator = ""))
        assertEquals(9, chunks.size)
    }

    @Test
    fun `multiline large and unicode clipboard payloads remain atomic`() {
        assertEquals(listOf("one\ntwo"), TerminalPastePolicy.chunks("one\ntwo"))
        assertEquals(listOf("x".repeat(65)), TerminalPastePolicy.chunks("x".repeat(65)))
        assertEquals(listOf("🔐code"), TerminalPastePolicy.chunks("🔐code"))
    }
}
