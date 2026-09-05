package com.example.verb.terminal

private val URL_REGEX = Regex("""https?://[^\s"()<>\\\]^{|}`]+""")

private val TRAILING_PUNCTUATION = charArrayOf('.', ',', ';', '\'', '"', ')', ']', '>')

internal fun sanitizeUrl(url: String): String {
    var cleaned = url
    while (cleaned.isNotEmpty() && cleaned.last() in TRAILING_PUNCTUATION) {
        cleaned = cleaned.dropLast(1)
    }
    return cleaned
}

/**
 * Finds a URL on a terminal output line near the tapped column.
 *
 * The tapped column is a raw terminal-buffer column. Keep leading whitespace in the line so the
 * URL span and the tap use the same coordinate space; the fallback caller handles lines with a
 * single URL when terminal text has been wrapped or decorated by a command's output.
 *
 * @param line the terminal line content
 * @param tappedColumn the raw buffer column that was tapped
 * @return the URL whose span is nearest the tap, or null when the tap is far from any URL
 */
internal fun findUrlAt(line: String, tappedColumn: Int): String? {
    if (line.isBlank()) return null
    val leadingTolerance = 8
    val trailingTolerance = 4
    val maxTapDistance = 8
    var best: MatchResult? = null
    var bestDistance = Int.MAX_VALUE
    for (match in URL_REGEX.findAll(line)) {
        val lo = match.range.first - leadingTolerance
        val hi = match.range.last + trailingTolerance
        val distance = when {
            tappedColumn < lo -> lo - tappedColumn
            tappedColumn > hi -> tappedColumn - hi
            else -> 0
        }
        if (distance <= maxTapDistance && distance < bestDistance) {
            bestDistance = distance
            best = match
        }
    }
    return best?.value?.let { sanitizeUrl(it) }
}

/** Returns the URL embedded in a line of terminal output, if any. */
internal fun findFirstUrl(line: String): String? =
    URL_REGEX.find(line)?.value?.let { sanitizeUrl(it) }

/**
 * Joins terminal output lines, recognizing lines that wrapped across terminal column bounds.
 *
 * When consecutive lines are part of a URL (or lack internal whitespace/tokens), they are joined
 * directly without inserting whitespace. Lines with whitespace are separated by spaces.
 */
internal fun joinWrappedTerminalLines(lines: List<String>, terminalColumns: Int): String {
    if (lines.isEmpty()) return ""
    val builder = StringBuilder()
    var inUrl = false

    // Set when the previous line ended mid-URL, so this one is a continuation rather than a new
    // line of output.
    //
    // A continuation contributes its *content* and not its indent. Antigravity draws its sign-in
    // screen inside a one-column inset and wraps the OAuth URL itself, so on a Vivo I2202 every row
    // of the URL came back as one leading space plus 90 characters in a 91-column terminal.
    // Appending those rows verbatim put that space back into the middle of the URL, the regex
    // stopped at it, and Chrome received everything up to `client_id=1071006060591-tmh` -- one
    // wrapped line -- which Google rejects with "Required parameter is missing: response_type".
    //
    // Nothing is lost for URLs the *emulator* wrapped: those continuations begin at column zero, so
    // trimming their (absent) indent is a no-op.
    var continuesUrl = false

    for (i in lines.indices) {
        val raw = lines[i].trimEnd()
        val line = if (continuesUrl) raw.trimStart() else raw
        continuesUrl = false
        if (line.isEmpty()) {
            inUrl = false
            if (i < lines.size - 1) builder.append(' ')
            continue
        }

        val lastHttpIndex = maxOf(line.lastIndexOf("http://"), line.lastIndexOf("https://"))
        if (lastHttpIndex != -1) {
            val textAfterHttp = line.substring(lastHttpIndex)
            if (!textAfterHttp.contains(' ')) {
                inUrl = true
            }
        }

        builder.append(line)

        if (i < lines.size - 1) {
            val nextLine = lines[i + 1].trimEnd()
            val nextFirstToken = nextLine.trimStart().substringBefore(' ')
            val nextIsNewHttp = nextFirstToken.startsWith("http://") || nextFirstToken.startsWith("https://")
            val nextIsPrompt = nextLine.startsWith("~ $") || nextLine.startsWith("$ ") || nextLine.startsWith("-> ")

            // Measured on the row as the terminal drew it, indent included: the question is whether
            // this row filled the width, and a continuation's indent occupies columns just like any
            // other character. Using the trimmed content here would misjudge any agent that insets
            // its output by more than a column or two.
            val isWrappedWidth = raw.length >= (terminalColumns - 6)

            if (inUrl && isWrappedWidth && !nextIsNewHttp && !nextIsPrompt && nextFirstToken.isNotEmpty()) {
                // The URL continues onto the next line without an intervening space.
                continuesUrl = true
                if (nextLine.trimStart().contains(' ')) {
                    inUrl = false
                }
            } else {
                inUrl = false
                if (line.length < terminalColumns) {
                    builder.append(' ')
                }
            }
        }
    }
    return builder.toString()
}



