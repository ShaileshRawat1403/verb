package com.example.verb.terminal

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
}
