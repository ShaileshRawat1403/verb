package com.example.verb.terminal

import com.example.verb.ai.AiAssistantRequest
import com.example.verb.ai.AiAssistantResponse
import com.example.verb.ai.AiAssistantService
import com.example.verb.ai.AiProviderClient
import com.example.verb.ai.AiProviderConfig
import com.example.verb.ai.AiProviderId
import com.example.verb.ai.AiProviderSettings
import com.example.verb.ai.AiProviderSettingsStore
import com.example.verb.session.VerbSessionState
import java.time.Instant
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TerminalAiHelperTest {

    private fun service(capture: (AiAssistantRequest) -> Unit): AiAssistantService {
        val config = AiProviderConfig(AiProviderId.OPENAI, "test", "https://example.invalid")
        return AiAssistantService(FakeStore(config)) {
            object : AiProviderClient {
                override suspend fun complete(
                    config: AiProviderConfig,
                    apiKey: String,
                    request: AiAssistantRequest
                ): AiAssistantResponse {
                    capture(request)
                    return AiAssistantResponse("unknown", config.providerId, config.model)
                }
            }
        }
    }

    private fun command(text: String, exit: Int?, state: CommandLifecycleState) =
        CommandExecutionRecord(
            id = "opaque",
            commandText = text,
            workingDirectory = "/private/client-name",
            startedAtEpochMs = 100,
            endedAtEpochMs = 150,
            exitCode = exit,
            state = state
        )

    @Test
    fun `provider receives structural evidence but never command text or terminal content`() = runTest {
        var received: AiAssistantRequest? = null
        val evidence = TerminalEvidence(
            sessionState = TerminalSessionState.RUNNING,
            workingDirectoryKnown = true,
            shellIntegrationActive = true,
            commandTail = listOf(command("curl -H 'Authorization: planted-secret'", 1, CommandLifecycleState.FAILED))
        )

        TerminalAiHelper.analyze(service { received = it }, evidence)

        val prompt = received!!.prompt
        assertTrue(prompt.contains("exit 1"))
        assertFalse(prompt.contains("planted-secret"))
        assertFalse(prompt.contains("curl"))
        assertFalse(prompt.contains("client-name"))
    }

    /** The M2 surface: the question rides the envelope, prior exchanges ride along, secrets never do. */
    @Test
    fun `a user question is answered from the envelope with the thread and names its sources`() = runTest {
        var received: AiAssistantRequest? = null
        val evidence = TerminalEvidence(
            sessionState = TerminalSessionState.RUNNING,
            workingDirectoryKnown = true,
            shellIntegrationActive = true,
            commandTail = listOf(command("cargo build --features planted-secret", 101, CommandLifecycleState.FAILED)),
            agentWork = listOf(
                AgentWorkFact("Claude Code", VerbSessionState.LIVE, Instant.parse("2026-08-26T10:00:00Z"), "claude")
            )
        )
        val priors = listOf(
            TerminalAiExchange("earlier question one", "earlier answer one"),
            TerminalAiExchange("earlier question two", "earlier answer two")
        )

        TerminalAiHelper.ask(service { received = it }, "Why did the build fail?", priors, evidence)

        val prompt = received!!.prompt
        assertTrue(prompt.contains("Why did the build fail?"))
        assertTrue(prompt.contains("Name which facts your answer relies on"))
        assertTrue(prompt.contains("exit 101"))
        assertTrue(prompt.contains("Claude Code: session LIVE"))
        assertTrue(prompt.contains("earlier question one") && prompt.contains("earlier answer two"))
        assertFalse(prompt.contains("planted-secret"))
        assertFalse(prompt.contains("client-name"))
    }

    /**
     * The envelope speaks the contract's vocabulary because a model should reason over it; the
     * answer is read by a person, so the instruction asks for the plain reading back. Without this
     * the device showed "RECOVERABLE" in the answer beside "recoverable" in the panel.
     */
    @Test
    fun `the model is told to answer in plain state words`() = runTest {
        var received: AiAssistantRequest? = null

        TerminalAiHelper.analyze(
            service { received = it },
            TerminalEvidence(TerminalSessionState.RUNNING, false, false)
        )

        val instruction = received!!.systemInstruction
        assertTrue(instruction.contains("plain words"))
        assertTrue(instruction.contains("recoverable"))
        assertTrue(instruction.contains("not in the uppercase"))
    }

    @Test
    fun `only the last three exchanges ride the prompt`() = runTest {
        var received: AiAssistantRequest? = null
        val evidence = TerminalEvidence(TerminalSessionState.RUNNING, false, false)
        val priors = (1..5).map { TerminalAiExchange("question $it", "answer $it") }

        TerminalAiHelper.ask(service { received = it }, "and now?", priors, evidence)

        val prompt = received!!.prompt
        assertFalse(prompt.contains("question 1"))
        assertFalse(prompt.contains("question 2"))
        assertTrue(prompt.contains("question 3"))
        assertTrue(prompt.contains("question 5"))
    }

    @Test
    fun `a blank question is refused before any provider call`() = runTest {
        try {
            TerminalAiHelper.ask(
                service { throw AssertionError("no provider call may happen") },
                "   ",
                emptyList(),
                TerminalEvidence(TerminalSessionState.RUNNING, false, false)
            )
            throw AssertionError("blank question must throw")
        } catch (expected: IllegalArgumentException) {
            assertTrue(expected.message!!.contains("empty"))
        }
    }

    /**
     * The evidence block rendered beside an answer is Verb's own knowledge, produced by the same
     * function that builds what the model receives, so the two can never disagree.
     */
    @Test
    fun `evidence lines mirror the envelope and withhold paths`() {
        val evidence = TerminalEvidence(
            sessionState = TerminalSessionState.RUNNING,
            workingDirectoryKnown = true,
            shellIntegrationActive = true,
            commandTail = listOf(
                command("alpha-secret-build", 0, CommandLifecycleState.COMPLETED),
                command("beta-secret-deploy", null, CommandLifecycleState.FAILED)
            ),
            agentWork = listOf(
                AgentWorkFact("Codex CLI", VerbSessionState.RECOVERABLE, Instant.parse("2026-08-26T09:00:00Z"))
            )
        )

        val lines = TerminalAiHelper.evidenceLines(evidence)

        assertTrue(lines.any { it.contains("RUNNING") })
        assertTrue(lines.any { it.contains("path withheld") })
        assertTrue(lines.any { it.contains("boundaries reported: yes") })
        // The block says newest first and renders newest first; a label that disagreed with the
        // order would hand the model a false claim about which command was last.
        assertTrue(lines.any { it.contains("newest first") })
        assertTrue(lines.any { it.contains("1. FAILED, exit unknown") })
        assertTrue(lines.any { it.contains("2. COMPLETED, exit 0") })
        assertTrue(lines.any { it.contains("Codex CLI: session RECOVERABLE") })
        assertFalse(
            lines.any {
                it.contains("client-name") || it.contains("alpha-secret-build") || it.contains("beta-secret-deploy")
            }
        )
    }

    private class FakeStore(private val config: AiProviderConfig) : AiProviderSettingsStore {
        override fun load() = AiProviderSettings(config, hasApiKey = true)
        override fun save(config: AiProviderConfig, apiKey: String?) = Unit
        override fun apiKey(): String = "test-key"
        override fun clearApiKey() = Unit
    }
}
