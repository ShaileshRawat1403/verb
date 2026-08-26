package com.example.verb.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em

/**
 * Renders the markdown the assistant actually produces into an [AnnotatedString].
 *
 * Still deliberately not a markdown library. The vocabulary here is the one observed coming back
 * from a real provider on a real device -- bold section headings, `- ` bullets, numbered steps,
 * backticked identifiers -- plus the two block shapes a developer tool inevitably gets asked for.
 * Anything outside it is shown as written rather than guessed at, because a wrong guess about
 * formatting is a claim about content: an unclosed marker is text the model meant to send.
 *
 * Sizes are relative ([em]), so the caller's font size stays the source of truth and a heading is
 * proportionate wherever the panel is hosted. Colour is passed in rather than chosen here — this
 * renderer has no business knowing the theme.
 */
object AssistMarkdown {

    private const val FENCE = "```"

    fun render(text: String, codeBackground: Color = Color.Unspecified): AnnotatedString =
        buildAnnotatedString {
            var inFence = false
            val lines = text.lines()

            lines.forEachIndexed { index, line ->
                if (index > 0) append('\n')
                val trimmed = line.trimStart()

                // A fence toggles a block and is never itself content. An unclosed fence simply
                // means the rest of the answer is code, which is what the model was saying.
                if (trimmed.startsWith(FENCE)) {
                    inFence = !inFence
                    return@forEachIndexed
                }

                if (inFence) {
                    appendCode(line, codeBackground)
                    return@forEachIndexed
                }

                when {
                    trimmed.startsWith("### ") -> appendHeading(trimmed.removePrefix("### "), 1.0f)
                    trimmed.startsWith("## ") -> appendHeading(trimmed.removePrefix("## "), 1.1f)
                    trimmed.startsWith("# ") -> appendHeading(trimmed.removePrefix("# "), 1.2f)

                    BULLET.matches(trimmed) -> {
                        append(indentFor(line))
                        append("•  ")
                        appendInline(trimmed.substring(2), codeBackground)
                    }

                    ORDERED.matches(trimmed) -> {
                        val marker = trimmed.substringBefore(' ')
                        append(indentFor(line))
                        append(marker)
                        append("  ")
                        appendInline(trimmed.substringAfter(' '), codeBackground)
                    }

                    else -> appendInline(line, codeBackground)
                }
            }
        }

    /** `- x`, `* x` and `+ x`. A bare `-` with nothing after it is a dash, not a list. */
    private val BULLET = Regex("""^[-*+] .*""")

    /** `1. x` and `1) x`, any number of digits. */
    private val ORDERED = Regex("""^\d+[.)] .*""")

    /**
     * Nesting is carried by indentation rather than by a second glyph. `docs/UX_FOUNDATION.md`
     * keeps the glyph set narrow on purpose, and a sub-bullet is not a new kind of thing.
     */
    private fun indentFor(line: String): String {
        val spaces = line.length - line.trimStart().length
        return " ".repeat((spaces / 2) * 3)
    }

    private fun AnnotatedString.Builder.appendHeading(content: String, scale: Float) {
        pushStyle(SpanStyle(fontWeight = FontWeight.Bold, fontSize = scale.em))
        // Inline marks still apply inside a heading, but its own emphasis is already carried by the
        // weight, so a heading wrapped in ** does not need to be bold twice.
        appendInline(content.removeSurrounding("**"), Color.Unspecified)
        pop()
    }

    private fun AnnotatedString.Builder.appendCode(content: String, background: Color) {
        pushStyle(SpanStyle(fontFamily = FontFamily.Monospace, fontSize = 0.9.em, background = background))
        append(content)
        pop()
    }

    /**
     * Inline marks, resolved left to right: `` `code` ``, `**bold**`, `*italic*` and `_italic_`.
     *
     * Code is resolved first and its contents are never re-scanned, so an asterisk inside a
     * backticked flag stays an asterisk. That is the whole reason this is not three passes.
     */
    private fun AnnotatedString.Builder.appendInline(line: String, codeBackground: Color) {
        var cursor = 0
        while (cursor < line.length) {
            val next = nextMark(line, cursor)
            if (next == null) {
                append(line.substring(cursor))
                return
            }

            append(line.substring(cursor, next.start))

            if (next.marker == "`") {
                appendCode(line.substring(next.start + 1, next.end), codeBackground)
            } else {
                val style = if (next.marker == "**") {
                    SpanStyle(fontWeight = FontWeight.Bold)
                } else {
                    SpanStyle(fontStyle = FontStyle.Italic)
                }
                pushStyle(style)
                appendInline(line.substring(next.start + next.marker.length, next.end), codeBackground)
                pop()
            }
            cursor = next.end + next.marker.length
        }
    }

    private class Mark(val marker: String, val start: Int, val end: Int)

    /**
     * The earliest closed mark at or after [from], or null when the rest of the line is plain.
     *
     * An unclosed marker is not a mark. It is content, and returning null for it is what makes
     * "a ** stray marker" render as written instead of swallowing the rest of the answer.
     */
    private fun nextMark(line: String, from: Int): Mark? {
        var best: Mark? = null
        for (marker in MARKERS) {
            var start = from
            while (true) {
                start = line.indexOf(marker, start)
                if (start < 0) break
                // `*` must not match the opening of a `**`, or bold renders as italic-then-stray.
                if (marker == "*" && line.startsWith("**", start)) { start += 2; continue }
                if (!opens(line, start, marker)) { start += marker.length; continue }
                val end = closingIndex(line, start + marker.length, marker)
                if (end < 0) break
                if (best == null || start < best.start) best = Mark(marker, start, end)
                break
            }
        }
        return best
    }

    /**
     * CommonMark's flanking rule, kept because arithmetic and shell globs are ordinary content in a
     * developer tool: an emphasis run only opens when a non-space follows it, and only closes when
     * a non-space precedes it. Without this, "2 * 3 * 4" silently becomes "2 3 4" in italics --
     * formatting quietly deleting a character the model meant to send.
     *
     * Code spans are exempt: `` ` `` delimits verbatim text and says nothing about what surrounds it.
     */
    private fun opens(line: String, start: Int, marker: String): Boolean {
        if (marker == "`") return true
        val after = line.getOrNull(start + marker.length) ?: return false
        return !after.isWhitespace()
    }

    private fun closingIndex(line: String, from: Int, marker: String): Int {
        var at = from
        while (true) {
            at = line.indexOf(marker, at)
            if (at < 0) return -1
            if (at == from) { at += marker.length; continue }
            if (marker == "`") return at
            val before = line.getOrNull(at - 1)
            if (before != null && !before.isWhitespace()) return at
            at += marker.length
        }
    }

    /** Longest first, so `**` is tested before `*`. */
    private val MARKERS = listOf("`", "**", "*", "_")
}
