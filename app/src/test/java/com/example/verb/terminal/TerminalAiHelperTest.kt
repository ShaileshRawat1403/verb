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

    private class FakeStore(private val config: AiProviderConfig) : AiProviderSettingsStore {
        override fun load() = AiProviderSettings(config, hasApiKey = true)
        override fun save(config: AiProviderConfig, apiKey: String?) = Unit
        override fun apiKey(): String = "test-key"
        override fun clearApiKey() = Unit
    }
}
