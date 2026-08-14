package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TerminalUrlMatcherTest {

    @Test
    fun `findUrlAt returns url when tap lands on the link`() {
        val line = "==> Open https://chatgpt.com/codex/activate/abcd and enter code"
        val url = findUrlAt(line, line.indexOf("https://"))
        assertEquals("https://chatgpt.com/codex/activate/abcd", url)
    }

    @Test
    fun `findUrlAt preserves leading whitespace coordinates`() {
        val line = "          https://example.com/a?b=c done"
        val url = findUrlAt(line, line.indexOf("https://"))
        assertEquals("https://example.com/a?b=c", url)
    }

    @Test
    fun `findUrlAt returns null when no url is on the line`() {
        assertNull(findUrlAt("~ $ git --version", 12))
    }

    @Test
    fun `findUrlAt returns null when tap is far from any url`() {
        val line = "see https://example.com for details"
        assertNull(findUrlAt(line, line.length + 20))
    }

    @Test
    fun `findUrlAt strips trailing punctuation and brackets`() {
        val line = "go to (https://example.com/path), ok"
        assertEquals("https://example.com/path", findUrlAt(line, line.indexOf("https")))
    }

    @Test
    fun `findFirstUrl extracts the first link on a line`() {
        assertEquals(
            "https://chatgpt.com/codex/auth",
            findFirstUrl("Start: https://chatgpt.com/codex/auth or https://other.dev")
        )
    }

    @Test
    fun `url remains detectable when terminal wraps it across rows`() {
        val firstRow = "https://auth.openai.com/oauth/authorize?response_type=code&cli"
        val secondRow = "ent_id=app_example&redirect_uri=http%3A%2F%2Flocalhost"
        val joined = firstRow + secondRow

        assertEquals(
            "https://auth.openai.com/oauth/authorize?response_type=code&client_id=app_example&redirect_uri=http%3A%2F%2Flocalhost",
            findUrlAt(joined, firstRow.length + 4)
        )
    }
}
