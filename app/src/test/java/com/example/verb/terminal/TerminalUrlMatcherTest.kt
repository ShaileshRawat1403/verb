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

    @Test
    fun `long Google OAuth URL with parameters is fully extracted without truncation`() {
        val fullUrl = "https://accounts.google.com/o/oauth2/auth?access_type=offline&client_id=1071006060591-tmhssin2h21lc.apps.googleusercontent.com&code_challenge=abc123xyz&code_challenge_method=S256&prompt=consent&redirect_uri=https%3A%2F%2Fantigravity.google%2Foauth-callback&response_type=code&scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform+openid&state=xyz"
        val line = "Your browser should open automatically. If not:\n" + fullUrl
        val url = findUrlAt(line, line.indexOf("https://") + 50)
        assertEquals(fullUrl, url)
        assertEquals(fullUrl, findFirstUrl(line))
    }

    @Test
    fun `joinWrappedTerminalLines reconstructs full URL from wrapped terminal rows`() {
        val fullUrl = "https://accounts.google.com/o/oauth2/auth?access_type=offline&client_id=1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com&code_challenge=1XY5jtNT7iPNdA0vohAktEYM8JGHHbgiyjvnHQCQMes&code_challenge_method=S256&prompt=consent&redirect_uri=https%3A%2F%2Fantigravity.google%2Foauth-callback&response_type=code&scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform+openid&state=UfZKc2TJ18-zDUV3ZMi2Pw"
        val cols = 80
        val rows = mutableListOf<String>()
        rows += "Your browser should open automatically. If not:"
        var remaining = fullUrl
        while (remaining.isNotEmpty()) {
            val chunk = remaining.take(cols)
            rows += chunk
            remaining = remaining.drop(cols)
        }
        rows += "Copy and paste the URL or click on the link below:"
        rows += "-> Click here to authenticate"

        val reconstructed = joinWrappedTerminalLines(rows, cols)
        val extractedUrl = findFirstUrl(reconstructed)
        assertEquals(fullUrl, extractedUrl)
    }
}
