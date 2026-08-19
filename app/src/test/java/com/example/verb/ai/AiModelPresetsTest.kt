package com.example.verb.ai

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AiModelPresetsTest {
    @Test
    fun `provider presets are local suggestions and compatible endpoints allow custom IDs`() {
        assertTrue(AiModelPresets.forProvider(AiProviderId.OPENAI).isNotEmpty())
        assertTrue(AiModelPresets.forProvider(AiProviderId.ANTHROPIC).isNotEmpty())
        assertTrue(AiModelPresets.forProvider(AiProviderId.GEMINI).isNotEmpty())
        assertEquals(emptyList<String>(), AiModelPresets.forProvider(AiProviderId.OPENAI_COMPATIBLE))
    }

    /**
     * DeepSeek reuses the OpenAI-compatible transport, so the thing worth pinning is that it still
     * arrives with a usable endpoint and suggestions of its own -- the reason for listing it
     * separately rather than telling users to pick "OpenAI-compatible" and find the URL themselves.
     */
    @Test
    fun `DeepSeek ships a default endpoint and model suggestions`() {
        assertEquals("https://api.deepseek.com/v1", AiProviderId.DEEPSEEK.defaultBaseUrl)
        assertTrue(AiModelPresets.forProvider(AiProviderId.DEEPSEEK).isNotEmpty())
    }

    @Test
    fun `every provider except the generic one has a default endpoint`() {
        AiProviderId.entries
            .filter { it != AiProviderId.OPENAI_COMPATIBLE }
            .forEach { assertTrue("$it needs a default base URL", it.defaultBaseUrl.isNotBlank()) }
    }
}
