package com.example.verb.terminal

import java.io.File

enum class RuntimeProfileId {
    CORE,
    PYTHON,
    HERMES,
    JAVASCRIPT,
    CODEX,
    CLAUDE_CODE,
    GEMINI_CLI,
    NATIVE,
    REMOTE,
    DATA_MEDIA
}

data class RuntimeRequirement(
    val command: String,
    val packageName: String,
    val maxVersionExclusive: String? = null
)

data class RuntimeProfile(
    val id: RuntimeProfileId,
    val displayName: String,
    val packages: List<String>,
    val requirements: List<RuntimeRequirement>,
    val prerequisiteProfiles: List<RuntimeProfileId> = emptyList(),
    val installCommandOverride: String? = null,
    val postInstallHint: String? = null
) {
    /** Safe because package names are catalog-owned, not user-provided shell input. */
    val installCommand: String
        get() = installCommandOverride
            ?: "apt-get update && apt-get install -y --no-install-recommends ${packages.joinToString(" ")}"
}

data class RuntimeProfileReport(
    val profile: RuntimeProfile,
    val missingPackages: List<String>,
    val missingCommands: List<String>,
    val incompatibleCommands: List<String>
) {
    val isReady: Boolean
        get() = missingPackages.isEmpty() && missingCommands.isEmpty() && incompatibleCommands.isEmpty()
}

object RuntimeProfiles {
    val all: List<RuntimeProfile> = listOf(
        RuntimeProfile(
            RuntimeProfileId.CORE,
            "Core CLI",
            listOf("ca-certificates", "curl", "git", "tar"),
            listOf(
                RuntimeRequirement("bash", "bash"),
                RuntimeRequirement("apt-get", "apt"),
                RuntimeRequirement("curl", "curl"),
                RuntimeRequirement("git", "git")
            )
        ),
        RuntimeProfile(
            RuntimeProfileId.PYTHON,
            "Python",
            listOf("python"),
            listOf(RuntimeRequirement("python", "python"))
        ),
        RuntimeProfile(
            RuntimeProfileId.HERMES,
            "Hermes Agent",
            listOf("python"),
            listOf(RuntimeRequirement("python", "python", maxVersionExclusive = "3.14"))
        ),
        RuntimeProfile(
            RuntimeProfileId.JAVASCRIPT,
            "JavaScript",
            listOf("nodejs-lts", "npm"),
            listOf(
                RuntimeRequirement("node", "nodejs-lts"),
                RuntimeRequirement("npm", "npm")
            )
        ),
        RuntimeProfile(
            RuntimeProfileId.CODEX,
            "Codex CLI",
            emptyList(),
            listOf(RuntimeRequirement("codex", "")),
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT),
            installCommandOverride = "npm install -g @openai/codex",
            postInstallHint = "In Terminal, run codex and complete its sign-in flow."
        ),
        RuntimeProfile(
            RuntimeProfileId.CLAUDE_CODE,
            "Claude Code",
            emptyList(),
            listOf(RuntimeRequirement("claude", "")),
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT),
            installCommandOverride = "npm install -g @anthropic-ai/claude-code",
            postInstallHint = "In Terminal, run claude and complete its sign-in flow."
        ),
        RuntimeProfile(
            RuntimeProfileId.GEMINI_CLI,
            "Gemini CLI",
            emptyList(),
            listOf(RuntimeRequirement("gemini", "")),
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT),
            installCommandOverride = "npm install -g @google/gemini-cli",
            postInstallHint = "In Terminal, run gemini and complete its sign-in flow."
        ),
        RuntimeProfile(
            RuntimeProfileId.NATIVE,
            "Native Development",
            listOf("clang", "make", "cmake", "pkg-config", "rust"),
            listOf(
                RuntimeRequirement("clang", "clang"),
                RuntimeRequirement("make", "make"),
                RuntimeRequirement("cmake", "cmake"),
                RuntimeRequirement("pkg-config", "pkg-config"),
                RuntimeRequirement("cargo", "rust")
            )
        ),
        RuntimeProfile(
            RuntimeProfileId.REMOTE,
            "Remote Development",
            listOf("openssh"),
            listOf(RuntimeRequirement("ssh", "openssh"))
        ),
        RuntimeProfile(
            RuntimeProfileId.DATA_MEDIA,
            "Data and Media",
            listOf("jq", "ripgrep", "ffmpeg", "sqlite"),
            listOf(
                RuntimeRequirement("jq", "jq"),
                RuntimeRequirement("rg", "ripgrep"),
                RuntimeRequirement("ffmpeg", "ffmpeg"),
                RuntimeRequirement("sqlite3", "sqlite")
            )
        )
    )

    fun forId(id: RuntimeProfileId): RuntimeProfile = all.first { it.id == id }
}

class RuntimeCapabilityDetector(
    private val filesDir: File
) {
    private val prefixDir = File(filesDir, "usr")
    private val statusFile = File(prefixDir, "var/lib/dpkg/status")

    fun inspect(profile: RuntimeProfile): RuntimeProfileReport {
        val packages = installedPackages()
        val missingPackages = profile.packages.filterNot(packages::contains)
        val missingCommands = profile.requirements
            .filterNot { commandFile(it.command).isFile }
            .map { it.command }
        val incompatibleCommands = profile.requirements.mapNotNull { requirement ->
            val version = packages[requirement.packageName] ?: return@mapNotNull null
            val maximum = requirement.maxVersionExclusive ?: return@mapNotNull null
            if (compareVersions(version, maximum) >= 0) requirement.command else null
        }
        return RuntimeProfileReport(profile, missingPackages, missingCommands, incompatibleCommands)
    }

    private fun commandFile(command: String): File = File(prefixDir, "bin/$command")

    private fun installedPackages(): Map<String, String> {
        if (!statusFile.isFile) return emptyMap()
        val result = mutableMapOf<String, String>()
        statusFile.readText().split("\n\n").forEach { stanza ->
            val fields = stanza.lines().associate { line ->
                val separator = line.indexOf(':')
                if (separator <= 0) "" to "" else line.substring(0, separator) to line.substring(separator + 1).trim()
            }
            if (fields["Status"] == "install ok installed") {
                fields["Package"]?.let { name -> fields["Version"]?.let { version -> result[name] = version } }
            }
        }
        return result
    }

    private fun compareVersions(left: String, right: String): Int {
        val leftParts = numericVersionParts(left)
        val rightParts = numericVersionParts(right)
        for (index in 0 until maxOf(leftParts.size, rightParts.size)) {
            val leftPart = leftParts.getOrElse(index) { 0 }
            val rightPart = rightParts.getOrElse(index) { 0 }
            if (leftPart != rightPart) return leftPart.compareTo(rightPart)
        }
        return 0
    }

    private fun numericVersionParts(version: String): List<Int> =
        version.takeWhile { it.isDigit() || it == '.' }
            .split('.')
            .filter { it.isNotEmpty() }
            .map { it.toIntOrNull() ?: 0 }
}
