package com.example.verb.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Usb
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

data class UsbAdbDiagnosticResult(
    val isAdbDaemonRunning: Boolean,
    val isUsbDebuggingConfigured: Boolean,
    val adbDaemonServiceState: String,
    val usbConfig: String,
    val persistentUsbConfig: String,
    val adbTcpPort: String,
    val rawOutputLog: String,
    val lastCheckedTimestamp: Long = System.currentTimeMillis()
)

@Composable
fun UsbDebuggingDiagnosticCard(
    modifier: Modifier = Modifier,
    onVerifyCompleted: (UsbAdbDiagnosticResult) -> Unit = {}
) {
    var isLoading by remember { mutableStateOf(false) }
    var result by remember { mutableStateOf<UsbAdbDiagnosticResult?>(null) }
    val coroutineScope = rememberCoroutineScope()
    val timeFormat = remember { SimpleDateFormat("HH:mm:ss", Locale.US) }

    fun runAdbVerification() {
        isLoading = true
        coroutineScope.launch {
            val diagResult = withContext(Dispatchers.IO) {
                performAdbShellCheck()
            }
            result = diagResult
            isLoading = false
            onVerifyCompleted(diagResult)
        }
    }

    LaunchedEffect(Unit) {
        runAdbVerification()
    }

    Card(
        modifier = modifier
            .fillMaxWidth()
            .testTag("usb_debugging_card"),
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(
                        imageVector = Icons.Default.Usb,
                        contentDescription = "USB Connection Status",
                        tint = MaterialTheme.colorScheme.primary
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = "USB Debugging Diagnostics",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.Bold
                    )
                }

                Button(
                    onClick = { runAdbVerification() },
                    enabled = !isLoading,
                    shape = RoundedCornerShape(12.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.primaryContainer,
                        contentColor = MaterialTheme.colorScheme.onPrimaryContainer
                    ),
                    modifier = Modifier.testTag("retry_adb_button")
                ) {
                    if (isLoading) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(16.dp),
                            strokeWidth = 2.dp,
                            color = MaterialTheme.colorScheme.onPrimaryContainer
                        )
                    } else {
                        Icon(
                            imageVector = Icons.Default.Refresh,
                            contentDescription = "Retry Connection",
                            modifier = Modifier.size(16.dp)
                        )
                        Spacer(modifier = Modifier.width(4.dp))
                        Text(text = "Retry", fontSize = 12.sp, fontWeight = FontWeight.Bold)
                    }
                }
            }

            Spacer(modifier = Modifier.height(12.dp))

            val current = result
            if (current != null) {
                val statusColor = when {
                    current.isAdbDaemonRunning && current.isUsbDebuggingConfigured -> Color(0xFF4CAF50)
                    current.isAdbDaemonRunning || current.isUsbDebuggingConfigured -> Color(0xFFFF9800)
                    else -> MaterialTheme.colorScheme.error
                }

                val statusLabel = when {
                    current.isAdbDaemonRunning && current.isUsbDebuggingConfigured -> "USB Debugging Active"
                    current.isAdbDaemonRunning -> "ADB Daemon Running (USB Pending)"
                    current.isUsbDebuggingConfigured -> "USB Configured (Daemon Waiting)"
                    else -> "USB Debugging Disconnected / Disabled"
                }

                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = statusColor.copy(alpha = 0.15f),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Icon(
                            imageVector = if (current.isAdbDaemonRunning && current.isUsbDebuggingConfigured) Icons.Default.CheckCircle else Icons.Default.Warning,
                            contentDescription = null,
                            tint = statusColor,
                            modifier = Modifier.size(20.dp)
                        )
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(
                            text = statusLabel,
                            fontSize = 13.sp,
                            fontWeight = FontWeight.Bold,
                            color = statusColor,
                            modifier = Modifier.testTag("usb_status_text")
                        )
                        Spacer(modifier = Modifier.weight(1f))
                        Text(
                            text = "Checked: ${timeFormat.format(Date(current.lastCheckedTimestamp))}",
                            fontSize = 11.sp,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }

                Spacer(modifier = Modifier.height(12.dp))

                AdbDetailRow(label = "adbd Service State", value = current.adbDaemonServiceState.ifEmpty { "N/A" })
                AdbDetailRow(label = "Active USB Config", value = current.usbConfig.ifEmpty { "N/A" })
                AdbDetailRow(label = "Persistent USB Config", value = current.persistentUsbConfig.ifEmpty { "N/A" })
                if (current.adbTcpPort.isNotBlank() && current.adbTcpPort != "0") {
                    AdbDetailRow(label = "Wireless ADB Port", value = current.adbTcpPort)
                }

                Spacer(modifier = Modifier.height(10.dp))

                Text(
                    text = "ADB Communication Shell Log",
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(modifier = Modifier.height(4.dp))
                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = MaterialTheme.colorScheme.surfaceVariant,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    SelectionContainer {
                        Text(
                            text = current.rawOutputLog,
                            fontFamily = FontFamily.Monospace,
                            fontSize = 11.sp,
                            lineHeight = 15.sp,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier
                                .padding(10.dp)
                                .testTag("adb_raw_output")
                        )
                    }
                }
            } else {
                Text(
                    text = "Initializing ADB connection diagnostics...",
                    fontSize = 13.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Composable
private fun AdbDetailRow(label: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 3.dp),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(
            text = label,
            fontSize = 12.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Text(
            text = value,
            fontSize = 12.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = FontFamily.Monospace,
            color = MaterialTheme.colorScheme.onSurface
        )
    }
}

private fun performAdbShellCheck(): UsbAdbDiagnosticResult {
    val logBuilder = StringBuilder()
    var adbServiceState = ""
    var usbConfig = ""
    var persistUsbConfig = ""
    var tcpPort = ""

    try {
        logBuilder.appendLine("$ getprop init.svc.adbd")
        adbServiceState = executeShellCommand("getprop init.svc.adbd").trim()
        logBuilder.appendLine("-> adbd service state: $adbServiceState")

        logBuilder.appendLine("$ getprop sys.usb.config")
        usbConfig = executeShellCommand("getprop sys.usb.config").trim()
        logBuilder.appendLine("-> sys.usb.config: $usbConfig")

        logBuilder.appendLine("$ getprop persist.sys.usb.config")
        persistUsbConfig = executeShellCommand("getprop persist.sys.usb.config").trim()
        logBuilder.appendLine("-> persist.sys.usb.config: $persistUsbConfig")

        logBuilder.appendLine("$ getprop service.adb.tcp.port")
        tcpPort = executeShellCommand("getprop service.adb.tcp.port").trim()
        if (tcpPort.isNotBlank()) {
            logBuilder.appendLine("-> wireless adb tcp port: $tcpPort")
        }

        logBuilder.appendLine("$ adb devices 2>&1")
        val adbDevicesOutput = executeShellCommand("adb devices 2>&1").trim()
        if (adbDevicesOutput.isNotBlank()) {
            logBuilder.appendLine("-> adb devices output:\n$adbDevicesOutput")
        } else {
            logBuilder.appendLine("-> adb client tool check completed (local system shell mode)")
        }
    } catch (e: Exception) {
        logBuilder.appendLine("Diagnostic exception: ${e.message}")
    }

    val isAdbRunning = adbServiceState.equals("running", ignoreCase = true) ||
        usbConfig.contains("adb", ignoreCase = true) ||
        persistUsbConfig.contains("adb", ignoreCase = true)

    val isUsbConfigured = usbConfig.contains("adb", ignoreCase = true) ||
        persistUsbConfig.contains("adb", ignoreCase = true) ||
        adbServiceState.isNotBlank()

    return UsbAdbDiagnosticResult(
        isAdbDaemonRunning = isAdbRunning,
        isUsbDebuggingConfigured = isUsbConfigured,
        adbDaemonServiceState = if (adbServiceState.isBlank()) "unknown" else adbServiceState,
        usbConfig = if (usbConfig.isBlank()) "none" else usbConfig,
        persistentUsbConfig = if (persistUsbConfig.isBlank()) "none" else persistUsbConfig,
        adbTcpPort = tcpPort,
        rawOutputLog = logBuilder.toString().trimEnd()
    )
}

private fun executeShellCommand(cmd: String): String {
    return try {
        val process = ProcessBuilder("/system/bin/sh", "-c", cmd)
            .redirectErrorStream(true)
            .start()
        val text = process.inputStream.bufferedReader().readText()
        process.waitFor()
        text
    } catch (e: Exception) {
        "Error: ${e.message}"
    }
}
