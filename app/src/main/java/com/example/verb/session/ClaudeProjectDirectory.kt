package com.example.verb.session

/** Claude's installed project-record directory mapping, shared in meaning with desktop. */
object ClaudeProjectDirectory {
    fun encode(path: String): String = buildString(path.length) {
        path.forEach { character ->
            append(if (character == '/' || character == '.' || character == '_') '-' else character)
        }
    }
}
