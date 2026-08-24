package com.example.verb.ui

import com.example.verb.session.VerbSessionState
import com.example.verb.terminal.TerminalSessionState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The accessibility rule, made a test rather than a habit.
 *
 * `docs/UX_FOUNDATION.md`: "every coloured thing also carries a word or a glyph, because `NO_COLOR`
 * is honoured, terminals disagree about palettes, and some readers cannot see the difference." The
 * Android tab dot this replaced carried its entire meaning in a hue, so the rule was documented on
 * one host and broken on the other. It is now checked.
 */
class VerbStatusVocabularyTest {

    /** Exactly the set `desktop/src/tui/theme.rs` uses, and nothing wider. */
    private val allowedGlyphs = setOf("●", "◐", "◌", "○", "✕")

    @Test
    fun `every process state has both a glyph and a word`() {
        val states: List<TerminalSessionState?> = TerminalSessionState.entries + listOf(null)
        states.forEach { state ->
            val glyph = VerbStatusVocabulary.processGlyph(state)
            val word = VerbStatusVocabulary.processWord(state)
            assertTrue("$state has no glyph", glyph.isNotBlank())
            assertTrue("$state has no word", word.isNotBlank())
            assertTrue("$state uses a glyph outside the shared set: $glyph", glyph in allowedGlyphs)
        }
    }

    @Test
    fun `every session state has both a glyph and a word`() {
        val states: List<VerbSessionState?> = VerbSessionState.entries + listOf(null)
        states.forEach { state ->
            val glyph = VerbStatusVocabulary.sessionGlyph(state)
            val word = VerbStatusVocabulary.sessionWord(state)
            assertTrue("$state has no glyph", glyph.isNotBlank())
            assertTrue("$state has no word", word.isNotBlank())
            assertTrue("$state uses a glyph outside the shared set: $glyph", glyph in allowedGlyphs)
        }
    }

    @Test
    fun `no two process states read the same, so the word alone identifies the state`() {
        val states: List<TerminalSessionState?> = TerminalSessionState.entries + listOf(null)
        val words = states.map { VerbStatusVocabulary.processWord(it) }
        assertEquals("two process states share a word", words.size, words.toSet().size)
    }

    @Test
    fun `no two session states read the same`() {
        val states: List<VerbSessionState?> = VerbSessionState.entries + listOf(null)
        val words = states.map { VerbStatusVocabulary.sessionWord(it) }
        assertEquals("two session states share a word", words.size, words.toSet().size)
    }

    @Test
    fun `the glyphs match the desktop host exactly`() {
        assertEquals("●", VerbStatusVocabulary.processGlyph(TerminalSessionState.RUNNING))
        assertEquals("○", VerbStatusVocabulary.processGlyph(TerminalSessionState.EXITED))
        assertEquals("✕", VerbStatusVocabulary.processGlyph(TerminalSessionState.FAILED))
        assertEquals("◌", VerbStatusVocabulary.processGlyph(TerminalSessionState.STARTING))
        assertEquals("◌", VerbStatusVocabulary.processGlyph(TerminalSessionState.STOPPING))

        assertEquals("●", VerbStatusVocabulary.sessionGlyph(VerbSessionState.LIVE))
        assertEquals("◐", VerbStatusVocabulary.sessionGlyph(VerbSessionState.RECOVERABLE))
        assertEquals("◌", VerbStatusVocabulary.sessionGlyph(VerbSessionState.INTERRUPTED))
        assertEquals("○", VerbStatusVocabulary.sessionGlyph(VerbSessionState.ENDED))
    }

    /**
     * A terminal session state cannot make a claim about an agent conversation, so it must never
     * borrow the glyph that means "Verb has evidence recovery will work".
     */
    @Test
    fun `no process state claims recoverability`() {
        val states: List<TerminalSessionState?> = TerminalSessionState.entries + listOf(null)
        assertFalse(states.any { VerbStatusVocabulary.processGlyph(it) == "◐" })
    }

    /**
     * `INTERRUPTED` is the absence of an answer. It must not read as a soft "ended" or an
     * optimistic "recoverable", both of which claim something Verb has not established.
     */
    @Test
    fun `an interrupted session says its recovery status is unknown`() {
        val word = VerbStatusVocabulary.sessionWord(VerbSessionState.INTERRUPTED)
        assertTrue("expected the word to name the uncertainty, got \"$word\"", word.contains("unknown"))
    }

    @Test
    fun `no state is described as absent when it is merely unreported`() {
        assertEquals("not ready", VerbStatusVocabulary.processWord(null))
        assertEquals("no session", VerbStatusVocabulary.sessionWord(null))
    }

    @Test
    fun `the screen reader description offers restarting only when a tap would restart`() {
        val running = VerbStatusVocabulary.processDescription(TerminalSessionState.RUNNING)
        assertTrue(running.contains("running"))
        assertFalse("a running session must not offer to start a new one", running.contains("Activate"))

        val failed = VerbStatusVocabulary.processDescription(TerminalSessionState.FAILED)
        assertTrue(failed.contains("failed"))
        assertTrue(failed.contains("Activate"))
    }

    @Test
    fun `a session description names the agent it belongs to`() {
        assertEquals(
            "Claude Code session recoverable",
            VerbStatusVocabulary.sessionDescription("Claude Code", VerbSessionState.RECOVERABLE)
        )
    }
}
