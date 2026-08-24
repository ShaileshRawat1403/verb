package com.example.verb.ai

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

interface AiProviderSettingsStore {
    fun load(): AiProviderSettings
    fun save(config: AiProviderConfig, apiKey: String?)
    fun apiKey(): String?
    fun clearApiKey()
}

/**
 * Stores non-sensitive provider choices in preferences and encrypts the provider API key with an
 * Android Keystore key. The key is never returned in [AiProviderSettings] and is never logged.
 */
class AndroidKeystoreAiProviderSettingsStore(context: Context) : AiProviderSettingsStore {
    private val preferences: SharedPreferences = context.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)

    override fun load(): AiProviderSettings {
        val providerName = preferences.getString(KEY_PROVIDER, null) ?: return AiProviderSettings()
        val provider = runCatching { AiProviderId.valueOf(providerName) }.getOrNull() ?: return AiProviderSettings()
        val model = preferences.getString(KEY_MODEL, "").orEmpty()
        val baseUrl = preferences.getString(KEY_BASE_URL, provider.defaultBaseUrl).orEmpty()
        val hasReadableApiKey = preferences.getString(KEY_ENCRYPTED_API_KEY, null)
            ?.let(::decrypt)
            ?.isNotBlank() == true
        if (!hasReadableApiKey && preferences.contains(KEY_ENCRYPTED_API_KEY)) {
            // A world archive cannot carry the Android Keystore key. A restored ciphertext that
            // this installation cannot decrypt is absence of a usable key, not a signed-in state.
            preferences.edit().remove(KEY_ENCRYPTED_API_KEY).commit()
        }
        return AiProviderSettings(
            config = AiProviderConfig(provider, model, baseUrl),
            hasApiKey = hasReadableApiKey
        )
    }

    override fun save(config: AiProviderConfig, apiKey: String?) {
        require(config.model.isNotBlank()) { "A model is required." }
        require(config.baseUrl.isHttpsUrl()) { "Provider endpoint must use HTTPS." }
        val savedProvider = preferences.getString(KEY_PROVIDER, null)
        if (savedProvider != null && savedProvider != config.providerId.name && apiKey.isNullOrBlank()) {
            throw IllegalArgumentException("Enter an API key for the newly selected provider.")
        }

        preferences.edit()
            .putString(KEY_PROVIDER, config.providerId.name)
            .putString(KEY_MODEL, config.model.trim())
            .putString(KEY_BASE_URL, config.baseUrl.trim().trimEnd('/'))
            .apply()

        apiKey?.trim()?.takeIf { it.isNotEmpty() }?.let { saveEncrypted(KEY_ENCRYPTED_API_KEY, it) }
    }

    override fun apiKey(): String? = preferences.getString(KEY_ENCRYPTED_API_KEY, null)?.let(::decrypt)

    override fun clearApiKey() {
        preferences.edit().remove(KEY_ENCRYPTED_API_KEY).apply()
    }

    private fun saveEncrypted(preferenceKey: String, plainText: String) {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, secretKey())
        val cipherText = cipher.doFinal(plainText.toByteArray(Charsets.UTF_8))
        val encoded = listOf(
            Base64.encodeToString(cipher.iv, Base64.NO_WRAP),
            Base64.encodeToString(cipherText, Base64.NO_WRAP)
        ).joinToString(ENCRYPTED_VALUE_SEPARATOR)
        preferences.edit().putString(preferenceKey, encoded).apply()
    }

    private fun decrypt(encoded: String): String? = runCatching {
        val components = encoded.split(ENCRYPTED_VALUE_SEPARATOR, limit = 2)
        require(components.size == 2)
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(
            Cipher.DECRYPT_MODE,
            secretKey(),
            GCMParameterSpec(GCM_TAG_LENGTH_BITS, Base64.decode(components[0], Base64.NO_WRAP))
        )
        String(cipher.doFinal(Base64.decode(components[1], Base64.NO_WRAP)), Charsets.UTF_8)
    }.getOrNull()

    private fun secretKey(): SecretKey {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        (keyStore.getEntry(KEYSTORE_ALIAS, null) as? KeyStore.SecretKeyEntry)?.secretKey?.let { return it }

        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE).run {
            init(
                KeyGenParameterSpec.Builder(
                    KEYSTORE_ALIAS,
                    KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
                )
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .setKeySize(256)
                    .build()
            )
            generateKey()
        }
    }

    private fun String.isHttpsUrl(): Boolean = startsWith("https://", ignoreCase = true)

    private companion object {
        const val PREFERENCES_NAME = "verb_ai_provider_settings"
        const val KEY_PROVIDER = "provider"
        const val KEY_MODEL = "model"
        const val KEY_BASE_URL = "base_url"
        const val KEY_ENCRYPTED_API_KEY = "encrypted_api_key"
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val KEYSTORE_ALIAS = "verb_ai_provider_key_v1"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val GCM_TAG_LENGTH_BITS = 128
        const val ENCRYPTED_VALUE_SEPARATOR = ":"
    }
}
