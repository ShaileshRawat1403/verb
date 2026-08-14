package com.example.verb.terminal

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

enum class LogLevel {
    INFO,
    WARN,
    ERROR,
    DEBUG
}

enum class LogCategory {
    LIFECYCLE,
    JNI,
    SHELL,
    IO,
    DIAGNOSTIC
}

data class TerminalLogEntry(
    val timestamp: String,
    val level: LogLevel,
    val category: LogCategory,
    val message: String
)

object TerminalSessionLogger {
    private val dateFormat = SimpleDateFormat("HH:mm:ss.SSS", Locale.US)
    private val maxCapacity = 200

    private val _logs = MutableStateFlow<List<TerminalLogEntry>>(emptyList())
    val logs: StateFlow<List<TerminalLogEntry>> = _logs.asStateFlow()

    @Synchronized
    fun log(level: LogLevel, category: LogCategory, message: String) {
        val entry = TerminalLogEntry(
            timestamp = dateFormat.format(Date()),
            level = level,
            category = category,
            message = message
        )
        val current = _logs.value.toMutableList()
        if (current.size >= maxCapacity) {
            current.removeAt(0)
        }
        current.add(entry)
        _logs.value = current
    }

    fun info(category: LogCategory, message: String) = log(LogLevel.INFO, category, message)
    fun warn(category: LogCategory, message: String) = log(LogLevel.WARN, category, message)
    fun error(category: LogCategory, message: String) = log(LogLevel.ERROR, category, message)
    fun debug(category: LogCategory, message: String) = log(LogLevel.DEBUG, category, message)

    fun clear() {
        _logs.value = emptyList()
    }

    fun exportDiagnosticReport(
        sessionState: TerminalSessionState?,
        workingDir: String?,
        shellExecutable: String?
    ): String {
        val sb = StringBuilder()
        sb.appendLine("=== VERB TERMINAL DIAGNOSTIC REPORT ===")
        sb.appendLine("Timestamp: ${SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.US).format(Date())}")
        sb.appendLine("Session State: ${sessionState ?: "UNKNOWN"}")
        sb.appendLine("Working Directory: ${workingDir ?: "Unknown"}")
        sb.appendLine("Shell Executable: ${shellExecutable ?: "Unknown"}")
        sb.appendLine("Total Log Entries: ${_logs.value.size}")
        sb.appendLine("\n--- RECENT LOGS ---")
        _logs.value.forEach { entry ->
            sb.appendLine("[${entry.timestamp}] [${entry.level.name}] [${entry.category.name}] ${entry.message}")
        }
        sb.appendLine("=== END DIAGNOSTIC REPORT ===")
        return sb.toString()
    }
}
