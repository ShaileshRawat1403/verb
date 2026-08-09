package com.example.verb.terminal

/**
 * The amount of terminal context Verb can observe without interpreting terminal text.
 */
enum class TerminalContextCapability {
    UNAVAILABLE,
    SESSION_ONLY
}

/**
 * Direct terminal-emulator screen state. UNKNOWN means no emulator is available to inspect.
 */
enum class AlternateScreenState {
    ACTIVE,
    INACTIVE,
    UNKNOWN
}

/**
 * Metadata-only terminal context. It intentionally contains no command or output payload.
 */
data class TerminalContextState(
    val capability: TerminalContextCapability = TerminalContextCapability.UNAVAILABLE,
    val sessionId: String? = null,
    val alternateScreenState: AlternateScreenState = AlternateScreenState.UNKNOWN
)
