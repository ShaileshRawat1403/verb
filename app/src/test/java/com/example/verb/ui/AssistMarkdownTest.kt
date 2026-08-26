package com.example.verb.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
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

    private fun boldCount(text: String) =
        AssistMarkdown.render(text).spanStyles.count { it.item.fontWeight == FontWeight.Bold }

    private fun monoSpans(text: String) =
        AssistMarkdown.render(text).spanStyles.filter { it.item.fontFamily == FontFamily.Monospace }

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
        assertEquals(1, boldCount("- **one** fact\n- plain fact"))
    }

    /** The model uses all three interchangeably; a reader should not be able to tell which. */
    @Test
    fun `asterisk and plus bullets read the same as hyphen bullets`() {
        assertEquals("•  one", AssistMarkdown.render("* one").text)
        assertEquals("•  one", AssistMarkdown.render("+ one").text)
    }

    /** Observed on the device: diagnostic steps come back as "1." / "2.". */
    @Test
    fun `numbered steps keep their numbers and align with bullets`() {
        val rendered = AssistMarkdown.render("1. Review the command\n2) Check the logs")

        assertEquals("1.  Review the command\n2)  Check the logs", rendered.text)
    }

    @Test
    fun `a nested bullet is carried by indentation, not by a second glyph`() {
        val rendered = AssistMarkdown.render("- top\n  - nested")

        assertEquals("•  top\n   •  nested", rendered.text)
    }

    @Test
    fun `backticked identifiers render monospaced without the backticks`() {
        val rendered = AssistMarkdown.render("run `git status` first")

        assertEquals("run git status first", rendered.text)
        val mono = rendered.spanStyles.single { it.item.fontFamily == FontFamily.Monospace }
        assertEquals("run ".length, mono.start)
        assertEquals("run git status".length, mono.end)
    }

    /** An asterisk inside code is an asterisk. Code is resolved first and never re-scanned. */
    @Test
    fun `emphasis markers inside code are left alone`() {
        val rendered = AssistMarkdown.render("pass `--exclude=*.log` to it")

        assertEquals("pass --exclude=*.log to it", rendered.text)
        assertEquals(0, boldCount("pass `--exclude=*.log` to it"))
    }

    @Test
    fun `a fenced block renders monospaced and the fences themselves are not content`() {
        val rendered = AssistMarkdown.render("before\n```\ngit log --oneline\n```\nafter")

        assertEquals("before\n\ngit log --oneline\n\nafter", rendered.text)
        assertTrue(monoSpans("before\n```\ngit log --oneline\n```\nafter").isNotEmpty())
    }

    /** An unclosed fence means the rest of the answer is code, which is what the model said. */
    @Test
    fun `an unclosed fence does not swallow the text as plain`() {
        assertTrue(monoSpans("intro\n```\ngit log").isNotEmpty())
    }

    @Test
    fun `headings are bold and larger, and are not bold twice when also wrapped in stars`() {
        val plain = AssistMarkdown.render("## What remains unknown")
        assertEquals("What remains unknown", plain.text)
        assertEquals(1, plain.spanStyles.count { it.item.fontWeight == FontWeight.Bold })

        val doubled = AssistMarkdown.render("## **What remains unknown**")
        assertEquals("What remains unknown", doubled.text)
        assertEquals(1, doubled.spanStyles.count { it.item.fontWeight == FontWeight.Bold })
    }

    @Test
    fun `italics render for both markers`() {
        val stars = AssistMarkdown.render("an *observed* fact")
        assertEquals("an observed fact", stars.text)
        assertEquals(1, stars.spanStyles.count { it.item.fontStyle == FontStyle.Italic })

        val unders = AssistMarkdown.render("an _observed_ fact")
        assertEquals("an observed fact", unders.text)
        assertEquals(1, unders.spanStyles.count { it.item.fontStyle == FontStyle.Italic })
    }

    /** Bold must not be read as an italic open followed by a stray marker. */
    @Test
    fun `a bold span is not mistaken for italics`() {
        val rendered = AssistMarkdown.render("**bold**")

        assertEquals("bold", rendered.text)
        assertEquals(1, rendered.spanStyles.count { it.item.fontWeight == FontWeight.Bold })
        assertEquals(0, rendered.spanStyles.count { it.item.fontStyle == FontStyle.Italic })
    }

    @Test
    fun `an unclosed marker is content and is shown as written`() {
        assertEquals("a ** stray marker", AssistMarkdown.render("a ** stray marker").text)
        assertEquals("a ` stray tick", AssistMarkdown.render("a ` stray tick").text)
    }

    /**
     * Arithmetic and shell globs are ordinary content here. A marker followed by a space does not
     * open emphasis, so formatting cannot quietly delete a character the model meant to send.
     */
    @Test
    fun `a star with space around it is multiplication, not emphasis`() {
        assertEquals("2 * 3 * 4", AssistMarkdown.render("2 * 3 * 4").text)
        assertEquals("rm *.log and *.tmp", AssistMarkdown.render("rm *.log and *.tmp").text)
        assertEquals("a ** b ** c", AssistMarkdown.render("a ** b ** c").text)
    }

    /** A lone dash is a dash. Only "- " with content after it is a list. */
    @Test
    fun `a bare dash is not a list`() {
        assertEquals("-", AssistMarkdown.render("-").text)
        assertEquals("exit 1 - failed", AssistMarkdown.render("exit 1 - failed").text)
    }

    @Test
    fun `plain text passes through unchanged`() {
        assertEquals("no formatting here", AssistMarkdown.render("no formatting here").text)
        assertTrue(AssistMarkdown.render("no formatting here").spanStyles.isEmpty())
    }

    /** Colour is the caller's, never the renderer's. */
    @Test
    fun `the code background is the one the caller passed`() {
        val rendered = AssistMarkdown.render("use `ls`", Color.Red)

        assertEquals(Color.Red, rendered.spanStyles.single { it.item.fontFamily == FontFamily.Monospace }.item.background)
    }

    /** The shape a real answer came back in, on the Vivo, end to end. */
    @Test
    fun `a real answer renders as sections, bullets and steps`() {
        val answer = """
            **Established facts (from the evidence):**
            - The session is running.
            - The last command failed with `exit 2`.

            **Safest next diagnostic step:**
            1. Re-run with the flag.
        """.trimIndent()

        val rendered = AssistMarkdown.render(answer)

        assertTrue(rendered.text.contains("Established facts (from the evidence):"))
        assertTrue(rendered.text.contains("•  The session is running."))
        assertTrue(rendered.text.contains("1.  Re-run with the flag."))
        assertTrue(rendered.text.contains("exit 2"))
        assertTrue(!rendered.text.contains("**"))
        assertTrue(!rendered.text.contains("`"))
    }
}
