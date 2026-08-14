package com.example.verb.terminal

import java.io.File

data class ShellExecutableDiagnostic(
    val executablePath: String,
    val exists: Boolean,
    val canRead: Boolean,
    val canExecute: Boolean,
    val isFile: Boolean,
    val errorMessage: String? = null
) {
    val isAccessible: Boolean = exists && isFile && canExecute
}

data class ShellAccessibilityResult(
    val isAccessible: Boolean,
    val executablePath: String,
    val permissionError: String? = null,
    val rawOutput: String = ""
)

object ShellAccessibilityCheck {
    fun checkShellAccessibility(targetPath: String = "/system/bin/sh"): ShellAccessibilityResult {
        val file = File(targetPath)
        if (!file.exists()) {
            return ShellAccessibilityResult(
                isAccessible = false,
                executablePath = targetPath,
                permissionError = "Permission Error: Shell binary does not exist at path '$targetPath'."
            )
        }
        if (!file.canExecute()) {
            return ShellAccessibilityResult(
                isAccessible = false,
                executablePath = targetPath,
                permissionError = "Permission Error: Shell binary at '$targetPath' lacks execution permission (Access Denied)."
            )
        }

        return try {
            val process = ProcessBuilder("/system/bin/sh", "-c", "which sh || echo $targetPath")
                .redirectErrorStream(true)
                .start()
            val output = process.inputStream.bufferedReader().readText().trim()
            val exitCode = process.waitFor()

            if (exitCode == 0 && output.isNotBlank()) {
                ShellAccessibilityResult(
                    isAccessible = true,
                    executablePath = output,
                    permissionError = null,
                    rawOutput = output
                )
            } else {
                ShellAccessibilityResult(
                    isAccessible = false,
                    executablePath = targetPath,
                    permissionError = "Permission Error: Unable to execute shell binary at $targetPath (exit code $exitCode)",
                    rawOutput = output
                )
            }
        } catch (e: Exception) {
            val err = e.message ?: "Permission Denied accessing $targetPath"
            ShellAccessibilityResult(
                isAccessible = false,
                executablePath = targetPath,
                permissionError = "Permission Error: $err",
                rawOutput = ""
            )
        }
    }
}

object ShellDiagnosticUtil {
    fun diagnoseShellExecutable(executablePath: String): ShellExecutableDiagnostic {
        val file = File(executablePath)
        if (!file.exists()) {
            return ShellExecutableDiagnostic(
                executablePath = executablePath,
                exists = false,
                canRead = false,
                canExecute = false,
                isFile = false,
                errorMessage = "Shell binary does not exist at path '$executablePath'."
            )
        }
        if (!file.isFile) {
            return ShellExecutableDiagnostic(
                executablePath = executablePath,
                exists = true,
                canRead = file.canRead(),
                canExecute = file.canExecute(),
                isFile = false,
                errorMessage = "Target shell path '$executablePath' exists but is not a regular file."
            )
        }
        if (!file.canExecute()) {
            return ShellExecutableDiagnostic(
                executablePath = executablePath,
                exists = true,
                canRead = file.canRead(),
                canExecute = false,
                isFile = true,
                errorMessage = "Shell binary at '$executablePath' lacks execution permission (canExecute=false)."
            )
        }
        if (!file.canRead()) {
            return ShellExecutableDiagnostic(
                executablePath = executablePath,
                exists = true,
                canRead = false,
                canExecute = true,
                isFile = true,
                errorMessage = "Shell binary at '$executablePath' lacks read permission (canRead=false)."
            )
        }
        return ShellExecutableDiagnostic(
            executablePath = executablePath,
            exists = true,
            canRead = true,
            canExecute = true,
            isFile = true,
            errorMessage = null
        )
    }
}
