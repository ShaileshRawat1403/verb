package com.example.verb.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.example.verb.terminal.AgentCompatibilityState
import com.example.verb.terminal.AgentRuntimeStatus

/** Entry point for the optional Linux runtime that hosts Claude Code and OpenCode. */
@Composable
fun AgentRuntimeCard(
    status: AgentRuntimeStatus,
    importing: Boolean,
    message: String?,
    archiveName: String?,
    checksumName: String?,
    manifestName: String?,
    onPickArchive: () -> Unit,
    onPickChecksum: () -> Unit,
    onPickManifest: () -> Unit,
    onImport: () -> Unit,
    onOpen: () -> Unit,
    onCheckCompatibility: () -> Unit,
    onReturnToVerb: () -> Unit,
    modifier: Modifier = Modifier
) {
    val complete = archiveName != null && checksumName != null && manifestName != null
    val runtime = status.runtime
    Card(
        modifier = modifier.fillMaxWidth().testTag("agent_runtime_card"),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text("Agent Runtime", style = MaterialTheme.typography.titleMedium)
            Text(
                "A separate ARM64 Linux userland for Claude Code and OpenCode. It shares only the selected project and never replaces the normal Verb terminal.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp)
            )
            Spacer(modifier = Modifier.height(10.dp))
            Text(
                runtime?.let { "Installed: ${it.manifest.runtimeVersion} · ${it.manifest.distro}" } ?: "Not installed",
                style = MaterialTheme.typography.labelLarge,
                color = if (runtime != null) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.testTag("agent_runtime_artifact_state")
            )
            // Deliberately a second line, not merged into the one above: "installed" and "runs here"
            // are different claims, and merging them is what previously let a launch button appear
            // for a runtime that could not execute.
            if (status.isInstalled) {
                Text(
                    when (status.compatibility) {
                        AgentCompatibilityState.NOT_CHECKED -> "Compatibility: not checked yet"
                        AgentCompatibilityState.CHECKING -> "Compatibility: checking…"
                        AgentCompatibilityState.COMPATIBLE -> "Compatibility: runs on this device"
                        AgentCompatibilityState.INCOMPATIBLE -> "Compatibility: cannot run on this device"
                        AgentCompatibilityState.CHECK_FAILED -> "Compatibility: check could not run"
                        AgentCompatibilityState.CHECK_TIMED_OUT -> "Compatibility: check timed out"
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = when (status.compatibility) {
                        AgentCompatibilityState.COMPATIBLE -> MaterialTheme.colorScheme.primary
                        AgentCompatibilityState.INCOMPATIBLE -> MaterialTheme.colorScheme.error
                        else -> MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    modifier = Modifier.padding(top = 2.dp).testTag("agent_runtime_compatibility_state")
                )
            }
            Spacer(modifier = Modifier.height(8.dp))
            Text("Choose the three files from the GitHub Actions artifact:", style = MaterialTheme.typography.bodySmall)
            FileChoiceRow("Rootfs", archiveName, onPickArchive)
            FileChoiceRow("Checksum", checksumName, onPickChecksum)
            FileChoiceRow("Manifest", manifestName, onPickManifest)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
                Button(
                    onClick = onImport,
                    enabled = complete && !importing,
                    modifier = Modifier.testTag("agent_runtime_import")
                ) { Text(if (importing) "Verifying…" else "Install runtime") }
                if (status.isInstalled) {
                    // Enabled only for COMPATIBLE. VerbViewModel.openAgentRuntime() enforces the
                    // same rule, so this is the visible half of a guard, not the whole of it.
                    OutlinedButton(
                        onClick = onOpen,
                        enabled = status.canOpen,
                        modifier = Modifier.testTag("agent_runtime_open")
                    ) {
                        Text("Open agent terminal")
                    }
                }
            }
            if (status.isInstalled) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
                    OutlinedButton(
                        onClick = onCheckCompatibility,
                        enabled = status.canCheck,
                        modifier = Modifier.testTag("agent_runtime_check")
                    ) { Text("Retry check") }
                    // Always available: the normal Verb terminal is unaffected by an incompatible
                    // Agent Runtime, and the installed rootfs is never deleted automatically.
                    OutlinedButton(onClick = onReturnToVerb, modifier = Modifier.testTag("agent_runtime_return")) {
                        Text("Use normal Verb terminal")
                    }
                }
            }
            message?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 8.dp))
            }
        }
    }
}

@Composable
private fun FileChoiceRow(label: String, selectedName: String?, onPick: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 6.dp),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text("$label: ${selectedName ?: "not selected"}", style = MaterialTheme.typography.bodySmall, modifier = Modifier.weight(1f))
        OutlinedButton(onClick = onPick) { Text("Choose") }
    }
}
