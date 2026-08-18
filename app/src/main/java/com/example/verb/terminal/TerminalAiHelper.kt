package com.example.verb.terminal

import com.example.verb.ai.AiAssistantRequest
import com.example.verb.ai.AiAssistantService
import com.example.verb.semantic.SecretGuard

/**
 * Explains a snapshot of terminal output using the user's configured provider (BYOK, Keystore
 * encrypted). The prompt is redacted through [SecretGuard] before it leaves the device, the
 * assistant is explicitly constrained to proposing steps rather than claiming execution, and the
 * recent output window is capped so only a bounded context is ever shared.
 */
object TerminalAiHelper {

    private const val TERMINAL_SYSTEM_INSTRUCTION =
        "You are Verb's terminal assistant helping a user inside an Android native shell " +
            "(Verb's PTY userland). Interpret the provided terminal output. Keep answers brief, " +
            "highlight any obvious errors, and propose 1-2 concrete next commands. You can only " +
            "propose commands: you must never claim a command was executed."

    /**
     * [workingDir] is the shell's own directory as a guest path, or null when Verb genuinely does
     * not know it. Null is passed through to the model as "unknown" rather than being replaced with
     * the session's launch directory, so the assistant is never told the user is somewhere they are
     * not.
     */
    suspend fun analyze(
        service: AiAssistantService,
        output: String,
        workingDir: String?,
        sessionState: TerminalSessionState
    ): String {
        val redacted = SecretGuard.redactKnownSensitiveText(output)
        val recent = redacted.takeLast(MAX_OUTPUT_CHARS)

        val prompt = buildString {
            appendLine("The user is working in an Android native shell.")
            appendLine("Working directory: ${workingDir ?: "unknown"}")
            appendLine("Session state: ${sessionState.name}")
            appendLine("Explain the following recent terminal output and suggest 1-2 next commands:")
            appendLine("```")
            appendLine(recent)
            appendLine("```")
        }

        val response = service.respond(
            AiAssistantRequest(
                prompt = prompt,
                systemInstruction = TERMINAL_SYSTEM_INSTRUCTION
            )
        )
        return response.text
    }

    private const val MAX_OUTPUT_CHARS = 2000
}
