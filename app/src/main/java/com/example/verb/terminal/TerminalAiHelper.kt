package com.example.verb.terminal

import com.example.verb.ai.AiAssistantRequest
import com.example.verb.ai.AiAssistantService
/**
 * Explains and answers questions about structural terminal evidence using the user's configured
 * provider. Raw PTY output and command text are deliberately not accepted by this API, so they
 * cannot leave the device through this path. Unknown detail stays unknown rather than being filled
 * from transcript surveillance.
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
        val prompt = evidenceEnvelope(lastCommand, workingDirectoryKnown, sessionState) +
            "Explain what these facts establish, what remains unknown, and the safest next diagnostic step."
        return respond(service, prompt)
    }

    /**
     * The M2 vertical slice: the user asks their own question about this terminal moment, and the
     * model receives exactly the evidence envelope Verb holds -- never more. The prompt instructs
     * the answer to name what it used, so every answer can be checked against the evidence block
     * the UI renders beside it.
     */
    suspend fun ask(
        service: AiAssistantService,
        question: String,
        lastCommand: CommandExecutionRecord?,
        workingDirectoryKnown: Boolean,
        sessionState: TerminalSessionState
    ): String {
        require(question.isNotBlank()) { "The question is empty." }
        val prompt = evidenceEnvelope(lastCommand, workingDirectoryKnown, sessionState) +
            "The user asks about this terminal moment: ${question.trim()}\n" +
            "Answer from the evidence above. Name which facts your answer relies on. " +
            "If the evidence cannot answer, say exactly what is missing."
        return respond(service, prompt)
    }

    /**
     * The evidence this helper can attach, as human-readable lines. Rendered by the UI beside the
     * answer so the user can always see what the model was given -- the answer names its evidence,
     * and here is that evidence.
     */
    fun evidenceLines(
        lastCommand: CommandExecutionRecord?,
        workingDirectoryKnown: Boolean,
        sessionState: TerminalSessionState
    ): List<String> = buildList {
        add("Session state: ${sessionState.name}")
        add("Working directory observed: ${if (workingDirectoryKnown) "yes" else "no"} (path withheld)")
        if (lastCommand == null) {
            add("Last command boundary: unknown")
        } else {
            add("Last command lifecycle: ${lastCommand.state.name}")
            add("Last command exit code: ${lastCommand.exitCode ?: "unknown"}")
            add("Last command duration ms: ${lastCommand.durationMs ?: "unknown"}")
        }
    }

    private fun evidenceEnvelope(
        lastCommand: CommandExecutionRecord?,
        workingDirectoryKnown: Boolean,
        sessionState: TerminalSessionState
    ): String = buildString {
        appendLine("Evidence source: Verb shell integration (structural metadata only).")
        evidenceLines(lastCommand, workingDirectoryKnown, sessionState).forEach(::appendLine)
        appendLine()
    }

    private suspend fun respond(service: AiAssistantService, prompt: String): String =
        service.respond(
            AiAssistantRequest(
                prompt = prompt,
                systemInstruction = TERMINAL_SYSTEM_INSTRUCTION
            )
        ).text
}
