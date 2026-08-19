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
import com.example.verb.terminal.RuntimeProfiles
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
                "Toolchains the agents build on. Verb checks package and version requirements before running apt, and installs prerequisites for you.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 12.dp)
            )

            // Agents have their own surface now; this card is the toolchains behind them. Showing
            // both here made a thirteen-row list where setup and product looked identical.
            val toolchains = reports.filterNot { it.profile.isAgent }
            toolchains.forEachIndexed { index, report ->
                if (index > 0) HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))
                val installing = installingProfile == report.profile.id
                // Unsatisfiable: a version constraint is violated by what is already installed,
                // so no install action can ever resolve it. Distinct from "not installed yet".
                val unsatisfiable = report.isUnsatisfiable
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
                                unsatisfiable -> "Unavailable"
                                else -> "Available"
                            },
                            color = when {
                                report.isReady -> MaterialTheme.colorScheme.primary
                                unsatisfiable -> MaterialTheme.colorScheme.error
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
                        if (report.nonExecutableCommands.isNotEmpty()) {
                            add("Not executable: ${report.nonExecutableCommands.joinToString()}")
                        }
                        if (report.unverifiedCommands.isNotEmpty()) {
                            add("Failed verification: ${report.unverifiedCommands.joinToString()}")
                        }
                        if (report.timedOutCommands.isNotEmpty()) {
                            add("Verification timed out: ${report.timedOutCommands.joinToString()}")
                        }
                        if (unsatisfiable) {
                            // Name the required version and say no action helps, rather than the bare
                            // "Incompatible: python", which reads like something the user got wrong.
                            val needed = report.profile.requirements
                                .filter { it.command in report.incompatibleCommands && it.maxVersionExclusive != null }
                                .joinToString { "${it.command} below ${it.maxVersionExclusive}" }
                                .ifEmpty { report.incompatibleCommands.joinToString() }
                            add("Needs $needed; the package repository has no compatible version.")
                            add("No install can resolve this. The rest of Verb is unaffected.")
                        }
                    }
                    if (details.isNotEmpty()) {
                        Text(
                            details.joinToString("\n"),
                            style = MaterialTheme.typography.bodySmall,
                            color = if (unsatisfiable) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 4.dp)
                        )
                    }
                    report.profile.postInstallHint?.let { hint ->
                        Text(
                            text = if (report.isReady) hint else "Verb never copies Assistant API keys into CLI agents.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 4.dp)
                        )
                    }
                    if (report.isInstallable) {
                        val prerequisites = report.profile.prerequisiteProfiles
                            .map { RuntimeProfiles.forId(it).displayName }
                        if (prerequisites.isNotEmpty()) {
                            // Verb installs these itself; the user is told what will happen, not
                            // handed a list of chores to do first.
                            Text(
                                "Installs ${prerequisites.joinToString()} first if needed.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(top = 4.dp)
                            )
                        }
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
