package com.example.verb.terminal

import com.example.verb.ai.AiAssistantRequest
import com.example.verb.ai.AiAssistantResponse
import com.example.verb.ai.AiAssistantService
import com.example.verb.ai.AiProviderClient
import com.example.verb.ai.AiProviderConfig
import com.example.verb.ai.AiProviderId
import com.example.verb.ai.AiProviderSettings
import com.example.verb.ai.AiProviderSettingsStore
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TerminalAiHelperTest {
    @Test
    fun `provider receives structural evidence but never command text or terminal content`() = runTest {
        var receivedRequest: AiAssistantRequest? = null
        val config = AiProviderConfig(AiProviderId.OPENAI, "test", "https://example.invalid")
        val service = AiAssistantService(FakeStore(config)) {
            object : AiProviderClient {
                override suspend fun complete(
                    config: AiProviderConfig,
                    apiKey: String,
                    request: AiAssistantRequest
                ): AiAssistantResponse {
                    receivedRequest = request
                    return AiAssistantResponse("unknown", config.providerId, config.model)
                }
            }
        }
        val lastCommand = CommandExecutionRecord(
            id = "opaque",
            commandText = "curl -H 'Authorization: planted-secret' https://client.invalid",
            workingDirectory = "/private/client-name",
            startedAtEpochMs = 100,
            endedAtEpochMs = 150,
            exitCode = 1,
            state = CommandLifecycleState.FAILED
        )

        TerminalAiHelper.analyze(service, lastCommand, true, TerminalSessionState.RUNNING)

        val prompt = receivedRequest!!.prompt
        assertTrue(prompt.contains("exit code: 1"))
        assertTrue(prompt.contains("duration ms: 50"))
        assertFalse(prompt.contains("planted-secret"))
        assertFalse(prompt.contains("curl"))
        assertFalse(prompt.contains("client-name"))
        assertFalse(prompt.contains("terminal output"))
    }

    /** The M2 slice: the user's question rides the evidence envelope, and nothing else does. */
    @Test
    fun `a user question is answered from the evidence envelope and names its sources`() = runTest {
        var receivedRequest: AiAssistantRequest? = null
        val config = AiProviderConfig(AiProviderId.OPENAI, "test", "https://example.invalid")
        val service = AiAssistantService(FakeStore(config)) {
            object : AiProviderClient {
                override suspend fun complete(
                    config: AiProviderConfig,
                    apiKey: String,
                    request: AiAssistantRequest
                ): AiAssistantResponse {
                    receivedRequest = request
                    return AiAssistantResponse("unknown", config.providerId, config.model)
                }
            }
        }
        val lastCommand = CommandExecutionRecord(
            id = "opaque",
            commandText = "cargo build --features planted-secret",
            workingDirectory = "/private/client-name",
            startedAtEpochMs = 100,
            endedAtEpochMs = 150,
            exitCode = 101,
            state = CommandLifecycleState.FAILED
        )

        TerminalAiHelper.ask(
            service = service,
            question = "Why did the build fail?",
            lastCommand = lastCommand,
            workingDirectoryKnown = true,
            sessionState = TerminalSessionState.RUNNING
        )

        val prompt = receivedRequest!!.prompt
        assertTrue(prompt.contains("Why did the build fail?"))
        assertTrue(prompt.contains("Name which facts your answer relies on"))
        assertTrue(prompt.contains("exit code: 101"))
        // The same privacy boundary as the canned analysis: command text and paths never cross.
        assertFalse(prompt.contains("planted-secret"))
        assertFalse(prompt.contains("client-name"))
    }

    @Test
    fun `a blank question is refused before any provider call`() = runTest {
        val config = AiProviderConfig(AiProviderId.OPENAI, "test", "https://example.invalid")
        val service = AiAssistantService(FakeStore(config)) {
            object : AiProviderClient {
                override suspend fun complete(
                    config: AiProviderConfig,
                    apiKey: String,
                    request: AiAssistantRequest
                ): AiAssistantResponse {
                    throw AssertionError("no provider call may happen for a blank question")
                }
            }
        }

        try {
            TerminalAiHelper.ask(service, "   ", null, false, TerminalSessionState.RUNNING)
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
        val lastCommand = CommandExecutionRecord(
            id = "opaque",
            commandText = "rm -rf /private/client-name",
            workingDirectory = "/private/client-name",
            startedAtEpochMs = 100,
            endedAtEpochMs = 150,
            exitCode = null,
            state = CommandLifecycleState.COMPLETED
        )

        val lines = TerminalAiHelper.evidenceLines(lastCommand, true, TerminalSessionState.RUNNING)

        assertTrue(lines.any { it.contains("RUNNING") })
        assertTrue(lines.any { it.contains("exit code: unknown") })
        assertTrue(lines.any { it.contains("path withheld") })
        assertFalse(lines.any { it.contains("client-name") || it.contains("rm -rf") })
    }

    private class FakeStore(private val config: AiProviderConfig) : AiProviderSettingsStore {
        override fun load() = AiProviderSettings(config, hasApiKey = true)
        override fun save(config: AiProviderConfig, apiKey: String?) = Unit
        override fun apiKey(): String = "test-key"
        override fun clearApiKey() = Unit
    }
}
