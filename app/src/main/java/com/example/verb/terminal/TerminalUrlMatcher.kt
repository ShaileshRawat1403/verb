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

    for (i in lines.indices) {
        val line = lines[i].trimEnd()
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

            val isWrappedWidth = line.length >= (terminalColumns - 6)

            if (inUrl && isWrappedWidth && !nextIsNewHttp && !nextIsPrompt && nextFirstToken.isNotEmpty()) {
                // The URL continues onto the next line without an intervening space.
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



