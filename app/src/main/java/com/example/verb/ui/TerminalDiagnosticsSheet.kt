package com.example.verb.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.BugReport
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.DeleteSweep
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
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
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.example.verb.terminal.LogCategory
import com.example.verb.terminal.LogLevel
import com.example.verb.terminal.TerminalLogEntry
import com.example.verb.terminal.TerminalRuntimeAdapter
import com.example.verb.terminal.TerminalSessionLogger
import com.example.verb.terminal.TerminalSessionState
import com.example.verb.terminal.ShellDiagnosticsReport
import com.example.verb.terminal.TerminalDiagnostics
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun TerminalDiagnosticsSheet(
    terminalRuntime: TerminalRuntimeAdapter?,
    onDismiss: () -> Unit
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val clipboardManager = LocalClipboardManager.current
    val logs by TerminalSessionLogger.logs.collectAsStateWithLifecycle()
    val sessionState by (terminalRuntime?.sessionState?.collectAsStateWithLifecycle()
        ?: remember { mutableStateOf(TerminalSessionState.FAILED) })
    val currentWorkingDirectory by (terminalRuntime?.currentWorkingDirectory?.collectAsStateWithLifecycle()
        ?: remember { mutableStateOf(null) })
    val launchWorkingDirectory = terminalRuntime?.launchWorkingDirectory
    // Where a host-side diagnostic probe should actually run: the shell's real directory when Verb
    // could map it, otherwise the launch directory. Never a guest path handed to File().
    val probeDirectory = currentWorkingDirectory?.hostPath ?: launchWorkingDirectory

    var selectedCategoryFilter by remember { mutableStateOf<LogCategory?>(null) }
    var copyNoticeVisible by remember { mutableStateOf(false) }
    var shellVerification by remember { mutableStateOf<ShellDiagnosticsReport?>(null) }
    var isVerifyingShell by remember { mutableStateOf(false) }
    val coroutineScope = rememberCoroutineScope()

    val filteredLogs = if (selectedCategoryFilter == null) {
        logs
    } else {
        logs.filter { it.category == selectedCategoryFilter }
    }

    val (statusLabel, statusColor) = when (sessionState) {
        TerminalSessionState.RUNNING -> "Connected (Native PTY)" to Color(0xFF22C55E)
        TerminalSessionState.STARTING -> "Connecting..." to Color(0xFFEAB308)
        TerminalSessionState.FAILED -> "Session Failed" to Color(0xFFEF4444)
        TerminalSessionState.EXITED -> "Disconnected" to Color(0xFF94A3B8)
        TerminalSessionState.STOPPING -> "Stopping..." to Color(0xFFF97316)
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface,
        contentColor = MaterialTheme.colorScheme.onSurface,
        modifier = Modifier.testTag("terminal_diagnostics_sheet")
    ) {
        // The bottom action row (Clear Logs / Copy Report) used to be laid out past the bottom of
        // the sheet on tall devices and was unreachable: the sheet claimed a fixed 0.85 of its
        // container without reserving the navigation-bar inset, so the last child fell underneath
        // the system bars with nothing to scroll it into view. The fixed fraction is now a maximum
        // rather than an exact height, the system insets are consumed here, and the only region
        // allowed to grow is the log list -- so the actions keep their place above the insets at
        // any font scale, without shrinking any touch target.
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(max = LocalConfiguration.current.screenHeightDp.dp * 0.85f)
                .navigationBarsPadding()
                .imePadding()
                .padding(horizontal = 16.dp, vertical = 8.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(
                        imageVector = Icons.Default.BugReport,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(22.dp)
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = "Terminal Session Diagnostics",
                        fontSize = 17.sp,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                }
                IconButton(onClick = onDismiss, modifier = Modifier.testTag("btn_close_diagnostics")) {
                    Icon(
                        imageVector = Icons.Default.Close,
                        contentDescription = "Close",
                        tint = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            Spacer(modifier = Modifier.height(12.dp))

            Surface(
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(12.dp),
                color = MaterialTheme.colorScheme.surfaceVariant
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Box(
                                modifier = Modifier
                                    .size(10.dp)
                                    .background(statusColor, CircleShape)
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text(
                                text = "State: $statusLabel",
                                fontSize = 14.sp,
                                fontWeight = FontWeight.SemiBold,
                                color = MaterialTheme.colorScheme.onSurface
                            )
                        }

                        Button(
                            onClick = { terminalRuntime?.restartSession() },
                            colors = ButtonDefaults.buttonColors(
                                containerColor = MaterialTheme.colorScheme.primary
                            ),
                            shape = RoundedCornerShape(8.dp),
                            modifier = Modifier
                                .height(32.dp)
                                .testTag("btn_reconnect_diagnostics")
                        ) {
                            Icon(
                                imageVector = Icons.Default.Refresh,
                                contentDescription = null,
                                modifier = Modifier.size(14.dp)
                            )
                            Spacer(modifier = Modifier.width(4.dp))
                            Text("Reconnect", fontSize = 12.sp)
                        }
                    }

                    Spacer(modifier = Modifier.height(8.dp))
                    // Two distinct facts, never collapsed into one line. "Launch directory" is
                    // where the PTY process was started and never moves; "Current directory" is
                    // where the shell actually is, and is honestly reported as unknown wherever
                    // shell integration does not run (the Agent Runtime, a non-bash shell, or
                    // before the first prompt of a session).
                    Text(
                        text = "Launch directory (device path): ${launchWorkingDirectory?.absolutePath ?: "Unknown"}",
                        fontSize = 12.sp,
                        fontFamily = FontFamily.Monospace,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.testTag("diagnostics_launch_directory")
                    )
                    Spacer(modifier = Modifier.height(4.dp))
                    Text(
                        text = "Current directory (terminal path): " +
                            (currentWorkingDirectory?.guestPath ?: "Unknown — shell integration unavailable"),
                        fontSize = 12.sp,
                        fontFamily = FontFamily.Monospace,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.testTag("diagnostics_current_directory")
                    )

                    Spacer(modifier = Modifier.height(8.dp))
                    OutlinedButton(
                        onClick = {
                            coroutineScope.launch {
                                isVerifyingShell = true
                                shellVerification = withContext(Dispatchers.IO) {
                                    TerminalDiagnostics.executeShellVerification(probeDirectory)
                                }
                                isVerifyingShell = false
                            }
                        },
                        enabled = !isVerifyingShell,
                        shape = RoundedCornerShape(8.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(34.dp)
                            .testTag("btn_run_shell_verification")
                    ) {
                        Text(
                            if (isVerifyingShell) "Verifying system shell..." else "Verify system shell",
                            fontSize = 11.sp,
                            color = MaterialTheme.colorScheme.primary
                        )
                    }
                    shellVerification?.let { report ->
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = if (report.isAccessible) {
                                "System shell available: ${report.binaryCount} commands found in ${report.executionTimeMs} ms."
                            } else {
                                "System shell unavailable: ${report.errorDetails ?: "Unknown error"}"
                            },
                            fontSize = 12.sp,
                            color = if (report.isAccessible) Color(0xFF86EFAC) else MaterialTheme.colorScheme.error
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(12.dp))

            // Horizontally scrollable: the category set grows with LogCategory, and an unscrollable
            // Row would either clip the last chips or force the sheet wider than the screen.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                FilterChip(
                    label = "All (${logs.size})",
                    isSelected = selectedCategoryFilter == null,
                    onClick = { selectedCategoryFilter = null }
                )
                LogCategory.entries.forEach { category ->
                    val count = logs.count { it.category == category }
                    FilterChip(
                        label = "${category.name} ($count)",
                        isSelected = selectedCategoryFilter == category,
                        onClick = { selectedCategoryFilter = category }
                    )
                }
            }

            Spacer(modifier = Modifier.height(12.dp))

            // The only child allowed to absorb leftover height, and the only one allowed to shrink:
            // `fill = false` lets it give space back at large font scales instead of pushing the
            // action row below the sheet, while heightIn keeps it usable rather than collapsing.
            Box(
                modifier = Modifier
                    .weight(1f, fill = false)
                    .heightIn(min = 96.dp)
                    .fillMaxWidth()
                    .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(8.dp))
                    .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(8.dp))
                    .padding(8.dp)
            ) {
                if (filteredLogs.isEmpty()) {
                    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        Text(
                            text = "No log entries recorded for this category.",
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            fontSize = 13.sp
                        )
                    }
                } else {
                    // The list follows new entries only while the reader is already near the
                    // bottom -- reproducing a bug means watching lines arrive, but reading older
                    // history means new lines must not yank the list out from under the eyes.
                    val listState = rememberLazyListState()
                    LaunchedEffect(filteredLogs.size) {
                        val last = filteredLogs.lastIndex
                        val bottomVisible = listState.layoutInfo.visibleItemsInfo
                            .lastOrNull()?.index ?: last
                        if (last >= 0 && bottomVisible >= last - 2) {
                            listState.scrollToItem(last)
                        }
                    }
                    LazyColumn(state = listState, modifier = Modifier.fillMaxSize()) {
                        items(filteredLogs) { entry ->
                            LogEntryRow(entry)
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(12.dp))

            // FlowRow, not Row: at large font scales the two labelled buttons no longer fit side by
            // side, and a plain Row would clip "Copy Report" rather than move it. Wrapping onto a
            // second line keeps both actions fully visible and full-size.
            FlowRow(
                modifier = Modifier
                    .fillMaxWidth()
                    .testTag("diagnostics_actions"),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                OutlinedButton(
                    onClick = { TerminalSessionLogger.clear() },
                    shape = RoundedCornerShape(8.dp),
                    colors = ButtonDefaults.outlinedButtonColors(
                        contentColor = MaterialTheme.colorScheme.error
                    ),
                    modifier = Modifier.testTag("btn_clear_diagnostics_logs")
                ) {
                    Icon(
                        imageVector = Icons.Default.DeleteSweep,
                        contentDescription = null,
                        modifier = Modifier.size(16.dp)
                    )
                    Spacer(modifier = Modifier.width(6.dp))
                    Text("Clear Logs", fontSize = 12.sp)
                }

                Button(
                    onClick = {
                        val report = TerminalSessionLogger.exportDiagnosticReport(
                            sessionState = sessionState,
                            launchWorkingDir = launchWorkingDirectory?.absolutePath,
                            currentWorkingDir = currentWorkingDirectory?.guestPath,
                            shellExecutable = "/system/bin/sh"
                        )
                        clipboardManager.setText(AnnotatedString(report))
                        copyNoticeVisible = true
                    },
                    shape = RoundedCornerShape(8.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF22C55E)),
                    modifier = Modifier.testTag("btn_copy_diagnostics_report")
                ) {
                    Icon(
                        imageVector = Icons.Default.ContentCopy,
                        contentDescription = null,
                        modifier = Modifier.size(16.dp)
                    )
                    Spacer(modifier = Modifier.width(6.dp))
                    Text(if (copyNoticeVisible) "Report Copied!" else "Copy Report", fontSize = 12.sp)
                }
            }
        }
    }
}

@Composable
private fun FilterChip(
    label: String,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    Surface(
        shape = RoundedCornerShape(16.dp),
        color = if (isSelected) {
            MaterialTheme.colorScheme.primary
        } else {
            MaterialTheme.colorScheme.surfaceVariant
        },
        modifier = Modifier.clickable { onClick() }
    ) {
        Text(
            text = label,
            fontSize = 11.sp,
            fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Normal,
            color = if (isSelected) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 5.dp)
        )
    }
}

@Composable
private fun LogEntryRow(entry: TerminalLogEntry) {
    val levelColor = when (entry.level) {
        LogLevel.ERROR -> Color(0xFFEF4444)
        LogLevel.WARN -> Color(0xFFEAB308)
        LogLevel.INFO -> Color(0xFF3B82F6)
        LogLevel.DEBUG -> Color(0xFF94A3B8)
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = entry.timestamp,
                fontSize = 10.sp,
                fontFamily = FontFamily.Monospace,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(modifier = Modifier.width(6.dp))
            Surface(
                shape = RoundedCornerShape(4.dp),
                color = levelColor.copy(alpha = 0.2f)
            ) {
                Text(
                    text = entry.level.name,
                    fontSize = 9.sp,
                    fontWeight = FontWeight.Bold,
                    color = levelColor,
                    modifier = Modifier.padding(horizontal = 4.dp, vertical = 2.dp)
                )
            }
            Spacer(modifier = Modifier.width(6.dp))
            Text(
                text = "[${entry.category.name}]",
                fontSize = 10.sp,
                fontFamily = FontFamily.Monospace,
                color = MaterialTheme.colorScheme.primary
            )
        }
        Spacer(modifier = Modifier.height(2.dp))
        Text(
            text = entry.message,
            fontSize = 11.sp,
            fontFamily = FontFamily.Monospace,
            color = MaterialTheme.colorScheme.onSurface
        )
    }
}
