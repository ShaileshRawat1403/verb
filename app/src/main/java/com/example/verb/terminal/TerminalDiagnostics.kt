package com.example.verb.terminal

import java.io.File

data class ShellDiagnosticsReport(
    val isAccessible: Boolean,
    val binaryCount: Int,
    val sampleBinaries: List<String>,
    val rawOutput: String,
    val executionTimeMs: Long,
    val errorDetails: String? = null
)

/**
 * Read-only diagnostic probes. These deliberately use a throwaway ProcessBuilder invocation to
 * inspect reachability of the Android system shell; they never run user commands and never touch
 * the live PTY session.
 */
object TerminalDiagnostics {

    fun executeShellVerification(workingDir: File? = null): ShellDiagnosticsReport {
        val startTime = System.currentTimeMillis()
        TerminalSessionLogger.info(LogCategory.SHELL, "Executing TerminalDiagnostics shell reachability test (/system/bin/sh -c 'ls /system/bin')")

        return try {
            val process = ProcessBuilder("/system/bin/sh", "-c", "ls /system/bin")
                .apply {
                    if (workingDir != null && workingDir.exists()) {
                        directory(workingDir)
                    }
                }
                .redirectErrorStream(true)
                .start()

            val output = process.inputStream.bufferedReader().readText()
            val exitCode = process.waitFor()
            val duration = System.currentTimeMillis() - startTime

            if (exitCode == 0 && output.isNotBlank()) {
                val binaries = output.split("\\s+".toRegex()).filter { it.isNotBlank() }
                val sample = binaries.take(12)

                TerminalSessionLogger.info(
                    LogCategory.SHELL,
                    "Shell reachability verified: ${binaries.size} binaries located in /system/bin in ${duration}ms"
                )

                ShellDiagnosticsReport(
                    isAccessible = true,
                    binaryCount = binaries.size,
                    sampleBinaries = sample,
                    rawOutput = output.take(2000),
                    executionTimeMs = duration
                )
            } else {
                val err = "Process exited with code $exitCode. Output: ${output.take(500)}"
                TerminalSessionLogger.warn(LogCategory.SHELL, "Shell verification returned non-zero code or empty output: $err")

                // Fallback check: verify /system/bin directory listing directly via File API
                val systemBinDir = File("/system/bin")
                if (systemBinDir.exists() && systemBinDir.isDirectory) {
                    val files = systemBinDir.list()?.filter { it.isNotBlank() } ?: emptyList()
                    ShellDiagnosticsReport(
                        isAccessible = true,
                        binaryCount = files.size,
                        sampleBinaries = files.take(12),
                        rawOutput = files.joinToString("  ").take(2000),
                        executionTimeMs = duration,
                        errorDetails = "Direct filesystem listing fallback used ($err)"
                    )
                } else {
                    ShellDiagnosticsReport(
                        isAccessible = false,
                        binaryCount = 0,
                        sampleBinaries = emptyList(),
                        rawOutput = output,
                        executionTimeMs = duration,
                        errorDetails = err
                    )
                }
            }
        } catch (e: Exception) {
            val duration = System.currentTimeMillis() - startTime
            val errMsg = e.message ?: "Failed to execute /system/bin/sh"
            TerminalSessionLogger.error(LogCategory.SHELL, "TerminalDiagnostics execution exception: $errMsg")

            // Fallback check
            val systemBinDir = File("/system/bin")
            if (systemBinDir.exists() && systemBinDir.isDirectory) {
                val files = systemBinDir.list()?.filter { it.isNotBlank() } ?: emptyList()
                ShellDiagnosticsReport(
                    isAccessible = true,
                    binaryCount = files.size,
                    sampleBinaries = files.take(12),
                    rawOutput = files.joinToString("  ").take(2000),
                    executionTimeMs = duration,
                    errorDetails = "FileSystem check fallback ($errMsg)"
                )
            } else {
                ShellDiagnosticsReport(
                    isAccessible = false,
                    binaryCount = 0,
                    sampleBinaries = emptyList(),
                    rawOutput = "",
                    executionTimeMs = duration,
                    errorDetails = errMsg
                )
            }
        }
    }
}
