package com.example.verb.semantic

import com.example.verb.model.ActionRisk
import com.example.verb.model.CommandBreakdownItem
import com.example.verb.model.EntityType
import com.example.verb.model.SemanticEntity
import com.example.verb.model.SuggestedAction
import java.util.regex.Pattern

class SemanticEngine {

    /**
     * Analyzes selected text from terminal or Ask screen to extract semantic entity,
     * explanations, breakdown, risk assessment, and safe suggested actions.
     */
    fun analyzeText(selectedText: String, surroundingContext: String? = null): SemanticEntity {
        val text = selectedText.trim()

        // 1. Port Conflict Error (EADDRINUSE / address in use)
        if (text.contains("EADDRINUSE", ignoreCase = true) || text.contains("address already in use", ignoreCase = true)) {
            val portMatch = Regex(""":(\d+)""").find(text) ?: Regex("""port\s*(\d+)""", RegexOption.IGNORE_CASE).find(text)
            val port = portMatch?.groupValues?.get(1)?.toIntOrNull() ?: 3000

            return SemanticEntity(
                rawText = text,
                entityType = EntityType.PORT_CONFLICT,
                title = "Port Conflict Detected",
                description = "Port $port is already being used by another active listening process on this device.",
                risk = ActionRisk.READ_ONLY,
                detectedPort = port,
                suggestedActions = listOf(
                    SuggestedAction(
                        id = "inspect_port_$port",
                        label = "Find process using port $port",
                        intentQuery = "what is using port $port",
                        risk = ActionRisk.READ_ONLY
                    ),
                    SuggestedAction(
                        id = "explain_port_$port",
                        label = "Explain port conflicts",
                        intentQuery = "explain port conflict $port",
                        risk = ActionRisk.READ_ONLY
                    )
                )
            )
        }

        // 2. Destructive Command (rm -rf, dd, mkfs)
        if (text.contains("rm -rf", ignoreCase = true) || text.contains("rm -r", ignoreCase = true) || text.contains("rm -f", ignoreCase = true)) {
            val target = text.substringAfter("rm").replace("-rf", "").replace("-r", "").replace("-f", "").trim().ifEmpty { "target" }

            return SemanticEntity(
                rawText = text,
                entityType = EntityType.DESTRUCTIVE_COMMAND,
                title = "Destructive Delete Command",
                description = "Permanently deletes directory '$target' and all contents inside it without prompt or trash backup.",
                risk = ActionRisk.DESTRUCTIVE,
                warningMessage = "HIGH RISK: Destructive file deletion operation.",
                breakdown = listOf(
                    CommandBreakdownItem("rm", "Remove / delete command"),
                    CommandBreakdownItem("-r", "Recursively delete directories and their subdirectories"),
                    CommandBreakdownItem("-f", "Force deletion, ignore non-existent files without prompting"),
                    CommandBreakdownItem(target, "Target file or directory path")
                ),
                suggestedActions = listOf(
                    SuggestedAction(
                        id = "explain_rm",
                        label = "Explain rm syntax",
                        intentQuery = "explain command $text",
                        risk = ActionRisk.READ_ONLY
                    )
                )
            )
        }

        // 3. Port number standalone or socket (:3000, port 8080)
        val standalonePortMatch = Regex("""\b(?:port\s*)?([1-9]\d{2,4})\b""", RegexOption.IGNORE_CASE).find(text)
        if (standalonePortMatch != null && (text.contains("port", ignoreCase = true) || text.startsWith(":") || text.matches(Regex("""\d{4,5}""")))) {
            val port = standalonePortMatch.groupValues[1].toIntOrNull() ?: 3000
            return SemanticEntity(
                rawText = text,
                entityType = EntityType.PORT,
                title = "Network Port $port",
                description = "TCP/UDP communication port $port.",
                detectedPort = port,
                suggestedActions = listOf(
                    SuggestedAction(
                        id = "inspect_port_$port",
                        label = "Inspect Port $port",
                        intentQuery = "check port $port",
                        risk = ActionRisk.READ_ONLY
                    )
                )
            )
        }

        // 4. PID (PID 19281, pid 1234)
        val pidMatch = Regex("""\b(?:PID|pid)\s*[:=]?\s*(\d+)\b""").find(text)
        if (pidMatch != null) {
            val pid = pidMatch.groupValues[1].toIntOrNull() ?: 0
            return SemanticEntity(
                rawText = text,
                entityType = EntityType.PID,
                title = "Process ID (PID $pid)",
                description = "System Process Identifier $pid.",
                detectedPid = pid,
                suggestedActions = listOf(
                    SuggestedAction(
                        id = "stop_pid_$pid",
                        label = "Stop Process $pid",
                        intentQuery = "stop process $pid",
                        risk = ActionRisk.CONTROLLED_WRITE,
                        isDangerous = true
                    )
                )
            )
        }

        // 5. Common Shell Commands (git status, du -h, ls, ps, df, free)
        if (text.startsWith("git ") || text.startsWith("du ") || text.startsWith("ls") || text.startsWith("ps") || text.startsWith("df") || text.startsWith("free")) {
            val breakdownList = parseCommandBreakdown(text)
            return SemanticEntity(
                rawText = text,
                entityType = EntityType.COMMAND,
                title = "Shell Command: '${text.take(20)}'",
                description = getCommandDescription(text),
                breakdown = breakdownList,
                suggestedActions = listOf(
                    SuggestedAction(
                        id = "run_command",
                        label = "Run in Terminal",
                        intentQuery = "terminal",
                        risk = ActionRisk.READ_ONLY
                    ),
                    SuggestedAction(
                        id = "explain_cmd",
                        label = "Explain Syntax",
                        intentQuery = "explain command $text",
                        risk = ActionRisk.READ_ONLY
                    )
                )
            )
        }

        // 6. Common Error Messages
        if (text.contains("TypeError", ignoreCase = true) || text.contains("Permission denied", ignoreCase = true) || text.contains("Command not found", ignoreCase = true) || text.contains("ENOENT", ignoreCase = true)) {
            val errorSummary = when {
                text.contains("TypeError", ignoreCase = true) -> "Attempted to call a method or read property on an undefined or null reference."
                text.contains("Permission denied", ignoreCase = true) -> "The current process lacks file system read/write permissions for this resource."
                text.contains("Command not found", ignoreCase = true) -> "The specified shell binary or executable is not installed or not in PATH."
                else -> "Runtime execution error encountered."
            }

            return SemanticEntity(
                rawText = text,
                entityType = EntityType.ERROR_MESSAGE,
                title = "Runtime Error",
                description = errorSummary,
                suggestedActions = listOf(
                    SuggestedAction(
                        id = "explain_err",
                        label = "Explain Error Cause",
                        intentQuery = "explain $text",
                        risk = ActionRisk.READ_ONLY
                    )
                )
            )
        }

        // 7. File Path
        if (text.startsWith("/") || text.startsWith("./") || text.contains("/") || text.endsWith(".js") || text.endsWith(".kts") || text.endsWith(".json")) {
            return SemanticEntity(
                rawText = text,
                entityType = EntityType.FILE_PATH,
                title = "File Path",
                description = "Path reference: '$text'.",
                detectedPath = text,
                suggestedActions = listOf(
                    SuggestedAction(
                        id = "list_files_path",
                        label = "List files in directory",
                        intentQuery = "show files in $text",
                        risk = ActionRisk.READ_ONLY
                    )
                )
            )
        }

        // 8. Generic Terminal Output Fallback
        return SemanticEntity(
            rawText = text,
            entityType = EntityType.GENERIC_TEXT,
            title = "Selected Terminal Content",
            description = "Selected text snippet from active session.",
            suggestedActions = listOf(
                SuggestedAction(
                    id = "explain_text",
                    label = "Explain text",
                    intentQuery = "explain $text",
                    risk = ActionRisk.READ_ONLY
                )
            )
        )
    }

    private fun getCommandDescription(cmd: String): String {
        return when {
            cmd.startsWith("git status") -> "Shows the current Git repository state, including staged, unstaged, and untracked files."
            cmd.startsWith("du") -> "Estimates file space usage and directory sizes."
            cmd.startsWith("df") -> "Displays available and used disk space on filesystems."
            cmd.startsWith("free") -> "Displays system memory (RAM) totals, usage, and available capacity."
            cmd.startsWith("ps") -> "Lists running process snapshots."
            cmd.startsWith("ls") -> "Lists directory contents and permissions."
            else -> "Executes '$cmd' in local shell environment."
        }
    }

    private fun parseCommandBreakdown(cmd: String): List<CommandBreakdownItem> {
        val parts = cmd.split(Regex("""\s+"""))
        if (parts.isEmpty()) return emptyList()

        val list = mutableListOf<CommandBreakdownItem>()
        val base = parts[0]
        list.add(CommandBreakdownItem(base, "Executable command name"))

        for (i in 1 until parts.size) {
            val part = parts[i]
            val meaning = when {
                part.startsWith("-") -> "Option / Flag argument '$part'"
                part.startsWith("/") || part.contains(".") -> "Target path argument '$part'"
                else -> "Parameter / Subject '$part'"
            }
            list.add(CommandBreakdownItem(part, meaning))
        }
        return list
    }
}
