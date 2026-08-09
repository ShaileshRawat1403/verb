package com.example.verb.semantic

import com.example.verb.model.*

class SemanticEngine {

    fun analyzeText(selectedText: String, surroundingContext: String? = null): SemanticEntity {
        val text = selectedText.trim()

        // 0. Sensitive Text Guard
        val sensitiveEntity = SecretGuard.checkSensitive(text)
        if (sensitiveEntity != null) {
            return sensitiveEntity
        }

        // 1. Error messages
        if (text.contains("EADDRINUSE", ignoreCase = true) || text.contains("address already in use", ignoreCase = true)) {
            val portMatch = Regex(""":(\d{1,5})""").find(text)
            val port = portMatch?.groupValues?.get(1)?.toIntOrNull()
            
            var detectedPort = port
            if (port != null && port !in 1..65535) detectedPort = null

            return SemanticEntity(
                rawText = text,
                entityType = EntityType.PORT_CONFLICT,
                title = "Port Conflict Detected",
                description = if (detectedPort != null) "Address/port conflict detected on port $detectedPort." else "Address/port conflict detected. The selected text does not identify the port.",
                risk = ActionRisk.READ_ONLY,
                detectedPort = detectedPort,
                confidence = DetectionConfidence.HIGH,
                detectionMethod = "ERROR_EADDRINUSE",
                normalizedValue = detectedPort?.toString(),
                suggestedActions = if (detectedPort != null) listOf(
                    SuggestedAction(
                        id = "inspect_port_$detectedPort",
                        label = "Inspect Port $detectedPort",
                        intent = VerbIntent(
                            id = "network.port.inspect",
                            name = "Inspect Port",
                            parameters = mapOf("port" to detectedPort.toString()),
                            risk = ActionRisk.READ_ONLY
                        ),
                        risk = ActionRisk.READ_ONLY
                    )
                ) else emptyList()
            )
        }

        if (text.contains("Permission denied", ignoreCase = true) || 
            text.contains("Command not found", ignoreCase = true) || 
            text.contains("ENOENT", ignoreCase = true) || 
            text.contains("TypeError", ignoreCase = true)) {
            val errorSummary = when {
                text.contains("TypeError", ignoreCase = true) -> "unknown without more context"
                text.contains("Permission denied", ignoreCase = true) -> "Selected output indicates the current process lacks permissions for the resource."
                text.contains("Command not found", ignoreCase = true) -> "Often means the executable is not installed or not in PATH."
                else -> "Often means a file or directory does not exist (ENOENT)."
            }
            return SemanticEntity(
                rawText = text,
                entityType = EntityType.ERROR_MESSAGE,
                title = if (text.contains("TypeError", ignoreCase = true)) "Type-related runtime error" else "Runtime Error",
                description = if (text.contains("TypeError", ignoreCase = true)) "Cause: unknown without more context" else errorSummary,
                confidence = DetectionConfidence.HIGH,
                detectionMethod = "ERROR_MESSAGE"
            )
        }

        // 2. URLs
        val inlineUrlMatch = Regex("""(https?://[^\s]+)""").find(text)
        if (inlineUrlMatch != null && text.trim() == inlineUrlMatch.groupValues[1]) {
            val url = inlineUrlMatch.groupValues[1]
            return SemanticEntity(
                rawText = text,
                entityType = EntityType.URL,
                title = "URL Link",
                description = "Web address: $url",
                confidence = DetectionConfidence.EXACT,
                detectionMethod = "EXACT_URL",
                normalizedValue = url
            )
        }

        // 3. IP Addresses
        val ipMatch = Regex("""\b(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})\b""").find(text)
        if (ipMatch != null && text.trim() == ipMatch.value) {
            val valid = (1..4).all { ipMatch.groupValues[it].toIntOrNull() in 0..255 }
            if (valid) {
                return SemanticEntity(
                    rawText = text,
                    entityType = EntityType.IP_ADDRESS,
                    title = "IP Address",
                    description = "IPv4 network address: ${ipMatch.value}",
                    confidence = DetectionConfidence.EXACT,
                    detectionMethod = "EXACT_IPV4",
                    normalizedValue = ipMatch.value
                )
            }
        }

        // 4. Standalone Ports
        val portRegex = Regex("""^(?:port\s+|:)(\d{1,5})$""", RegexOption.IGNORE_CASE)
        val pMatch = portRegex.find(text)
        if (pMatch != null) {
            val p = pMatch.groupValues[1].toIntOrNull()
            if (p != null && p in 1..65535) {
                return SemanticEntity(
                    rawText = text,
                    entityType = EntityType.PORT,
                    title = "Network Port $p",
                    description = "TCP/UDP communication port $p.",
                    detectedPort = p,
                    confidence = DetectionConfidence.EXACT,
                    detectionMethod = "EXACT_PORT",
                    normalizedValue = p.toString(),
                    suggestedActions = listOf(
                        SuggestedAction(
                            id = "inspect_port_$p",
                            label = "Inspect Port $p",
                            intent = VerbIntent(
                                id = "network.port.inspect",
                                name = "Inspect Port",
                                parameters = mapOf("port" to p.toString()),
                                risk = ActionRisk.READ_ONLY
                            ),
                            risk = ActionRisk.READ_ONLY
                        )
                    )
                )
            }
        }

        // 5. PID
        val pidRegex = Regex("""^(?:PID|pid)\s*[:=]?\s*(\d+)$""")
        val pidMatch = pidRegex.find(text)
        if (pidMatch != null) {
            val pid = pidMatch.groupValues[1].toIntOrNull()
            if (pid != null && pid > 0) {
                return SemanticEntity(
                    rawText = text,
                    entityType = EntityType.PID,
                    title = "Process ID (PID $pid)",
                    description = "System Process Identifier $pid.",
                    detectedPid = pid,
                    confidence = DetectionConfidence.EXACT,
                    detectionMethod = "PID_PATTERN",
                    normalizedValue = pid.toString()
                )
            }
        }

        // 6. Commands
        val cmdTokens = text.split(Regex("""\s+"""))
        if (cmdTokens.isNotEmpty()) {
            val baseCmd = cmdTokens[0]
            val isRm = baseCmd == "rm"
            val hasDestructiveFlag = cmdTokens.any { it == "-rf" || it == "-r" || it == "-f" }
            if (isRm && hasDestructiveFlag) {
                val breakdownList = parseCommandBreakdown(text)
                return SemanticEntity(
                    rawText = text,
                    entityType = EntityType.DESTRUCTIVE_COMMAND,
                    title = "Destructive Delete Command",
                    description = "Permanently deletes target without prompt or trash backup.",
                    risk = ActionRisk.DESTRUCTIVE,
                    warningMessage = "HIGH RISK: Destructive file deletion operation.",
                    breakdown = breakdownList,
                    confidence = DetectionConfidence.HIGH,
                    detectionMethod = "DETERMINISTIC_COMMAND"
                )
            }

            val supportedCommands = setOf("ls", "cd", "pwd", "cat", "grep", "find", "du", "df", "ps", "free", "git", "curl", "wget", "echo", "printf")
            if (supportedCommands.contains(baseCmd)) {
                val breakdownList = parseCommandBreakdown(text)
                return SemanticEntity(
                    rawText = text,
                    entityType = EntityType.COMMAND,
                    title = "Shell Command: '${text.take(20)}'",
                    description = getCommandDescription(baseCmd),
                    breakdown = breakdownList,
                    confidence = DetectionConfidence.HIGH,
                    detectionMethod = "DETERMINISTIC_COMMAND",
                    normalizedValue = baseCmd,
                    suggestedActions = listOf(
                        SuggestedAction(
                            id = "explain_cmd",
                            label = "Explain Syntax",
                            intent = VerbIntent(
                                id = "terminal.explain",
                                name = "Explain Command",
                                parameters = mapOf("command" to text),
                                risk = ActionRisk.READ_ONLY
                            ),
                            risk = ActionRisk.READ_ONLY
                        )
                    )
                )
            }
        }

        // 7. File Path
        if (text.startsWith("/") || text.startsWith("./") || text.startsWith("../") || text.startsWith("~/")) {
            val isPath = Regex("""^[~./a-zA-Z0-9_\-]+(/[a-zA-Z0-9_\-.]+)*$""").matches(text)
            if (isPath) {
                return SemanticEntity(
                    rawText = text,
                    entityType = EntityType.FILE_PATH,
                    title = "File Path",
                    description = "Path reference: '$text'.",
                    detectedPath = text,
                    confidence = DetectionConfidence.HIGH,
                    detectionMethod = "FILE_PATH",
                    normalizedValue = text,
                    suggestedActions = listOf(
                        SuggestedAction(
                            id = "list_files_path",
                            label = "List directory",
                            intent = VerbIntent(
                                id = "file.list",
                                name = "List Files",
                                parameters = mapOf("path" to text),
                                risk = ActionRisk.READ_ONLY
                            ),
                            risk = ActionRisk.READ_ONLY
                        )
                    )
                )
            }
        }

        // 8. Generic Fallback
        return SemanticEntity(
            rawText = text,
            entityType = EntityType.GENERIC_TEXT,
            title = "Selected Terminal Content",
            description = "Selected text snippet from active session.",
            confidence = DetectionConfidence.LOW,
            detectionMethod = "FALLBACK"
        )
    }

    private fun getCommandDescription(baseCmd: String): String {
        return when (baseCmd) {
            "git" -> "Shows the current Git repository state, including staged, unstaged, and untracked files."
            "du" -> "Estimates file space usage and directory sizes."
            "df" -> "Displays available and used disk space on filesystems."
            "free" -> "Displays system memory (RAM) totals, usage, and available capacity."
            "ps" -> "Lists running process snapshots."
            "ls" -> "Lists directory contents and permissions."
            else -> "Executes '$baseCmd' in local shell environment."
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
                part.startsWith("-") -> "Option / Flag argument '${part}'"
                part.startsWith("/") || part.contains(".") -> "Target path argument '${part}'"
                else -> "Parameter / Subject '${part}'"
            }
            list.add(CommandBreakdownItem(part, meaning))
        }
        return list
    }
}
