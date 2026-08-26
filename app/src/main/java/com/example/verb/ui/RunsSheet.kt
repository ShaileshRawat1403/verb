package com.example.verb.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Error
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.example.verb.terminal.CommandExecutionRecord
import com.example.verb.terminal.CommandLifecycleState
import com.example.verb.terminal.TerminalRuntimeAdapter

/**
 * Read-only "Command Runs" sheet: a bounded, session-local, non-persistent list of completed
 * command lifecycle records built from advisory OSC 7/633 shell-integration markers (see
 * [com.example.verb.terminal.CommandExecutionTracker]).
 *
 * Deliberately shows only command text, pass/fail/interrupted state, duration, and exit code --
 * never transcript/output text, never the working directory, never an AI action, never a way to
 * re-run or retry a command. This is a status view, not a control surface.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RunsSheet(
    terminalRuntime: TerminalRuntimeAdapter?,
    onDismiss: () -> Unit
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val history by (terminalRuntime?.commandHistory?.collectAsStateWithLifecycle()
        ?: remember { mutableStateOf(emptyList()) })
    val integrationActive by (terminalRuntime?.shellIntegrationActive?.collectAsStateWithLifecycle()
        ?: remember { mutableStateOf(false) })

    // Newest first: commandHistory is append-ordered (oldest first), so reverse for display only.
    val newestFirst = remember(history) { history.asReversed() }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface,
        contentColor = MaterialTheme.colorScheme.onSurface,
        modifier = Modifier.testTag("terminal_runs_sheet")
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                // A maximum fraction, not an exact height, with the system insets consumed here
                // and only the run list allowed to grow -- the same repair the diagnostics sheet
                // needed first: a fixed 0.85 of the full container laid the privacy footer out
                // underneath the navigation bars on gesture-nav devices, where nothing could
                // scroll it into view.
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
                        imageVector = Icons.Default.History,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(22.dp)
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = "Command Runs",
                        fontSize = 17.sp,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                }
                IconButton(onClick = onDismiss, modifier = Modifier.testTag("btn_close_runs")) {
                    Icon(
                        imageVector = Icons.Default.Close,
                        contentDescription = "Close",
                        tint = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            Spacer(modifier = Modifier.height(12.dp))

            Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
                when {
                    newestFirst.isEmpty() && integrationActive -> EmptyRunsMessage(
                        text = "No commands run yet.",
                        testTag = "runs_empty_active"
                    )
                    newestFirst.isEmpty() -> EmptyRunsMessage(
                        text = "Runs will appear after the terminal shell is ready.",
                        testTag = "runs_empty_inactive"
                    )
                    else -> LazyColumn(modifier = Modifier.fillMaxSize().testTag("runs_list")) {
                        items(newestFirst, key = { it.id }) { record ->
                            RunRow(record)
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = "Local terminal activity. Nothing is sent to AI.",
                fontSize = 11.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.testTag("runs_privacy_footer")
            )
        }
    }
}

@Composable
private fun EmptyRunsMessage(text: String, testTag: String) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(
            text = text,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontSize = 13.sp,
            modifier = Modifier.testTag(testTag)
        )
    }
}

@Composable
private fun RunRow(record: CommandExecutionRecord) {
    val commandLabel = record.commandText.ifBlank { "Command run" }
    val (icon, iconTint, statusLabel) = when (record.state) {
        CommandLifecycleState.COMPLETED -> Triple(Icons.Default.CheckCircle, Color(0xFF22C55E), formatDuration(record.durationMs))
        CommandLifecycleState.FAILED -> Triple(
            Icons.Default.Error,
            MaterialTheme.colorScheme.error,
            "Failed · exit ${record.exitCode ?: "?"} · ${formatDuration(record.durationMs)}"
        )
        CommandLifecycleState.ABANDONED -> Triple(Icons.Default.Warning, Color(0xFFEAB308), "Interrupted")
        CommandLifecycleState.RUNNING -> Triple(Icons.Default.CheckCircle, Color(0xFF94A3B8), "") // not published to history; unreachable in practice
    }
    val rowDescription = "$commandLabel. ${describeStateForAccessibility(record)}"

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp)
            .testTag("run_row_${record.id}")
            .semantics(mergeDescendants = true) { contentDescription = rowDescription },
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null, // row-level contentDescription above already covers this
            tint = iconTint,
            modifier = Modifier
                .size(18.dp)
                .testTag("run_row_${record.id}_status_${record.state.name.lowercase()}")
        )
        Spacer(modifier = Modifier.width(10.dp))
        Text(
            text = commandLabel,
            fontFamily = FontFamily.Monospace,
            fontSize = 13.sp,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f)
        )
        Spacer(modifier = Modifier.width(10.dp))
        Text(
            text = statusLabel,
            fontSize = 11.sp,
            color = iconTint,
            maxLines = 1
        )
    }
}

/** Text-only description of state/exit/duration, independent of the icon's color, for TalkBack. */
private fun describeStateForAccessibility(record: CommandExecutionRecord): String = when (record.state) {
    CommandLifecycleState.COMPLETED -> "Completed in ${formatDuration(record.durationMs)}"
    CommandLifecycleState.FAILED -> "Failed with exit code ${record.exitCode ?: "unknown"}, ${formatDuration(record.durationMs)}"
    CommandLifecycleState.ABANDONED -> "Interrupted before it finished"
    CommandLifecycleState.RUNNING -> "Running"
}

private fun formatDuration(durationMs: Long?): String {
    val millis = durationMs ?: return "? ms"
    return if (millis < 1000) "$millis ms" else "%.1f s".format(millis / 1000.0)
}
