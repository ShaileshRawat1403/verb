package com.example.verb.ai

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import org.json.JSONArray
import org.json.JSONObject
import java.io.IOException
import java.util.concurrent.TimeUnit

/** Factory for the four supported text-generation REST protocols. */
class DefaultAiProviderClientFactory(
    private val httpClient: OkHttpClient = defaultHttpClient()
) {
    operator fun invoke(providerId: AiProviderId): AiProviderClient = when (providerId) {
        AiProviderId.OPENAI -> OpenAiResponsesClient(httpClient)
        AiProviderId.ANTHROPIC -> AnthropicMessagesClient(httpClient)
        AiProviderId.GEMINI -> GeminiGenerateContentClient(httpClient)
        // Same wire format as any other OpenAI-compatible endpoint; only the default
        // base URL and model suggestions differ.
        AiProviderId.DEEPSEEK -> OpenAiCompatibleClient(httpClient)
        AiProviderId.OPENAI_COMPATIBLE -> OpenAiCompatibleClient(httpClient)
    }

    private companion object {
        fun defaultHttpClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(20, TimeUnit.SECONDS)
            .readTimeout(90, TimeUnit.SECONDS)
            .callTimeout(120, TimeUnit.SECONDS)
            .build()
    }
}

private abstract class HttpAiProviderClient(
    private val httpClient: OkHttpClient
) : AiProviderClient {
    protected suspend fun postJson(request: Request): JSONObject = withContext(Dispatchers.IO) {
        try {
            httpClient.newCall(request).execute().use { response ->
                if (!response.isSuccessful) {
                    throw AiProviderException("Provider request failed with HTTP ${response.code}.")
                }
                val responseText = response.body?.string().orEmpty()
                if (responseText.isBlank()) throw AiProviderException("Provider returned an empty response.")
                JSONObject(responseText)
            }
        } catch (exception: AiProviderException) {
            throw exception
        } catch (exception: IOException) {
            throw AiProviderException("Could not reach the provider. Check your connection and endpoint.", exception)
        } catch (exception: Exception) {
            throw AiProviderException("Provider returned an unreadable response.", exception)
        }
    }

    protected fun jsonRequest(url: String, body: JSONObject, configure: Request.Builder.() -> Unit): Request =
        Request.Builder()
            .url(url)
            .header("Content-Type", JSON_MEDIA_TYPE.toString())
            .post(body.toString().toRequestBody(JSON_MEDIA_TYPE))
            .apply(configure)
            .build()

    protected fun endpoint(baseUrl: String, vararg pathSegments: String): String {
        val base = baseUrl.trimEnd('/').toHttpUrlOrNull()
            ?: throw AiProviderException("The provider endpoint is invalid.")
        return base.newBuilder().apply {
            pathSegments.forEach(::addPathSegment)
        }.build().toString()
    }

    protected fun modelName(model: String): String = model.trim().takeIf {
        it.isNotEmpty() && it.length <= 200 && it.none(Char::isISOControl)
    } ?: throw AiProviderException("Enter a valid model name.")

    protected fun responseTextOrThrow(text: String?, provider: AiProviderId, model: String): AiAssistantResponse =
        text?.trim()?.takeIf { it.isNotEmpty() }
            ?.let { AiAssistantResponse(it, provider, model) }
            ?: throw AiProviderException("Provider returned no text response.")

    private companion object {
        val JSON_MEDIA_TYPE = "application/json; charset=utf-8".toMediaType()
    }
}

private class OpenAiResponsesClient(httpClient: OkHttpClient) : HttpAiProviderClient(httpClient) {
    override suspend fun complete(
        config: AiProviderConfig,
        apiKey: String,
        request: AiAssistantRequest
    ): AiAssistantResponse {
        val body = JSONObject()
            .put("model", modelName(config.model))
            .put("instructions", request.systemInstruction)
            .put("input", request.prompt)
            .put("max_output_tokens", MAX_OUTPUT_TOKENS)
        val response = postJson(
            jsonRequest(endpoint(config.baseUrl, "responses"), body) {
                header("Authorization", "Bearer $apiKey")
            }
        )
        val topLevelText = response.optString("output_text").takeIf { it.isNotBlank() }
        val nestedText = response.optJSONArray("output")
            ?.firstContentText(type = "output_text")
        return responseTextOrThrow(topLevelText ?: nestedText, AiProviderId.OPENAI, config.model)
    }
}

private class AnthropicMessagesClient(httpClient: OkHttpClient) : HttpAiProviderClient(httpClient) {
    override suspend fun complete(
        config: AiProviderConfig,
        apiKey: String,
        request: AiAssistantRequest
    ): AiAssistantResponse {
        val messages = JSONArray().put(
            JSONObject()
                .put("role", "user")
                .put("content", request.prompt)
        )
        val body = JSONObject()
            .put("model", modelName(config.model))
            .put("max_tokens", MAX_OUTPUT_TOKENS)
            .put("system", request.systemInstruction)
            .put("messages", messages)
        val response = postJson(
            jsonRequest(endpoint(config.baseUrl, "v1", "messages"), body) {
                header("x-api-key", apiKey)
                header("anthropic-version", ANTHROPIC_VERSION)
            }
        )
        return responseTextOrThrow(
            response.optJSONArray("content")?.firstContentText(type = "text"),
            AiProviderId.ANTHROPIC,
            config.model
        )
    }
}

private class GeminiGenerateContentClient(httpClient: OkHttpClient) : HttpAiProviderClient(httpClient) {
    override suspend fun complete(
        config: AiProviderConfig,
        apiKey: String,
        request: AiAssistantRequest
    ): AiAssistantResponse {
        val content = JSONObject()
            .put("role", "user")
            .put("parts", JSONArray().put(JSONObject().put("text", request.prompt)))
        val body = JSONObject()
            .put(
                "system_instruction",
                JSONObject().put("parts", JSONArray().put(JSONObject().put("text", request.systemInstruction)))
            )
            .put("contents", JSONArray().put(content))
            .put("generationConfig", JSONObject().put("maxOutputTokens", MAX_OUTPUT_TOKENS))
        val response = postJson(
            jsonRequest(endpoint(config.baseUrl, "models", "${modelName(config.model)}:generateContent"), body) {
                header("x-goog-api-key", apiKey)
            }
        )
        val text = response.optJSONArray("candidates")
            ?.optJSONObject(0)
            ?.optJSONObject("content")
            ?.optJSONArray("parts")
            ?.firstContentText(type = null)
        return responseTextOrThrow(text, AiProviderId.GEMINI, config.model)
    }
}

private class OpenAiCompatibleClient(httpClient: OkHttpClient) : HttpAiProviderClient(httpClient) {
    override suspend fun complete(
        config: AiProviderConfig,
        apiKey: String,
        request: AiAssistantRequest
    ): AiAssistantResponse {
        val messages = JSONArray()
            .put(JSONObject().put("role", "system").put("content", request.systemInstruction))
            .put(JSONObject().put("role", "user").put("content", request.prompt))
        val body = JSONObject()
            .put("model", modelName(config.model))
            .put("messages", messages)
        val response = postJson(
            jsonRequest(endpoint(config.baseUrl, "chat", "completions"), body) {
                header("Authorization", "Bearer $apiKey")
            }
        )
        val text = response.optJSONArray("choices")
            ?.optJSONObject(0)
            ?.optJSONObject("message")
            ?.optString("content")
        return responseTextOrThrow(text, AiProviderId.OPENAI_COMPATIBLE, config.model)
    }
}

private fun JSONArray.firstContentText(type: String?): String? {
    for (index in 0 until length()) {
        val item = optJSONObject(index) ?: continue
        if (type == null || item.optString("type") == type) {
            item.optString("text").takeIf { it.isNotBlank() }?.let { return it }
        }
        item.optJSONArray("content")?.firstContentText(type)?.let { return it }
    }
    return null
}

private const val MAX_OUTPUT_TOKENS = 1_024
private const val ANTHROPIC_VERSION = "2023-06-01"
