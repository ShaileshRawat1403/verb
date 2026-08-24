package com.example.verb.terminal

import com.example.verb.ai.AiAssistantRequest
import com.example.verb.ai.AiAssistantService
/**
 * Explains structural terminal evidence using the user's configured provider. Raw PTY output and
 * command text are deliberately not accepted by this API, so they cannot leave the device through
 * this path. Unknown detail stays unknown rather than being filled from transcript surveillance.
 */
object TerminalAiHelper {

    private const val TERMINAL_SYSTEM_INSTRUCTION =
        "You are Verb's evidence-bound terminal assistant. Use only the structural facts provided. " +
            "Say when the evidence cannot explain a cause. Keep answers brief and propose at most " +
            "two diagnostic actions. Never claim an action ran and never invent terminal content."

    /**
     * Only lifecycle metadata crosses the provider boundary. [workingDirectoryKnown] carries the
     * distinction between observed and unknown without disclosing an absolute path.
     */
    suspend fun analyze(
        service: AiAssistantService,
        lastCommand: CommandExecutionRecord?,
        workingDirectoryKnown: Boolean,
        sessionState: TerminalSessionState
    ): String {
        val prompt = buildString {
            appendLine("Evidence source: Verb shell integration (structural metadata only).")
            appendLine("Working directory observed: $workingDirectoryKnown (path withheld).")
            appendLine("Session state: ${sessionState.name}")
            if (lastCommand == null) {
                appendLine("Last command boundary: unknown.")
            } else {
                appendLine("Last command lifecycle: ${lastCommand.state.name}")
                appendLine("Last command exit code: ${lastCommand.exitCode ?: "unknown"}")
                appendLine("Last command duration ms: ${lastCommand.durationMs ?: "unknown"}")
            }
            appendLine("Explain what these facts establish, what remains unknown, and the safest next diagnostic step.")
        }

        val response = service.respond(
            AiAssistantRequest(
                prompt = prompt,
                systemInstruction = TERMINAL_SYSTEM_INSTRUCTION
            )
        )
        return response.text
    }

}
