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
}
