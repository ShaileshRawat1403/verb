package com.example.verb.ai

import kotlinx.coroutines.test.runTest
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class HttpAiProviderClientsTest {

    @Test
    fun providerAdaptersUseTheirDocumentedRequestShapeAndNeverPlaceKeyInBody() = runTest {
        MockWebServer().use { server ->
            server.start()
            val factory = DefaultAiProviderClientFactory(OkHttpClient())
            val prompt = "Explain a safe terminal workflow"

            verifyRequest(
                server = server,
                client = factory(AiProviderId.OPENAI),
                config = AiProviderConfig(AiProviderId.OPENAI, "gpt-test", server.url("/v1").toString().trimEnd('/')),
                apiKey = "openai-test-key",
                responseBody = """{"output":[{"content":[{"type":"output_text","text":"OpenAI answer"}]}]}""",
                expectedPath = "/v1/responses",
                expectedHeader = "Authorization",
                expectedHeaderValue = "Bearer openai-test-key",
                expectedText = "OpenAI answer",
                prompt = prompt
            )
            verifyRequest(
                server = server,
                client = factory(AiProviderId.ANTHROPIC),
                config = AiProviderConfig(AiProviderId.ANTHROPIC, "claude-test", server.url("/").toString().trimEnd('/')),
                apiKey = "anthropic-test-key",
                responseBody = """{"content":[{"type":"text","text":"Anthropic answer"}]}""",
                expectedPath = "/v1/messages",
                expectedHeader = "x-api-key",
                expectedHeaderValue = "anthropic-test-key",
                expectedText = "Anthropic answer",
                prompt = prompt
            )
            verifyRequest(
                server = server,
                client = factory(AiProviderId.GEMINI),
                config = AiProviderConfig(AiProviderId.GEMINI, "gemini-test", server.url("/v1beta").toString().trimEnd('/')),
                apiKey = "gemini-test-key",
                responseBody = """{"candidates":[{"content":{"parts":[{"text":"Gemini answer"}]}}]}""",
                expectedPath = "/v1beta/models/gemini-test:generateContent",
                expectedHeader = "x-goog-api-key",
                expectedHeaderValue = "gemini-test-key",
                expectedText = "Gemini answer",
                prompt = prompt
            )
            verifyRequest(
                server = server,
                client = factory(AiProviderId.OPENAI_COMPATIBLE),
                config = AiProviderConfig(AiProviderId.OPENAI_COMPATIBLE, "compatible-test", server.url("/v1").toString().trimEnd('/')),
                apiKey = "compatible-test-key",
                responseBody = """{"choices":[{"message":{"content":"Compatible answer"}}]}""",
                expectedPath = "/v1/chat/completions",
                expectedHeader = "Authorization",
                expectedHeaderValue = "Bearer compatible-test-key",
                expectedText = "Compatible answer",
                prompt = prompt
            )
        }
    }

    private suspend fun verifyRequest(
        server: MockWebServer,
        client: AiProviderClient,
        config: AiProviderConfig,
        apiKey: String,
        responseBody: String,
        expectedPath: String,
        expectedHeader: String,
        expectedHeaderValue: String,
        expectedText: String,
        prompt: String
    ) {
        server.enqueue(MockResponse().setBody(responseBody).setHeader("Content-Type", "application/json"))

        val response = client.complete(config, apiKey, AiAssistantRequest(prompt))
        val recorded = server.takeRequest()

        assertEquals(expectedPath, recorded.path)
        assertEquals(expectedHeaderValue, recorded.getHeader(expectedHeader))
        assertEquals(expectedText, response.text)
        assertEquals(config.providerId, response.providerId)
        assertEquals(config.model, response.model)
        val requestBody = recorded.body.readUtf8()
        assertEquals(true, requestBody.contains(prompt))
        assertFalse(requestBody.contains(apiKey))
    }
}
