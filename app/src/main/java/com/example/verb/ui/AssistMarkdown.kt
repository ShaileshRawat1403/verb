package com.example.verb.ui

import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight

/**
 * Renders the small markdown the assistant actually produces -- **bold** spans and "- " bullets --
 * into an [AnnotatedString]. Deliberately not a markdown library: the model is instructed to keep
 * answers brief, the vocabulary of shapes is tiny, and anything outside it is shown as written
 * rather than guessed at.
 */
object AssistMarkdown {

    fun render(text: String): AnnotatedString = buildAnnotatedString {
        text.lines().forEachIndexed { lineIndex, line ->
            if (lineIndex > 0) append('\n')
            val trimmed = line.trimStart()
            if (trimmed.startsWith("- ")) {
                append("•  ")
                appendStyled(trimmed.removePrefix("- "))
            } else {
                appendStyled(line)
            }
        }
    }

    private fun AnnotatedString.Builder.appendStyled(line: String) {
        var cursor = 0
        while (true) {
            val start = line.indexOf("**", cursor)
            if (start < 0) {
                append(line.substring(cursor))
                break
            }
            val end = line.indexOf("**", start + 2)
            if (end < 0) {
                // An unclosed marker is content, not formatting -- render it as written.
                append(line.substring(cursor))
                break
            }
            append(line.substring(cursor, start))
            pushStyle(SpanStyle(fontWeight = FontWeight.Bold))
            append(line.substring(start + 2, end))
            pop()
            cursor = end + 2
        }
    }
}
