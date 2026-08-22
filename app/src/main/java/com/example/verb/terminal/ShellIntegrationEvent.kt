package com.example.verb.terminal

/**
 * Typed shell-integration events parsed from OSC 7 (CWD) / OSC 633 (lifecycle) markers.
 *
 * Advisory metadata only. Any process writing to the PTY -- not just the shell Verb's own
 * shell-integration script controls -- can emit bytes that parse into one of these events, so
 * this must never be treated as an execution-approval or security signal, only as a best-effort
 * hint about what the shell believes is happening.
 */
sealed class ShellIntegrationEvent {
    /** OSC 7: the shell's current working directory. */
    data class CurrentDirectory(val path: String) : ShellIntegrationEvent()

    /** OSC 633;A: prompt is about to be drawn. */
    data object PromptStart : ShellIntegrationEvent()

    /** OSC 633;B: prompt finished drawing; interactive input begins. */
    data object PromptEnd : ShellIntegrationEvent()

    /** OSC 633;C: a command's execution is starting. */
    data object CommandStart : ShellIntegrationEvent()

    /** OSC 633;D;<exitCode>: the most recently started command finished. */
    data class CommandEnd(val exitCode: Int) : ShellIntegrationEvent()

    /** OSC 633;E;<text>: bounded command-line text for the command about to start. */
    data class CommandMetadata(val commandText: String) : ShellIntegrationEvent()

    /** OSC 633;P;Verb=1: one-shot handshake confirming Verb's shell integration loaded. */
    data object Handshake : ShellIntegrationEvent()
}

/**
 * Strict, defensive parser for the OSC 7 / OSC 633 subset Verb's shell integration emits.
 * Never throws: any malformed, oversized, or unrecognized input resolves to `null` rather than
 * propagating an exception up through the vendored emulator's callback.
 */
object ShellIntegrationParser {
    /** Command text is truncated to this many characters before it ever reaches Kotlin state. */
    const val MAX_COMMAND_TEXT_LENGTH = 200

    /**
     * Defense in depth: the vendored emulator already caps a whole OSC argument string at 8192
     * chars (MAX_OSC_STRING_LENGTH); this is a much tighter bound specific to the small, fixed
     * shape of markers this protocol actually uses.
     */
    private const val MAX_PAYLOAD_LENGTH = 512

    private const val MIN_VALID_EXIT_CODE = 0
    private const val MAX_VALID_EXIT_CODE = 255

    fun parse(oscCode: Int, rawArgs: String): ShellIntegrationEvent? {
        if (rawArgs.length > MAX_PAYLOAD_LENGTH) return null
        return when (oscCode) {
            7 -> parseCurrentDirectory(rawArgs)
            633 -> parseLifecycle(rawArgs)
            else -> null
        }
    }

    /**
     * Standard OSC 7 payload is `file://<host><path>` (host is often empty, i.e. `file:///path`).
     * Deliberately not delegated to `java.net.URI`: the emitting shell script does not
     * percent-encode `$PWD`, so strict RFC 3986 parsing would reject legitimate paths containing
     * spaces or other raw characters. Instead this only validates the scheme prefix and that an
     * absolute path follows -- garbage that doesn't match is rejected, never guessed at.
     */
    private fun parseCurrentDirectory(rawArgs: String): ShellIntegrationEvent.CurrentDirectory? {
        if (!rawArgs.startsWith("file://")) return null
        val afterScheme = rawArgs.removePrefix("file://")
        val pathStart = afterScheme.indexOf('/')
        if (pathStart < 0) return null
        val path = afterScheme.substring(pathStart)
        if (!path.startsWith("/") || path.isBlank()) return null
        return ShellIntegrationEvent.CurrentDirectory(path)
    }

    private fun parseLifecycle(rawArgs: String): ShellIntegrationEvent? {
        val separatorIndex = rawArgs.indexOf(';')
        val code = if (separatorIndex < 0) rawArgs else rawArgs.substring(0, separatorIndex)
        val payload = if (separatorIndex < 0) "" else rawArgs.substring(separatorIndex + 1)
        return when (code) {
            "A" -> ShellIntegrationEvent.PromptStart
            "B" -> ShellIntegrationEvent.PromptEnd
            "C" -> ShellIntegrationEvent.CommandStart
            "D" -> parseExitCode(payload)?.let { ShellIntegrationEvent.CommandEnd(it) }
            "E" -> ShellIntegrationEvent.CommandMetadata(payload.take(MAX_COMMAND_TEXT_LENGTH))
            "P" -> if (payload == "Verb=1") ShellIntegrationEvent.Handshake else null
            else -> null
        }
    }

    private fun parseExitCode(payload: String): Int? {
        val value = payload.toIntOrNull() ?: return null
        return value.takeIf { it in MIN_VALID_EXIT_CODE..MAX_VALID_EXIT_CODE }
    }
}
