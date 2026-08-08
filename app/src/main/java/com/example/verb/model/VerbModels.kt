package com.example.verb.model

enum class ActionRisk {
    READ_ONLY,
    CONTROLLED_WRITE,
    DESTRUCTIVE
}

enum class EntityType {
    PORT_CONFLICT,
    PORT,
    COMMAND,
    DESTRUCTIVE_COMMAND,
    ERROR_MESSAGE,
    FILE_PATH,
    PID,
    URL,
    IP_ADDRESS,
    GENERIC_TEXT
}

data class VerbIntent(
    val id: String,
    val name: String,
    val parameters: Map<String, String> = emptyMap(),
    val risk: ActionRisk = ActionRisk.READ_ONLY,
    val confidence: Float = 1.0f,
    val description: String = ""
) {
    val summary: String
        get() = description.ifEmpty { name }

    val commandTemplate: String?
        get() = when (id) {
            "storage.summary" -> "df -h"
            "memory.summary" -> "free -m"
            "process.list" -> "ps -A"
            "file.list" -> "ls -la ${parameters["path"] ?: "."}"
            "file.search" -> "find . -name '*${parameters["query"] ?: ""}*'"
            "network.port.inspect" -> "netstat -tuln | grep :${parameters["port"] ?: "3000"}"
            "process.stop" -> "kill -9 ${parameters["pid"] ?: ""}"
            "system.summary" -> "uname -a"
            "terminal.explain" -> "man ${parameters["command"] ?: ""}"
            else -> parameters["raw"] ?: "sh"
        }
}

data class ActionResult(
    val intentId: String,
    val title: String,
    val summary: String,
    val metrics: Map<String, String> = emptyMap(),
    val rawCommand: String? = null,
    val rawOutput: String? = null,
    val isSuccess: Boolean = true,
    val errorMessage: String? = null,
    val requiresConfirmation: Boolean = false,
    val confirmationPrompt: String? = null,
    val targetPid: Int? = null,
    val timestamp: Long = System.currentTimeMillis()
)

data class SuggestedAction(
    val id: String,
    val label: String,
    val intentQuery: String? = null,
    val risk: ActionRisk = ActionRisk.READ_ONLY,
    val isDangerous: Boolean = false
)

data class CommandBreakdownItem(
    val part: String,
    val meaning: String
)

data class SemanticEntity(
    val rawText: String,
    val entityType: EntityType,
    val title: String,
    val description: String,
    val breakdown: List<CommandBreakdownItem> = emptyList(),
    val risk: ActionRisk = ActionRisk.READ_ONLY,
    val warningMessage: String? = null,
    val detectedPort: Int? = null,
    val detectedPid: Int? = null,
    val detectedPath: String? = null,
    val suggestedActions: List<SuggestedAction> = emptyList()
)
