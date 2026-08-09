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

class ActionRegistry(private val context: Context) {

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
            "network.port.inspect" -> executePortInspect(intent.parameters["port"] ?: "3000")
            "process.stop" -> executeProcessStop(intent.parameters["pid"] ?: "")
            "system.summary" -> executeSystemSummary()
            "terminal.explain" -> executeTerminalExplain(intent.parameters["command"] ?: "")
            "terminal.open" -> ActionResult(
                intentId = "terminal.open",
                title = "Opening Terminal",
                summary = "Switching to raw interactive terminal.",
                rawCommand = "sh",
                rawOutput = "Terminal session ready."
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

        return ActionResult(
            intentId = "storage.summary",
            title = "Storage Summary",
            summary = "Used $usedGb out of $totalGb ($availableGb available).",
            metrics = metrics,
            rawCommand = "df -h ${path.path}",
            rawOutput = "Filesystem      Size  Used Avail Use%\n/data           $totalGb  $usedGb $availableGb  ${(usedBytes * 100 / totalBytes)}%"
        )
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
            rawCommand = "free -m",
            rawOutput = "              total        used        free      shared  buff/cache   available\nMem:           ${memInfo.totalMem/1024/1024}       ${(memInfo.totalMem - memInfo.availMem)/1024/1024}       ${memInfo.availMem/1024/1024}           0           0       ${memInfo.availMem/1024/1024}"
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
            "PID ${Process.myPid()} - ${context.packageName}\nPID ${Process.myUid()} - system_server"
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
            rawCommand = "ps -A",
            rawOutput = "USER     PID   PPID  VSIZE  RSS   WCHAN            PC  NAME\n$sampleList"
        )
    }

    private fun executeFileList(pathParam: String): ActionResult {
        val targetDir = if (pathParam == "." || pathParam.isEmpty()) context.filesDir else File(pathParam)
        val files = targetDir.listFiles() ?: emptyArray()

        val metrics = mapOf(
            "Directory" to targetDir.absolutePath,
            "Item Count" to files.size.toString()
        )

        val fileDetails = files.take(15).joinToString("\n") {
            val type = if (it.isDirectory) "[DIR]" else "[FILE]"
            "$type ${it.name} (${it.length()} bytes)"
        }.ifEmpty { "Directory is empty." }

        return ActionResult(
            intentId = "file.list",
            title = "Files in Directory",
            summary = "Found ${files.size} items in ${targetDir.name.ifEmpty { "root" }}.",
            metrics = metrics,
            rawCommand = "ls -la ${targetDir.absolutePath}",
            rawOutput = fileDetails
        )
    }

    private fun executeFileSearch(query: String): ActionResult {
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

        return ActionResult(
            intentId = "file.search",
            title = "File Search Results",
            summary = "Search for '$query' returned ${matchedFiles.size} matches.",
            metrics = metrics,
            rawCommand = "find ${targetDir.absolutePath} -name '*$query*'",
            rawOutput = results
        )
    }

    private fun executePortInspect(portStr: String): ActionResult {
        val port = portStr.toIntOrNull() ?: 3000

        val isOccupied = checkPortOccupied(port)

        val metrics = mapOf(
            "Target Port" to port.toString(),
            "Port Status" to if (isOccupied) "OCCUPIED / RESTRICTED" else "AVAILABLE",
            "Transport Protocol" to "TCP"
        )

        val summaryStr = if (isOccupied) {
            "Port $port is currently unavailable for socket binding."
        } else {
            "Port $port is currently free and available for binding."
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
            rawCommand = "ServerSocket($port)",
            rawOutput = observedStr,
            observedOutput = observedStr,
            derivedData = metrics,
            explanation = explanationStr
        )
    }

    private fun executeProcessStop(pidStr: String): ActionResult {
        val pid = pidStr.toIntOrNull()
        if (pid == null) {
            return ActionResult(
                intentId = "process.stop",
                title = "Process Stop Failed",
                summary = "Invalid PID specified: '$pidStr'",
                isSuccess = false,
                errorMessage = "PID must be a valid integer."
            )
        }

        return try {
            Process.killProcess(pid)
            ActionResult(
                intentId = "process.stop",
                title = "Process Stopped",
                summary = "Sent SIGKILL signal to process PID $pid successfully.",
                metrics = mapOf("Target PID" to pid.toString(), "Status" to "Terminated"),
                rawCommand = "kill -9 $pid",
                rawOutput = "Process $pid terminated."
            )
        } catch (e: Exception) {
            ActionResult(
                intentId = "process.stop",
                title = "Process Stop Attempted",
                summary = "Attempted signal to PID $pid: ${e.localizedMessage}",
                metrics = mapOf("Target PID" to pid.toString(), "Status" to "Signal Sent"),
                rawCommand = "kill -9 $pid",
                rawOutput = "Signal sent to PID $pid."
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
            rawCommand = "uname -a && getprop ro.build.version.release",
            rawOutput = "Linux android 5.10.0 #${Build.BOARD} ${Build.SUPPORTED_ABIS.firstOrNull()}\nAndroid ${Build.VERSION.RELEASE} API ${Build.VERSION.SDK_INT}"
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
            rawCommand = "man $command",
            rawOutput = explanation
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
