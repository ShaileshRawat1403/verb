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
    /**
     * DeepSeek serves an OpenAI-compatible chat-completions API, so it reuses that transport rather
     * than getting a client of its own. It is listed separately anyway because a first-class entry
     * carries the correct base URL and model suggestions, which "OpenAI-compatible" cannot: that
     * option leaves the user to find and type the endpoint themselves.
     */
    DEEPSEEK("DeepSeek", "https://api.deepseek.com/v1"),
    OPENAI_COMPATIBLE("OpenAI-compatible", "")
}

/**
 * Locally bundled suggestions for common provider model IDs. They are not an account capability
 * check: availability depends on the user's account and endpoint, and custom IDs remain allowed.
 */
object AiModelPresets {
    fun forProvider(provider: AiProviderId): List<String> = when (provider) {
        AiProviderId.OPENAI -> listOf("gpt-5", "gpt-5-mini", "gpt-4.1")
        AiProviderId.ANTHROPIC -> listOf("claude-sonnet-4", "claude-opus-4", "claude-3-5-haiku-latest")
        AiProviderId.GEMINI -> listOf("gemini-2.5-pro", "gemini-2.5-flash", "gemini-2.0-flash")
        // Suggestions only, as above: the account and endpoint decide what actually exists, and a
        // model id typed by the user is always allowed.
        AiProviderId.DEEPSEEK -> listOf("deepseek-chat", "deepseek-reasoner")
        AiProviderId.OPENAI_COMPATIBLE -> emptyList()
    }
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
