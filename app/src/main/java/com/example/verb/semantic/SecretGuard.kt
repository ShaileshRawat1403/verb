package com.example.verb.semantic

import com.example.verb.model.ActionRisk
import com.example.verb.model.DetectionConfidence
import com.example.verb.model.EntityType
import com.example.verb.model.SemanticEntity

object SecretGuard {

    private const val REDACTED_SENSITIVE_CONTENT = "[REDACTED_SENSITIVE_CONTENT]"

    private val secretPatterns = listOf(
        Regex("-----BEGIN (?:RSA )?PRIVATE KEY-----"),
        Regex("Authorization:\\s*Bearer\\s+[\\w\\-.]+"),
        Regex("(?i)password\\s*=\\s*\\S+"),
        Regex("(?i)secret\\s*=\\s*\\S+"),
        Regex("(?i)token\\s*=\\s*\\S+"),
        Regex("(?i)api_?key\\s*=\\s*\\S+"),
        Regex("(?i)AWS_SECRET_ACCESS_KEY(?:\\s*=|:\\s*)\\s*\\S+")
    )

    fun checkSensitive(text: String): SemanticEntity? {
        if (containsKnownSensitivePattern(text)) {
            return SemanticEntity(
                rawText = "******** (Redacted)",
                entityType = EntityType.SENSITIVE_TEXT,
                title = "Sensitive Text",
                description = "Credential or sensitive material detected. Remote analysis disabled.",
                risk = ActionRisk.READ_ONLY,
                isSensitive = true,
                confidence = DetectionConfidence.HIGH,
                detectionMethod = "SECRET_PATTERN"
            )
        }
        return null
    }

    /**
     * Removes an entire value when it matches one of Verb's known local secret patterns.
     *
     * This is deliberately conservative: it is a prerequisite for future bounded context,
     * not permission to retain arbitrary terminal input or output.
     */
    fun redactKnownSensitiveText(text: String): String =
        if (containsKnownSensitivePattern(text)) REDACTED_SENSITIVE_CONTENT else text

    private fun containsKnownSensitivePattern(text: String): Boolean =
        secretPatterns.any { it.containsMatchIn(text) }
}
