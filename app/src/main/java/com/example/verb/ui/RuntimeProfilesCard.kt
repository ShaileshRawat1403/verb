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
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.example.verb.terminal.RuntimeProfileId
import com.example.verb.terminal.RuntimeProfileReport

@Composable
fun RuntimeProfilesCard(
    reports: List<RuntimeProfileReport>,
    installingProfile: RuntimeProfileId?,
    message: String?,
    onInstall: (RuntimeProfileId) -> Unit,
    modifier: Modifier = Modifier
) {
    Card(
        modifier = modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text("Runtime Capabilities", style = MaterialTheme.typography.titleMedium)
            Text(
                "Install only the toolchains you need. Verb checks package and version requirements before running apt.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 12.dp)
            )

            reports.forEachIndexed { index, report ->
                if (index > 0) HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))
                val installing = installingProfile == report.profile.id
                val blocked = report.incompatibleCommands.isNotEmpty()
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .testTag("runtime_profile_${report.profile.id.name.lowercase()}")
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(report.profile.displayName, style = MaterialTheme.typography.titleSmall)
                        Text(
                            when {
                                installing -> "Installing"
                                report.isReady -> "Ready"
                                blocked -> "Blocked"
                                else -> "Available"
                            },
                            color = when {
                                report.isReady -> MaterialTheme.colorScheme.primary
                                blocked -> MaterialTheme.colorScheme.error
                                else -> MaterialTheme.colorScheme.onSurfaceVariant
                            },
                            style = MaterialTheme.typography.labelMedium
                        )
                    }
                    val details = buildList {
                        if (report.missingPackages.isNotEmpty()) {
                            add("Packages: ${report.missingPackages.joinToString()}")
                        }
                        if (report.missingCommands.isNotEmpty()) {
                            add("Commands: ${report.missingCommands.joinToString()}")
                        }
                        if (blocked) {
                            add("Incompatible: ${report.incompatibleCommands.joinToString()}")
                        }
                    }
                    if (details.isNotEmpty()) {
                        Text(
                            details.joinToString("\n"),
                            style = MaterialTheme.typography.bodySmall,
                            color = if (blocked) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 4.dp)
                        )
                    }
                    report.profile.postInstallHint?.let { hint ->
                        Text(
                            text = if (report.isReady) hint else "Requires JavaScript. Verb never copies Assistant API keys into CLI agents.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 4.dp)
                        )
                    }
                    if (!report.isReady && !blocked) {
                        Spacer(modifier = Modifier.height(8.dp))
                        Button(
                            onClick = { onInstall(report.profile.id) },
                            enabled = installingProfile == null,
                            modifier = Modifier.testTag("runtime_install_${report.profile.id.name.lowercase()}")
                        ) {
                            Text(if (installing) "Installing..." else "Install")
                        }
                    }
                }
            }
            if (message != null) {
                Text(
                    message,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 12.dp)
                )
            }
        }
    }
}
