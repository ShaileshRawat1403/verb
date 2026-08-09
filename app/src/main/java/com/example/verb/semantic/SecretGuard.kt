package com.example.verb.semantic

import com.example.verb.model.ActionRisk
import com.example.verb.model.DetectionConfidence
import com.example.verb.model.EntityType
import com.example.verb.model.SemanticEntity

object SecretGuard {

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
        if (secretPatterns.any { it.containsMatchIn(text) }) {
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
}
