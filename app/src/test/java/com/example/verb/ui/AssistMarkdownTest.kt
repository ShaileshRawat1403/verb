package com.example.verb.ui

import androidx.compose.ui.text.font.FontWeight
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class AssistMarkdownTest {

    @Test
    fun `bold spans render as bold text without the markers`() {
        val rendered = AssistMarkdown.render("**Established by evidence:** rest")

        assertEquals("Established by evidence: rest", rendered.text)
        val bold = rendered.spanStyles.filter { it.item.fontWeight == FontWeight.Bold }
        assertEquals(1, bold.size)
        assertEquals(0, bold.single().start)
        assertEquals("Established by evidence:".length, bold.single().end)
    }

    @Test
    fun `bullets become bullet glyphs and their content is still markdown-rendered`() {
        val rendered = AssistMarkdown.render("- **one** fact\n- plain fact")

        assertEquals("•  one fact\n•  plain fact", rendered.text)
        assertEquals(1, rendered.spanStyles.count { it.item.fontWeight == FontWeight.Bold })
    }

    @Test
    fun `an unclosed marker is content and is shown as written`() {
        val rendered = AssistMarkdown.render("a ** stray marker")

        assertEquals("a ** stray marker", rendered.text)
    }

    @Test
    fun `plain text passes through unchanged`() {
        assertEquals("no formatting here", AssistMarkdown.render("no formatting here").text)
        assertTrue(AssistMarkdown.render("no formatting here").spanStyles.isEmpty())
    }
}
