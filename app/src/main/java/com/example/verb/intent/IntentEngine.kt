package com.example.verb.intent

import com.example.verb.model.ActionRisk
import com.example.verb.model.VerbIntent

class IntentEngine {

    /**
     * Resolves user natural language query into a structured VerbIntent.
     */
    fun resolveIntent(query: String): VerbIntent {
        val normalized = query.trim().lowercase()

        // Explicit explanation requests must be resolved before any action keyword. A user asking
        // what a command does must never be routed to the corresponding action.
        explanationCommand(normalized)?.let { command ->
            return VerbIntent(
                id = "terminal.explain",
                name = "Explain Command",
                parameters = mapOf("command" to command),
                risk = ActionRisk.READ_ONLY,
                confidence = 0.85f,
                description = "Explain syntax and behavior of '$command' without executing it"
            )
        }

        // 1. Storage summary
        if (normalized.contains("storage") || normalized.contains("disk space") || normalized.contains("df")) {
            return VerbIntent(
                id = "storage.summary",
                name = "Storage Summary",
                risk = ActionRisk.READ_ONLY,
                confidence = 0.95f,
                description = "Analyse local storage usage and capacity"
            )
        }

        // 2. Memory summary
        if (normalized.contains("memory") || normalized.contains("ram") || normalized.contains("free")) {
            return VerbIntent(
                id = "memory.summary",
                name = "Memory Summary",
                risk = ActionRisk.READ_ONLY,
                confidence = 0.95f,
                description = "Check system RAM usage and availability"
            )
        }

        // 3. Process stop (Controlled write - checked before process list)
        if (normalized.contains("stop process") || normalized.contains("kill process") || normalized.contains("kill ")) {
            val pidList = extractNumbers(normalized)
            if (pidList.size != 1 || pidList.first().toIntOrNull() == null || pidList.first().toInt() <= 0) {
                return VerbIntent(
                    id = "unsupported.intent",
                    name = "Insufficient Information",
                    parameters = mapOf("raw" to query),
                    risk = ActionRisk.READ_ONLY,
                    confidence = 0.80f,
                    description = "A single valid positive PID is required to stop a process."
                )
            }
            val pid = pidList.first()
            return VerbIntent(
                id = "process.stop",
                name = "Stop Process",
                parameters = mapOf("pid" to pid),
                risk = ActionRisk.CONTROLLED_WRITE,
                confidence = 0.90f,
                description = "Stop process with PID $pid (Requires user confirmation)"
            )
        }

        // 4. Process list
        if (normalized.contains("process") || normalized.contains("running app") || normalized.contains("ps")) {
            return VerbIntent(
                id = "process.list",
                name = "Process List",
                risk = ActionRisk.READ_ONLY,
                confidence = 0.90f,
                description = "List running processes visible to the runtime"
            )
        }

        // 4. File list
        if (normalized.contains("show file") || normalized.contains("list file") || normalized.contains("files in") || normalized == "ls" || normalized.contains("directory")) {
            val path = extractPath(query) ?: "."
            return VerbIntent(
                id = "file.list",
                name = "List Files",
                parameters = mapOf("path" to path),
                risk = ActionRisk.READ_ONLY,
                confidence = 0.90f,
                description = "List files in directory $path"
            )
        }

        // 5. File search
        if (normalized.contains("find file") || normalized.contains("search file")) {
            val term = normalized.substringAfter("file").trim().removePrefix("s").trim()
            return VerbIntent(
                id = "file.search",
                name = "Search Files",
                parameters = mapOf("query" to term),
                risk = ActionRisk.READ_ONLY,
                confidence = 0.85f,
                description = "Search accessible files by filename matching $term"
            )
        }

        // 6. Port inspection
        val portRegex = Regex("""port\s*(\d+)""")
        val portMatch = portRegex.find(normalized)
        if (portMatch != null || normalized.contains("using port") || normalized.contains("port conflict")) {
            val portStr = portMatch?.groupValues?.get(1) ?: extractNumbers(normalized).firstOrNull()
            if (portStr == null || portStr.toIntOrNull() == null || portStr.toInt() !in 1..65535) {
                return VerbIntent(
                    id = "unsupported.intent",
                    name = "Insufficient Information",
                    parameters = mapOf("raw" to query),
                    risk = ActionRisk.READ_ONLY,
                    confidence = 0.80f,
                    description = "A valid port number (1-65535) is required to inspect a port."
                )
            }
            return VerbIntent(
                id = "network.port.inspect",
                name = "Inspect Port",
                parameters = mapOf("port" to portStr),
                risk = ActionRisk.READ_ONLY,
                confidence = 0.90f,
                description = "Inspect process or socket using port $portStr"
            )
        }

        // 8. System summary / info
        if (normalized.contains("system") || normalized.contains("device") || normalized.contains("info")) {
            return VerbIntent(
                id = "system.summary",
                name = "System Summary",
                risk = ActionRisk.READ_ONLY,
                confidence = 0.90f,
                description = "Overview of Android system & runtime"
            )
        }

        // 9. Open terminal
        if (normalized.contains("terminal") || normalized.contains("shell")) {
            return VerbIntent(
                id = "terminal.open",
                name = "Open Terminal",
                risk = ActionRisk.READ_ONLY,
                confidence = 0.95f,
                description = "Switch to raw interactive terminal interface"
            )
        }

        // Fallback: Unknown or unsupported intent
        return VerbIntent(
            id = "unsupported.intent",
            name = "Unsupported Intent",
            parameters = mapOf("raw" to query),
            risk = ActionRisk.READ_ONLY,
            confidence = 0.20f,
            description = "Verb could not map '$query' to a supported V0 action."
        )
    }

    private fun extractNumbers(text: String): List<String> {
        return Regex("""\d+""").findAll(text).map { it.value }.toList()
    }

    private fun explanationCommand(normalized: String): String? {
        val command = when {
            normalized.startsWith("explain") -> normalized.removePrefix("explain").trim()
            normalized.contains("explain ") -> normalized.substringAfter("explain ").trim()
            normalized.startsWith("what does ") -> normalized
                .removePrefix("what does ")
                .removeSuffix("?")
                .removeSuffix(" do")
                .trim()
            else -> return null
        }

        return command.takeIf { it.isNotBlank() }
    }

    private fun extractPath(text: String): String? {
        val match = Regex("""(/[\w\-./]+)""").find(text)
        return match?.value
    }
}
