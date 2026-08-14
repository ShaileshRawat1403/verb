package com.example.verb.terminal

private val URL_REGEX = Regex("""https?://[^\s"()<>\\\]^{|}`]+""")

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
    return best?.value
}

/** Returns the URL embedded in a line of terminal output, if any. */
internal fun findFirstUrl(line: String): String? =
    URL_REGEX.find(line)?.value
