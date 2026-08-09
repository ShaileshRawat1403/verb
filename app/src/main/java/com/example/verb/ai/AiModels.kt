package com.example.verb.ai

/**
 * The providers Verb can address directly. Provider authentication remains local to the app and
 * is intentionally independent from credentials a user may configure for terminal CLIs.
 */
enum class AiProviderId(
    val displayName: String,
    val defaultBaseUrl: String
) {
    OPENAI("OpenAI", "https://api.openai.com/v1"),
    ANTHROPIC("Anthropic", "https://api.anthropic.com"),
    GEMINI("Gemini", "https://generativelanguage.googleapis.com/v1beta"),
    OPENAI_COMPATIBLE("OpenAI-compatible", "")
}

data class AiProviderConfig(
    val providerId: AiProviderId,
    val model: String,
    val baseUrl: String = providerId.defaultBaseUrl
)

data class AiProviderSettings(
    val config: AiProviderConfig? = null,
    val hasApiKey: Boolean = false
) {
    val isReady: Boolean
        get() = hasApiKey && config?.model?.isNotBlank() == true && config.baseUrl.isNotBlank()
}

data class AiAssistantRequest(
    val prompt: String,
    val systemInstruction: String = DEFAULT_SYSTEM_INSTRUCTION
) {
    companion object {
        const val DEFAULT_SYSTEM_INSTRUCTION =
            "You are Verb's mobile assistant. Give concise, helpful answers. " +
                "You may explain commands and propose steps, but you cannot execute terminal " +
                "commands or device actions. Never claim that an action was performed unless " +
                "the user supplied direct evidence."
    }
}

data class AiAssistantResponse(
    val text: String,
    val providerId: AiProviderId,
    val model: String
)

sealed interface AiAssistantState {
    data object Idle : AiAssistantState
    data object Generating : AiAssistantState
    data class Answer(val response: AiAssistantResponse) : AiAssistantState
    data class Failure(val message: String) : AiAssistantState
}

class AiProviderException(message: String, cause: Throwable? = null) : Exception(message, cause)
