package com.example.verb.ai

interface AiProviderClient {
    suspend fun complete(
        config: AiProviderConfig,
        apiKey: String,
        request: AiAssistantRequest
    ): AiAssistantResponse
}

class AiAssistantService(
    private val settingsStore: AiProviderSettingsStore,
    private val clientFactory: (AiProviderId) -> AiProviderClient
) {
    suspend fun respond(request: AiAssistantRequest): AiAssistantResponse {
        val settings = settingsStore.load()
        val config = settings.config ?: throw AiProviderException("Choose an AI provider first.")
        if (!settings.hasApiKey) throw AiProviderException("Add an API key for ${config.providerId.displayName} first.")
        val apiKey = settingsStore.apiKey()
            ?: throw AiProviderException("The saved API key could not be read. Please add it again.")
        return clientFactory(config.providerId).complete(config, apiKey, request)
    }
}
