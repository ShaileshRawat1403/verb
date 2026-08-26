package com.example.verb.terminal

import com.example.verb.ai.AiAssistantRequest
import com.example.verb.ai.AiAssistantService
import com.example.verb.session.VerbSessionState
import java.time.Instant

/**
 * The evidence Verb attaches when the user asks about their work. Everything here is structural
 * fact Verb observed itself; command text, PTY output and absolute paths are deliberately absent,
 * so they cannot leave the device through this path. Unknown detail stays unknown rather than
 * being filled from transcript surveillance.
 */
data class TerminalEvidence(
    val sessionState: TerminalSessionState,
    val workingDirectoryKnown: Boolean,
    val shellIntegrationActive: Boolean,
    /** The newest command boundaries last; only lifecycle facts travel, never command text. */
    val commandTail: List<CommandExecutionRecord> = emptyList(),
    /** One entry per agent session Verb holds. */
    val agentWork: List<AgentWorkFact> = emptyList()
)

/**
 * What Verb knows about one agent session, as fields rather than as a formatted line.
 *
 * This is structured on purpose. An earlier shape accepted pre-formatted strings from the caller,
 * which punched a free-text hole straight through the privacy boundary this type exists to be: a
 * caller could put an absolute path or a transcript excerpt in it and nothing here would notice.
 * Fields cannot carry what fields do not have.
 */
data class AgentWorkFact(
    /** The runtime profile's display name -- "Claude Code", not a path. */
    val profileName: String,
    val sessionState: VerbSessionState,
    val lastSeenAt: Instant,
    /** The agent kind the session record names, when one is bound. Never a resume identity. */
    val agentType: String? = null
)

/** One question and its evidence-bound answer, kept so a follow-up can build on the last ones. */
data class TerminalAiExchange(
    val question: String,
    val answer: String
)

object TerminalAiHelper {

    /**
     * The last sentence exists because the envelope legitimately speaks the contract's vocabulary
     * and the model was quoting it straight back -- a user reading "RECOVERABLE" beside a panel
     * that says "recoverable" is reading two spellings of one fact. `docs/UX_FOUNDATION.md`: plain
     * language on screen, exact vocabulary underneath, and the answer is on screen.
     */
    private const val TERMINAL_SYSTEM_INSTRUCTION =
        "You are Verb's evidence-bound terminal assistant. Use only the structural facts provided. " +
            "Say when the evidence cannot explain a cause. Keep answers brief and propose at most " +
            "two diagnostic actions. Never claim an action ran and never invent terminal content. " +
            "Write session and command states in plain words -- running, recoverable, recovery " +
            "status unknown, ended, failed, finished, never finished -- not in the uppercase " +
            "spellings the evidence uses."

    /** How much of the conversation rides along so a follow-up needs no restating. */
    private const val MAX_PRIOR_EXCHANGES = 3

    suspend fun analyze(service: AiAssistantService, evidence: TerminalEvidence): String {
        val prompt = evidenceEnvelope(evidence, emptyList()) +
            "Explain what these facts establish, what remains unknown, and the safest next diagnostic step."
        return respond(service, prompt)
    }

    /**
     * The M2 surface: the user asks their own question about their work, and the model receives
     * exactly the evidence envelope Verb holds -- plus at most the last few exchanges of this
     * conversation, so a follow-up never has to restate what Verb already heard. The prompt
     * requires the answer to name the facts it relied on; the UI renders the same evidence beside
     * it, built by [evidenceLines] from the same snapshot.
     */
    suspend fun ask(
        service: AiAssistantService,
        question: String,
        priorExchanges: List<TerminalAiExchange>,
        evidence: TerminalEvidence
    ): String {
        require(question.isNotBlank()) { "The question is empty." }
        val prompt = evidenceEnvelope(evidence, priorExchanges.takeLast(MAX_PRIOR_EXCHANGES)) +
            "The user asks about this work: ${question.trim()}\n" +
            "Answer from the evidence above. Name which facts your answer relies on. " +
            "If the evidence cannot answer, say exactly what is missing."
        return respond(service, prompt)
    }

    /**
     * The evidence this helper attaches, as the lines that actually cross the provider boundary.
     *
     * These carry the contract's exact vocabulary (`RUNNING`, `LIVE`, `FAILED`) because that is
     * what a model should reason over. The user sees the same facts read back in plain language by
     * `com.example.verb.ui.AssistEvidence`, from this same snapshot --
     * `docs/UX_FOUNDATION.md`: plain language on screen, exact vocabulary underneath.
     */
    fun evidenceLines(evidence: TerminalEvidence): List<String> = buildList {
        add("Session state: ${evidence.sessionState.name}")
        add("Working directory observed: ${if (evidence.workingDirectoryKnown) "yes" else "no"} (path withheld)")
        add("Shell command boundaries reported: ${if (evidence.shellIntegrationActive) "yes" else "no"}")
        if (evidence.commandTail.isEmpty()) {
            add("Command boundaries recorded: none")
        } else {
            add(
                if (evidence.commandTail.size == 1) {
                    "Last command boundary (text withheld):"
                } else {
                    "Last ${evidence.commandTail.size} command boundaries (newest first, text withheld):"
                }
            )
            evidence.commandTail.asReversed().forEachIndexed { index, record ->
                add(
                    "  ${index + 1}. ${record.state.name}, exit ${record.exitCode ?: "unknown"}, " +
                        "duration ${record.durationMs ?: "unknown"} ms"
                )
            }
        }
        evidence.agentWork.forEach { fact ->
            val agent = fact.agentType?.let { " ($it)" } ?: ""
            add("${fact.profileName}: session ${fact.sessionState.name}, last seen ${fact.lastSeenAt}$agent")
        }
    }

    private fun evidenceEnvelope(
        evidence: TerminalEvidence,
        priorExchanges: List<TerminalAiExchange>
    ): String = buildString {
        appendLine("Evidence source: Verb shell integration and session records (structural metadata only).")
        evidenceLines(evidence).forEach(::appendLine)
        if (priorExchanges.isNotEmpty()) {
            appendLine()
            appendLine("Earlier in this conversation (oldest first):")
            priorExchanges.forEach { exchange ->
                appendLine("Q: ${exchange.question}")
                appendLine("A: ${exchange.answer.take(600)}")
            }
        }
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
