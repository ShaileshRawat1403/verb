package com.example.verb.session

/**
 * Claude's installed project-record directory mapping. Keep this identical to the desktop
 * `claude_project_dir` rule: replace each non-ASCII-alphanumeric UTF-16 unit, then truncate long
 * names and suffix the absolute Java string hash of the original path in base 36.
 */
object ClaudeProjectDirectory {
    fun encode(path: String): String {
        val replaced = buildString(path.length) {
            path.forEach { character ->
                append(
                    if (character in 'a'..'z' || character in 'A'..'Z' || character in '0'..'9') {
                        character
                    } else {
                        '-'
                    }
                )
            }
        }
        if (replaced.length <= MAX_DIRECTORY_LENGTH) return replaced
        val hash = kotlin.math.abs(path.hashCode().toLong()).toString(36)
        return "${replaced.take(MAX_DIRECTORY_LENGTH)}-$hash"
    }

    private const val MAX_DIRECTORY_LENGTH = 200
}
