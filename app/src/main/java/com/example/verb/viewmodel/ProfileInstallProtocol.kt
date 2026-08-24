package com.example.verb.viewmodel

/**
 * Completion protocol for the visible terminal commands Verb uses to install runtime profiles.
 *
 * The terminal transcript contains the command itself before it contains the command's output. A
 * literal completion marker in that command therefore looks like completion before the installer
 * has even run. Build the marker from two shell literals so its full value appears only in the
 * final output record.
 */
internal object ProfileInstallProtocol {
    private val SAFE_MARKER = Regex("[A-Z0-9_]+")

    fun command(installCommand: String, marker: String): String {
        require(marker.matches(SAFE_MARKER)) { "Profile marker must be an opaque safe token." }
        val split = marker.length / 2
        val left = marker.substring(0, split)
        val right = marker.substring(split)
        return "$installCommand; profile_status=${'$'}?; " +
            "verb_marker='$left'; verb_marker=\"${'$'}{verb_marker}$right\"; " +
            "printf '\\n%s:%s\\n' \"${'$'}verb_marker\" \"${'$'}profile_status\""
    }

    /** Returns null until a complete numeric record exists. The newest record wins. */
    fun exitCode(output: String, marker: String): Int? {
        val prefix = "$marker:"
        val start = output.lastIndexOf(prefix)
        if (start < 0) return null
        return output.substring(start + prefix.length)
            .trimStart()
            .takeWhile { it == '-' || it.isDigit() }
            .toIntOrNull()
    }
}
