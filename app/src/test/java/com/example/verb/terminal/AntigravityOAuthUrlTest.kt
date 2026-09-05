package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The Antigravity sign-in URL, captured off the Vivo I2202's terminal buffer rather than retyped.
 *
 * Tapping it opened `accounts.google.com/signin/oauth/error`, whose `authError` base64-decodes to
 * `invalid_request / Required parameter is missing: response_type`, and whose echoed `client_id`
 * was `1071006060591-tmh` -- the first line of the URL and nothing after it.
 *
 * The rows below are logged verbatim from `findUrlAtBuffer`: `numCols=91`, and **every row is 90
 * characters beginning with a space**. Antigravity draws its sign-in screen inside a one-column
 * inset and wraps the URL itself, so these are not emulator-wrapped rows -- the native buffer join
 * puts real newlines between them. Appending them verbatim dropped that indent into the middle of
 * the URL and the regex stopped at the first one.
 *
 * The leading spaces are the entire point of this fixture. Do not tidy them away.
 */
class AntigravityOAuthUrlTest {

    private val columns = 91

    private val wrapped = listOf(
        " https://accounts.google.com/o/oauth2/auth?access_type=offline&client_id=1071006060591-tmh",
        " ssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com&code_challenge=f_PkL9CA8ArFiqdMk",
        " WJkyvJl9p5O7vsDl21qTsr-9NE&code_challenge_method=S256&prompt=consent&redirect_uri=https%3",
        " A%2F%2Fantigravity.google%2Foauth-callback&response_type=code&scope=https%3A%2F%2Fwww.goo",
        " gleapis.com%2Fauth%2Fcloud-platform+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fuserinfo.em",
        " ail+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fuserinfo.profile+https%3A%2F%2Fwww.googleap",
        " is.com%2Fauth%2Fcclog+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fexperimentsandconfigs+htt",
        " ps%3A%2F%2Fwww.googleapis.com%2Fauth%2Faicode+openid&state=6OzNCMXhTvNtjW3xKBeufg"
    )

    private val expected = wrapped.joinToString("") { it.trim() }

    @Test
    fun `an indented wrapped sign-in URL rejoins without the indent landing inside it`() {
        val url = findFirstUrl(joinWrappedTerminalLines(wrapped, columns))
        assertEquals(expected, url)
    }

    @Test
    fun `the rejoined URL keeps the parameter Google rejected the request for`() {
        val url = findFirstUrl(joinWrappedTerminalLines(wrapped, columns))
        assertTrue("response_type was dropped: $url", url!!.contains("response_type=code"))
    }

    /** The visible symptom: Google echoed the client_id back cut at the first wrap. */
    @Test
    fun `the rejoined URL carries the whole client_id, not the first line of it`() {
        val url = findFirstUrl(joinWrappedTerminalLines(wrapped, columns))
        assertTrue(
            "client_id truncated at the wrap: $url",
            url!!.contains("client_id=1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com")
        )
    }

    /** Emulator-wrapped URLs have no indent to strip, and must be unaffected. */
    @Test
    fun `an unindented wrapped URL still rejoins`() {
        val unindented = wrapped.map { it.trimStart() }
        val url = findFirstUrl(joinWrappedTerminalLines(unindented, 89))
        assertEquals(expected, url)
    }

    /**
     * A deeper inset than Antigravity's must work too, which is why the wrap-width test measures
     * the row as drawn rather than the content left after the indent is stripped.
     */
    @Test
    fun `a four column inset still rejoins`() {
        val content = wrapped.map { it.trim() }
        val inset = content.map { "    $it" }
        val columns = content.maxOf { it.length } + 4
        assertEquals(expected, findFirstUrl(joinWrappedTerminalLines(inset, columns)))
    }

    /** Prose above the URL must not be swept into it, which a blanket join would do. */
    @Test
    fun `prose above the URL is not absorbed into it`() {
        val withProse = listOf(" Your browser should open automatically. If not:", "") + wrapped
        val url = findFirstUrl(joinWrappedTerminalLines(withProse, columns))
        assertEquals(expected, url)
    }

    /** A short URL followed by an ordinary sentence must not glue the sentence onto it. */
    @Test
    fun `a short URL is not glued to the line beneath it`() {
        val lines = listOf(" See https://example.com", " for details")
        assertEquals("https://example.com", findFirstUrl(joinWrappedTerminalLines(lines, columns)))
    }
}
