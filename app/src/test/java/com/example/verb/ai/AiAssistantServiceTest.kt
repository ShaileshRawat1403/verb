package com.example.verb.ai

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.fail
import org.junit.Test

class AiAssistantServiceTest {

    @Test
    fun sendsOnlyExplicitAssistantPromptToSelectedProvider() = runTest {
        val config = AiProviderConfig(
            providerId = AiProviderId.OPENAI,
            model = "test-model",
            baseUrl = "https://api.openai.com/v1"
        )
        val store = FakeSettingsStore(config, "test-key")
        var receivedRequest: AiAssistantRequest? = null
        var receivedKey: String? = null
        val service = AiAssistantService(store) {
            object : AiProviderClient {
                override suspend fun complete(
                    config: AiProviderConfig,
                    apiKey: String,
                    request: AiAssistantRequest
                ): AiAssistantResponse {
                    receivedKey = apiKey
                    receivedRequest = request
                    return AiAssistantResponse("Answer", config.providerId, config.model)
                }
            }
        }

        val result = service.respond(AiAssistantRequest("Explain git status"))

        assertEquals("Answer", result.text)
        assertEquals("test-key", receivedKey)
        assertEquals("Explain git status", receivedRequest?.prompt)
    }

    @Test
    fun rejectsProviderCallsUntilAKeyIsConfigured() = runTest {
        val store = FakeSettingsStore(
            AiProviderConfig(AiProviderId.GEMINI, "gemini-test", "https://generativelanguage.googleapis.com/v1beta"),
            null
        )
        val service = AiAssistantService(store) {
            throw AssertionError("A client must not be created without a key")
        }

        try {
            service.respond(AiAssistantRequest("Hello"))
            fail("Expected missing-key failure")
        } catch (exception: AiProviderException) {
            assertEquals("Add an API key for Gemini first.", exception.message)
        }
    }

    private class FakeSettingsStore(
        config: AiProviderConfig?,
        private var key: String?
    ) : AiProviderSettingsStore {
        private var settings = AiProviderSettings(config, key != null)

        override fun load(): AiProviderSettings = settings

        override fun save(config: AiProviderConfig, apiKey: String?) {
            key = apiKey ?: key
            settings = AiProviderSettings(config, key != null)
        }

        override fun apiKey(): String? = key

        override fun clearApiKey() {
            key = null
            settings = settings.copy(hasApiKey = false)
        }
    }
}
