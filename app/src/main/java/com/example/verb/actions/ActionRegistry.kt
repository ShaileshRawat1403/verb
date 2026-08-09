package com.example.verb.actions

import android.app.ActivityManager
import android.content.Context
import android.os.Build
import android.os.Environment
import android.os.Process
import android.os.StatFs
import com.example.verb.model.ActionResult
import com.example.verb.model.ActionRisk
import com.example.verb.model.VerbIntent
import java.io.File

class ActionRegistry(
    private val context: Context,
    private val processStopper: (Int) -> Unit = { pid -> Process.killProcess(pid) },
    private val currentProcessId: () -> Int = { Process.myPid() }
) {

    private val supportedIntents = setOf(
        "storage.summary",
        "memory.summary",
        "process.list",
        "file.list",
        "file.search",
        "network.port.inspect",
        "process.stop",
        "system.summary",
        "terminal.explain",
        "terminal.open"
    )

    private val actionRiskPolicy = mapOf(
        "storage.summary" to ActionRisk.READ_ONLY,
        "memory.summary" to ActionRisk.READ_ONLY,
        "process.list" to ActionRisk.READ_ONLY,
        "file.list" to ActionRisk.READ_ONLY,
        "file.search" to ActionRisk.READ_ONLY,
        "network.port.inspect" to ActionRisk.READ_ONLY,
        "process.stop" to ActionRisk.CONTROLLED_WRITE,
        "system.summary" to ActionRisk.READ_ONLY,
        "terminal.explain" to ActionRisk.READ_ONLY,
        "terminal.open" to ActionRisk.READ_ONLY
    )

    fun isActionSupported(intentId: String): Boolean = supportedIntents.contains(intentId)

    /**
     * Executes the requested intent if policy and parameters permit.
     */
    fun executeAction(intent: VerbIntent, confirmed: Boolean = false): ActionResult {
        if (!isActionSupported(intent.id)) {
            return ActionResult(
                intentId = intent.id,
                title = "Capability Not Supported",
                summary = "Verb V0.1 does not support the requested capability '${intent.id}'.",
                isSuccess = false,
                errorMessage = "Action not registered in V0 Action Registry.",
                originalIntent = intent
            )
        }

        val authoritativeRisk = actionRiskPolicy[intent.id] ?: intent.risk
        val enforcedIntent = intent.copy(risk = authoritativeRisk)

        if (enforcedIntent.id == "process.stop") {
            val pidStr = enforcedIntent.parameters["pid"] ?: ""
            val pid = pidStr.toIntOrNull()
            if (pid == currentProcessId()) {
                return ActionResult(
                    intentId = "process.stop",
                    title = "Process Stop Blocked",
                    summary = "Cannot kill Verb's own process (PID $pid).",
                    isSuccess = false,
                    errorMessage = "Self-termination blocked.",
                    originalIntent = enforcedIntent
                )
            }
            if (pid == null || pid <= 0) {
                return ActionResult(
                    intentId = "process.stop",
                    title = "Process Stop Failed",
                    summary = "Invalid PID specified: '$pidStr'. Must be a positive integer.",
                    isSuccess = false,
                    errorMessage = "PID must be a valid positive integer.",
                    originalIntent = enforcedIntent
                )
            }
        }

        // Check Risk & Confirmation Policy
        if (enforcedIntent.risk == ActionRisk.CONTROLLED_WRITE && !confirmed) {
            return ActionResult(
                intentId = enforcedIntent.id,
                title = "Confirmation Required: ${enforcedIntent.name}",
                summary = "This action modifies device runtime state. Explicit confirmation required.",
                requiresConfirmation = true,
                confirmationPrompt = "Are you sure you want to execute '${enforcedIntent.name}' for parameter ${enforcedIntent.parameters}?",
                targetPid = enforcedIntent.parameters["pid"]?.toIntOrNull(),
                isSuccess = false,
                originalIntent = enforcedIntent
            )
        }

        if (enforcedIntent.risk == ActionRisk.DESTRUCTIVE) {
            return ActionResult(
                intentId = enforcedIntent.id,
                title = "Destructive Action Blocked",
                summary = "Verb V0.1 does not execute destructive filesystem operations automatically.",
                isSuccess = false,
                errorMessage = "Action blocked by V0 Safety Policy.",
                originalIntent = enforcedIntent
            )
        }

        val result = when (enforcedIntent.id) {
            "storage.summary" -> executeStorageSummary()
            "memory.summary" -> executeMemorySummary()
            "process.list" -> executeProcessList()
            "file.list" -> executeFileList(intent.parameters["path"] ?: ".")
            "file.search" -> executeFileSearch(intent.parameters["query"] ?: "")
            "network.port.inspect" -> executePortInspect(intent.parameters["port"] ?: "")
            "process.stop" -> executeProcessStop(intent.parameters["pid"] ?: "")
            "system.summary" -> executeSystemSummary()
            "terminal.explain" -> executeTerminalExplain(intent.parameters["command"] ?: "")
            "terminal.open" -> ActionResult(
                intentId = "terminal.open",
                title = "Opening Terminal",
                summary = "Switching to raw interactive terminal."
            )
            else -> ActionResult(
                intentId = enforcedIntent.id,
                title = "Error",
                summary = "Unhandled intent",
                isSuccess = false
            )
        }

        return result.copy(originalIntent = enforcedIntent)
    }

    private fun executeStorageSummary(): ActionResult {
        return try {
            val path = Environment.getDataDirectory()
            val stat = StatFs(path.path)
            val blockSize = stat.blockSizeLong
            val totalBlocks = stat.blockCountLong
            val availableBlocks = stat.availableBlocksLong

            val totalBytes = totalBlocks * blockSize
            val availableBytes = availableBlocks * blockSize
            val usedBytes = totalBytes - availableBytes

            val totalGb = String.format("%.1f GB", totalBytes / (1024.0 * 1024.0 * 1024.0))
            val usedGb = String.format("%.1f GB", usedBytes / (1024.0 * 1024.0 * 1024.0))
            val availableGb = String.format("%.1f GB", availableBytes / (1024.0 * 1024.0 * 1024.0))

            val appDir = context.filesDir
            val termuxDirSize = getFolderSize(appDir)
            val termuxSizeMb = String.format("%.1f MB", termuxDirSize / (1024.0 * 1024.0))

            val metrics = mapOf(
                "Total Storage" to totalGb,
                "Used Storage" to usedGb,
                "Available Storage" to availableGb,
                "Verb/Termux Runtime" to termuxSizeMb
            )

            ActionResult(
                intentId = "storage.summary",
                title = "Storage Summary",
                summary = "Used $usedGb out of $totalGb ($availableGb available).",
                metrics = metrics,
                observedOutput = "totalBytes=$totalBytes, availableBytes=$availableBytes, blockSize=$blockSize",
                derivedData = metrics,
                explanation = "Storage information obtained via Android StatFs.",
                isSuccess = true
            )
        } catch (e: Exception) {
            ActionResult(
                intentId = "storage.summary",
                title = "Storage Information Unavailable",
                summary = "Unable to retrieve device storage statistics.",
                metrics = emptyMap(),
                isSuccess = false,
                errorMessage = e.localizedMessage ?: "Unknown error"
            )
        }
    }

    private fun executeMemorySummary(): ActionResult {
        val am = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        val memInfo = ActivityManager.MemoryInfo()
        am.getMemoryInfo(memInfo)

        val totalMemGb = String.format("%.2f GB", memInfo.totalMem / (1024.0 * 1024.0 * 1024.0))
        val availMemGb = String.format("%.2f GB", memInfo.availMem / (1024.0 * 1024.0 * 1024.0))
        val usedMemGb = String.format("%.2f GB", (memInfo.totalMem - memInfo.availMem) / (1024.0 * 1024.0 * 1024.0))

        val metrics = mapOf(
            "Total Memory" to totalMemGb,
            "Used Memory" to usedMemGb,
            "Available Memory" to availMemGb,
            "Low Memory State" to if (memInfo.lowMemory) "YES (Warning)" else "Normal"
        )

        return ActionResult(
            intentId = "memory.summary",
            title = "Memory Summary",
            summary = "Used $usedMemGb out of $totalMemGb ($availMemGb available).",
            metrics = metrics,
            observedOutput = "totalMem=${memInfo.totalMem}, availMem=${memInfo.availMem}, lowMemory=${memInfo.lowMemory}",
            derivedData = metrics,
            explanation = "Memory information obtained via Android ActivityManager."
        )
    }

    private fun executeProcessList(): ActionResult {
        val runningProcesses = mutableListOf<String>()
        val appProcesses = runCatching {
            val am = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
            am.runningAppProcesses ?: emptyList()
        }.getOrDefault(emptyList())

        val count = appProcesses.size.coerceAtLeast(1)
        val sampleList = if (appProcesses.isNotEmpty()) {
            appProcesses.take(8).joinToString("\n") { "PID ${it.pid} - ${it.processName}" }
        } else {
            "PID ${Process.myPid()} - ${context.packageName}\n(Android did not expose a broader process list to this application.)"
        }

        val metrics = mapOf(
            "Visible Processes" to count.toString(),
            "Current App PID" to Process.myPid().toString(),
            "Runtime UID" to Process.myUid().toString()
        )

        return ActionResult(
            intentId = "process.list",
            title = "Running Processes",
            summary = "Found $count active processes visible to Verb runtime.",
            metrics = metrics,
            observedOutput = sampleList,
            derivedData = metrics,
            explanation = "Process list obtained via Android ActivityManager."
        )
    }

    private fun executeFileList(pathParam: String): ActionResult {
        val targetDir = if (pathParam == "." || pathParam.isEmpty()) context.filesDir else File(pathParam)

        if (!targetDir.exists()) {
            return ActionResult(
                intentId = "file.list",
                title = "File Listing Failed",
                summary = "Path does not exist: ${targetDir.absolutePath}",
                isSuccess = false,
                errorMessage = "Directory or file not found."
            )
        }

        if (!targetDir.isDirectory) {
            return ActionResult(
                intentId = "file.list",
                title = "File Listing Failed",
                summary = "Path is not a directory: ${targetDir.absolutePath}",
                isSuccess = false,
                errorMessage = "Target path is a file, not a directory."
            )
        }

        val files = targetDir.listFiles()
        if (files == null) {
            return ActionResult(
                intentId = "file.list",
                title = "File Listing Failed",
                summary = "Unable to read contents of: ${targetDir.absolutePath}",
                isSuccess = false,
                errorMessage = "Permission denied or I/O failure."
            )
        }

        val metrics = mapOf(
            "Directory" to targetDir.absolutePath,
            "Item Count" to files.size.toString()
        )

        val fileDetails = files.take(15).joinToString("\n") {
            val type = if (it.isDirectory) "[DIR]" else "[FILE]"
            "$type ${it.name} (${it.length()} bytes)"
        }.ifEmpty { "(Directory is empty)" }

        return ActionResult(
            intentId = "file.list",
            title = "Files in Directory",
            summary = "Found ${files.size} items in ${targetDir.name.ifEmpty { "root" }}.",
            metrics = metrics,
            observedOutput = fileDetails,
            derivedData = metrics,
            explanation = "File listing read directly from filesystem."
        )
    }

    private fun executeFileSearch(query: String): ActionResult {
        return try {
            val targetDir = context.filesDir
            val matchedFiles = targetDir.walkTopDown()
                .filter { it.name.contains(query, ignoreCase = true) }
                .take(10)
                .toList()

            val metrics = mapOf(
                "Search Query" to query,
                "Matches Found" to matchedFiles.size.toString()
            )

            val results = matchedFiles.joinToString("\n") { it.absolutePath }
                .ifEmpty { "No files matching '$query' found in app storage." }

            ActionResult(
                intentId = "file.search",
                title = "File Search Results",
                summary = "Search for '$query' returned ${matchedFiles.size} matches.",
                metrics = metrics,
                observedOutput = results,
                derivedData = metrics,
                explanation = "File search executed directly on filesystem."
            )
        } catch (e: Exception) {
            ActionResult(
                intentId = "file.search",
                title = "File Search Failed",
                summary = "Failed to search for '$query'.",
                isSuccess = false,
                errorMessage = e.localizedMessage ?: "Unknown I/O error."
            )
        }
    }

    private fun executePortInspect(portStr: String): ActionResult {
        val port = portStr.toIntOrNull()
        if (port == null || port !in 1..65535) {
            return ActionResult(
                intentId = "network.port.inspect",
                title = "Port Inspection Failed",
                summary = "Invalid port specified: '$portStr'. Must be between 1 and 65535.",
                isSuccess = false,
                errorMessage = "Port must be a valid integer between 1 and 65535."
            )
        }

        val isOccupied = checkPortOccupied(port)

        val metrics = mapOf(
            "Target Port" to port.toString(),
            "Port Status" to if (isOccupied) "OCCUPIED / RESTRICTED" else "AVAILABLE",
            "Transport Protocol" to "TCP"
        )

        val summaryStr = if (isOccupied) {
            "Port $port is unavailable for this bind attempt."
        } else {
            "Port $port is available for this bind attempt."
        }

        val observedStr = if (isOccupied) {
            "Socket bind check: java.net.BindException (Port $port bound or restricted)"
        } else {
            "Socket bind check: Successfully bound and unbound local port $port"
        }

        val explanationStr = if (isOccupied) {
            "Socket bind check returned a conflict for port $port. Direct OS process identification is restricted by Android sandbox policies."
        } else {
            "Socket bind check confirmed port $port is available."
        }

        return ActionResult(
            intentId = "network.port.inspect",
            title = "Port $port Inspection",
            summary = summaryStr,
            metrics = metrics,
            observedOutput = observedStr,
            derivedData = metrics,
            explanation = explanationStr
        )
    }

    private fun executeProcessStop(pidStr: String): ActionResult {
        val pid = pidStr.toIntOrNull()
        if (pid == null || pid <= 0) {
            return ActionResult(
                intentId = "process.stop",
                title = "Process Stop Failed",
                summary = "Invalid PID specified: '$pidStr'. Must be a positive integer.",
                isSuccess = false,
                errorMessage = "PID must be a valid positive integer."
            )
        }

        if (pid == currentProcessId()) {
            return ActionResult(
                intentId = "process.stop",
                title = "Process Stop Blocked",
                summary = "Cannot kill Verb's own process (PID $pid).",
                isSuccess = false,
                errorMessage = "Self-termination blocked."
            )
        }

        return try {
            processStopper(pid)
            ActionResult(
                intentId = "process.stop",
                title = "Process Stop Attempted",
                summary = "Signal requested for PID $pid. Outcome unverified.",
                metrics = mapOf("Target PID" to pid.toString(), "Status" to "Signal Requested"),
                observedOutput = "Process.killProcess($pid) executed without exceptions.",
                explanation = "Attempted to terminate process via Android API. The system does not guarantee immediate termination.",
                isSuccess = false,
                errorMessage = "The signal request returned, but Verb cannot observe whether the target exited."
            )
        } catch (e: Exception) {
            ActionResult(
                intentId = "process.stop",
                title = "Process Stop Failed",
                summary = "Unable to verify process stop for PID $pid.",
                metrics = mapOf("Target PID" to pid.toString(), "Status" to "Failed"),
                isSuccess = false,
                errorMessage = e.localizedMessage ?: "Unknown error during process kill."
            )
        }
    }

    private fun executeSystemSummary(): ActionResult {
        val metrics = mapOf(
            "Device Model" to "${Build.MANUFACTURER} ${Build.MODEL}",
            "Android Version" to "${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})",
            "Architecture" to Build.SUPPORTED_ABIS.firstOrNull().orEmpty(),
            "Hardware" to Build.HARDWARE,
            "Verb Runtime" to "V0.1 (Android-Native)"
        )

        return ActionResult(
            intentId = "system.summary",
            title = "System Summary",
            summary = "Running on ${Build.MANUFACTURER} ${Build.MODEL} (Android ${Build.VERSION.RELEASE}).",
            metrics = metrics,
            observedOutput = "Manufacturer=${Build.MANUFACTURER}, Model=${Build.MODEL}, API=${Build.VERSION.SDK_INT}",
            derivedData = metrics,
            explanation = "System properties obtained via Android Build API."
        )
    }

    private fun executeTerminalExplain(command: String): ActionResult {
        val explanation = when {
            command.contains("git status") -> "Shows modified, staged, and untracked files in the active Git workspace."
            command.contains("rm -rf") -> "WARNING: Recursively deletes specified directory and all nested files without confirmation."
            command.contains("df") -> "Displays total, used, and available filesystem disk space."
            command.contains("free") -> "Displays total, used, and available RAM memory metrics."
            command.contains("ps") -> "Lists active operating processes and their process IDs (PIDs)."
            else -> "Command '$command' executes shell operation in the current runtime working directory."
        }

        return ActionResult(
            intentId = "terminal.explain",
            title = "Command Explanation",
            summary = explanation,
            metrics = mapOf("Command" to command, "Risk Class" to if (command.contains("rm")) "DESTRUCTIVE" else "READ_ONLY"),
            explanation = explanation
        )
    }

    private fun checkPortOccupied(port: Int): Boolean {
        // Port check simulated or socket binding test
        return try {
            val socket = java.net.ServerSocket(port)
            socket.close()
            false
        } catch (e: Exception) {
            true // Port occupied or restricted
        }
    }

    private fun getFolderSize(file: File): Long {
        var size: Long = 0
        val files = file.listFiles() ?: return 0
        for (f in files) {
            size += if (f.isDirectory) getFolderSize(f) else f.length()
        }
        return size
    }
}
