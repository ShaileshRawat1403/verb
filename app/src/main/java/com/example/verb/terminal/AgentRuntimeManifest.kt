package com.example.verb.terminal

import java.io.File

/**
 * Immutable description of a Linux agent runtime artifact.
 *
 * The manifest is deliberately data-only. Agent binaries are never selected from PATH and an
 * artifact is never activated without matching its declared architecture, digest, and required
 * commands. This runtime is separate from the Verb/Termux-compatible CLI userland.
 */
data class AgentRuntimeManifest(
    val runtimeVersion: String,
    val architecture: String,
    val rootfsSha256: String,
    val distro: String,
    val nodeVersion: String,
    val claudeVersion: String,
    val openCodeVersion: String,
    val minimumVerbVersion: String,
    val createdAt: String,
    val requiredCommands: List<String> = listOf("/bin/bash", "/usr/bin/node", "/usr/bin/npm")
) {
    fun validateForArm64(): Result<Unit> {
        if (runtimeVersion.isBlank() || architecture != "aarch64") {
            return Result.failure(IllegalArgumentException("Agent runtime manifest has an invalid identity."))
        }
        if (!rootfsSha256.matches(Regex("[0-9a-fA-F]{64}"))) {
            return Result.failure(IllegalArgumentException("Agent runtime manifest has an invalid SHA-256."))
        }
        if (distro.isBlank() || nodeVersion.isBlank() || claudeVersion.isBlank() || openCodeVersion.isBlank()) {
            return Result.failure(IllegalArgumentException("Agent runtime manifest is incomplete."))
        }
        if (requiredCommands.isEmpty() || requiredCommands.any { !it.startsWith('/') }) {
            return Result.failure(IllegalArgumentException("Agent runtime command paths must be absolute."))
        }
        return Result.success(Unit)
    }

    fun toPropertiesText(): String = buildString {
        appendLine("runtimeVersion=$runtimeVersion")
        appendLine("architecture=$architecture")
        appendLine("rootfsSha256=$rootfsSha256")
        appendLine("distro=$distro")
        appendLine("nodeVersion=$nodeVersion")
        appendLine("claudeVersion=$claudeVersion")
        appendLine("openCodeVersion=$openCodeVersion")
        appendLine("minimumVerbVersion=$minimumVerbVersion")
        appendLine("createdAt=$createdAt")
        appendLine("requiredCommands=${requiredCommands.joinToString(",")}")
    }

    companion object {
        fun fromFile(file: File): Result<AgentRuntimeManifest> = runCatching {
            require(file.isFile) { "Agent runtime manifest is unavailable." }
            val values = linkedMapOf<String, String>()
            file.forEachLine { line ->
                val trimmed = line.trim()
                if (trimmed.isEmpty() || trimmed.startsWith("#")) return@forEachLine
                val separator = trimmed.indexOf('=')
                require(separator > 0) { "Agent runtime manifest contains an invalid line." }
                values[trimmed.substring(0, separator)] = trimmed.substring(separator + 1)
            }
            AgentRuntimeManifest(
                runtimeVersion = values.required("runtimeVersion"),
                architecture = values.required("architecture"),
                rootfsSha256 = values.required("rootfsSha256"),
                distro = values.required("distro"),
                nodeVersion = values.required("nodeVersion"),
                claudeVersion = values.required("claudeVersion"),
                openCodeVersion = values.required("openCodeVersion"),
                minimumVerbVersion = values.required("minimumVerbVersion"),
                createdAt = values.required("createdAt"),
                requiredCommands = values.required("requiredCommands")
                    .split(',')
                    .map(String::trim)
                    .filter(String::isNotEmpty)
            ).also { manifest -> manifest.validateForArm64().getOrThrow() }
        }

        private fun Map<String, String>.required(key: String): String =
            get(key)?.takeIf(String::isNotBlank)
                ?: error("Agent runtime manifest is missing $key.")
    }
}
