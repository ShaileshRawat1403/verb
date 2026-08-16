package com.example.verb.ui

import android.os.Build
import android.os.Environment
import android.os.StatFs
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Memory
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material.icons.filled.SdCard
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.verb.ai.AiModelPresets
import com.example.verb.ai.AiProviderConfig
import com.example.verb.ai.AiProviderId
import com.example.verb.ai.AiProviderSettings
import com.example.verb.terminal.TerminalEnvironment
import com.example.verb.terminal.AgentRuntimeInstaller
import com.example.verb.terminal.RuntimeProfileId
import com.example.verb.terminal.RuntimeProfileReport
import com.example.verb.ui.theme.SecondaryCyan
import java.util.Locale

@Composable
fun SystemScreen(
    isTerminalSessionActive: Boolean,
    terminalEnvironment: TerminalEnvironment,
    aiProviderSettings: AiProviderSettings = AiProviderSettings(),
    onSaveAiProviderSettings: (AiProviderConfig, String?) -> Result<Unit> = { _, _ -> Result.success(Unit) },
    onClearAiProviderApiKey: () -> Unit = {},
    onOpenTerminal: () -> Unit = {},
    distributionName: String = "Full CLI",
    runtimeProfileReports: List<RuntimeProfileReport> = emptyList(),
    installingRuntimeProfile: RuntimeProfileId? = null,
    runtimeInstallMessage: String? = null,
    onInstallRuntimeProfile: (RuntimeProfileId) -> Unit = {},
    agentRuntime: AgentRuntimeInstaller.InstalledRuntime? = null,
    agentRuntimeImporting: Boolean = false,
    agentRuntimeMessage: String? = null,
    agentArchiveName: String? = null,
    agentChecksumName: String? = null,
    agentManifestName: String? = null,
    onPickAgentArchive: () -> Unit = {},
    onPickAgentChecksum: () -> Unit = {},
    onPickAgentManifest: () -> Unit = {},
    onImportAgentRuntime: () -> Unit = {},
    onOpenAgentRuntime: () -> Unit = {},
    onReturnToVerbRuntime: () -> Unit = {},
    modifier: Modifier = Modifier
) {
    val scrollState = rememberScrollState()

    // Calculate live storage stats
    val dataDir = Environment.getDataDirectory()
    val stat = StatFs(dataDir.path)
    val totalStorageGb = (stat.blockCountLong * stat.blockSizeLong) / (1024.0 * 1024.0 * 1024.0)
    val availStorageGb = (stat.availableBlocksLong * stat.blockSizeLong) / (1024.0 * 1024.0 * 1024.0)
    val usedStorageGb = totalStorageGb - availStorageGb
    val storageFraction = if (totalStorageGb > 0) (usedStorageGb / totalStorageGb).toFloat() else 0f

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .padding(16.dp)
            .verticalScroll(scrollState)
    ) {
        Spacer(modifier = Modifier.height(16.dp))

        Text(
            text = "System & setup",
            style = MaterialTheme.typography.headlineMedium,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onBackground
        )

        Text(
            text = "Configure AI first, then inspect your terminal runtime and device.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp, bottom = 20.dp)
        )

        AiProviderSettingsCard(
            settings = aiProviderSettings,
            onSave = onSaveAiProviderSettings,
            onClearApiKey = onClearAiProviderApiKey
        )

        Spacer(modifier = Modifier.height(16.dp))

        // Device Specs Card
        SystemMetricCard(
            title = "Device Information",
            icon = Icons.Default.PhoneAndroid,
            details = listOf(
                "Model" to "${Build.MANUFACTURER.capitalize()} ${Build.MODEL}",
                "Android Version" to "${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})",
                "Architecture" to Build.SUPPORTED_ABIS.firstOrNull().orEmpty(),
                "Hardware Board" to Build.BOARD
            )
        )

        Spacer(modifier = Modifier.height(16.dp))

        // Storage Usage Gauge Card
        Card(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(16.dp),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
        ) {
            Column(modifier = Modifier.padding(16.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(
                        imageVector = Icons.Default.SdCard,
                        contentDescription = null,
                        tint = SecondaryCyan
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = "Storage Space",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.Bold
                    )
                }

                Spacer(modifier = Modifier.height(12.dp))

                Text(
                    text = String.format(
                        Locale.getDefault(),
                        "%.1f GB used of %.1f GB (%.1f GB available)",
                        usedStorageGb,
                        totalStorageGb,
                        availStorageGb
                    ),
                    fontSize = 13.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                Spacer(modifier = Modifier.height(8.dp))

                LinearProgressIndicator(
                    progress = { storageFraction },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(8.dp),
                    color = SecondaryCyan,
                    trackColor = MaterialTheme.colorScheme.surfaceVariant
                )
            }
        }

        Spacer(modifier = Modifier.height(16.dp))

        // Runtime Status Card
        SystemMetricCard(
            title = "Verb Runtime Status",
            icon = Icons.Default.Terminal,
            details = listOf(
                "Runtime Environment" to "Verb / Android PTY Adapter",
                "Distribution" to distributionName,
                "CLI Userland" to terminalEnvironment.displayName,
                "Shell Process" to terminalEnvironment.shellExecutable,
                "Session State" to if (isTerminalSessionActive) "ACTIVE (Running)" else "INACTIVE",
                "Root Access" to "No Root (Unprivileged Safe User)"
            )
        )

        Spacer(modifier = Modifier.height(8.dp))
        Button(
            onClick = onOpenTerminal,
            modifier = Modifier
                .fillMaxWidth()
                .testTag("btn_system_open_terminal")
        ) {
            Icon(
                imageVector = Icons.Default.Terminal,
                contentDescription = null,
                modifier = Modifier.size(16.dp)
            )
            Spacer(modifier = Modifier.width(8.dp))
            Text("Open Terminal")
        }

        Spacer(modifier = Modifier.height(16.dp))

        RuntimeProfilesCard(
            reports = runtimeProfileReports,
            installingProfile = installingRuntimeProfile,
            message = runtimeInstallMessage,
            onInstall = onInstallRuntimeProfile
        )

        Spacer(modifier = Modifier.height(16.dp))

        AgentRuntimeCard(
            runtime = agentRuntime,
            importing = agentRuntimeImporting,
            message = agentRuntimeMessage,
            archiveName = agentArchiveName,
            checksumName = agentChecksumName,
            manifestName = agentManifestName,
            onPickArchive = onPickAgentArchive,
            onPickChecksum = onPickAgentChecksum,
            onPickManifest = onPickAgentManifest,
            onImport = onImportAgentRuntime,
            onOpen = onOpenAgentRuntime,
            onReturnToVerb = onReturnToVerbRuntime
        )

        Spacer(modifier = Modifier.height(16.dp))

        UsbDebuggingDiagnosticCard()

        Spacer(modifier = Modifier.height(24.dp))
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AiProviderSettingsCard(
    settings: AiProviderSettings,
    onSave: (AiProviderConfig, String?) -> Result<Unit>,
    onClearApiKey: () -> Unit
) {
    var selectedProvider by remember(settings.config?.providerId) {
        mutableStateOf(settings.config?.providerId ?: AiProviderId.OPENAI)
    }
    var model by remember(settings.config?.model) { mutableStateOf(settings.config?.model.orEmpty()) }
    var baseUrl by remember(settings.config?.baseUrl, selectedProvider) {
        mutableStateOf(settings.config?.baseUrl ?: selectedProvider.defaultBaseUrl)
    }
    var apiKeyInput by remember(selectedProvider) { mutableStateOf("") }
    var feedback by remember { mutableStateOf<String?>(null) }
    val hasKeyForSelectedProvider = settings.hasApiKey && settings.config?.providerId == selectedProvider

    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text("AI Provider", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
            Text(
                "Bring your own API key. The key is encrypted with Android Keystore and is never sent to Terminal.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 12.dp)
            )

            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                AiProviderId.entries.forEach { provider ->
                    FilterChip(
                        selected = selectedProvider == provider,
                        onClick = {
                            selectedProvider = provider
                            baseUrl = provider.defaultBaseUrl
                            model = ""
                            feedback = null
                        },
                        label = { Text(provider.displayName) }
                    )
                }
            }

            val presets = AiModelPresets.forProvider(selectedProvider)
            if (presets.isNotEmpty()) {
                Text(
                    text = "Suggested model IDs",
                    style = MaterialTheme.typography.labelMedium,
                    modifier = Modifier.padding(top = 12.dp)
                )
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .padding(top = 4.dp)
                ) {
                    presets.forEach { preset ->
                        FilterChip(
                            selected = model == preset,
                            onClick = { model = preset },
                            label = { Text(preset) }
                        )
                    }
                }
                Text(
                    text = "Availability depends on your account and endpoint.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 4.dp)
                )
            }

            OutlinedTextField(
                value = model,
                onValueChange = { model = it },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 12.dp)
                    .testTag("ai_provider_model"),
                label = { Text("Model ID") },
                placeholder = { Text("Select a suggestion or enter a custom ID") },
                singleLine = true
            )
            OutlinedTextField(
                value = baseUrl,
                onValueChange = { baseUrl = it },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp)
                    .testTag("ai_provider_endpoint"),
                label = { Text("HTTPS endpoint") },
                placeholder = { Text(selectedProvider.defaultBaseUrl.ifBlank { "https://…" }) },
                singleLine = true
            )
            OutlinedTextField(
                value = apiKeyInput,
                onValueChange = { apiKeyInput = it },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp)
                    .testTag("ai_provider_api_key"),
                label = { Text(if (hasKeyForSelectedProvider) "New API key (optional)" else "API key") },
                placeholder = {
                    Text(if (hasKeyForSelectedProvider) "A key is stored for this provider" else "Paste this provider's API key")
                },
                visualTransformation = PasswordVisualTransformation(),
                singleLine = true
            )

            feedback?.let {
                Text(
                    it,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 8.dp)
                )
            }

            Row(
                modifier = Modifier.padding(top = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Button(
                    onClick = {
                        if (!hasKeyForSelectedProvider && apiKeyInput.isBlank()) {
                            feedback = "An API key is required to enable this provider."
                            return@Button
                        }
                        if (model.isBlank()) {
                            feedback = "Select or enter a model ID."
                            return@Button
                        }
                        onSave(
                            AiProviderConfig(selectedProvider, model, baseUrl),
                            apiKeyInput.takeIf { it.isNotBlank() }
                        ).onSuccess {
                            apiKeyInput = ""
                            feedback = "Provider settings saved."
                        }.onFailure { exception ->
                            feedback = exception.message ?: "Provider settings could not be saved."
                        }
                    },
                    modifier = Modifier.testTag("save_ai_provider")
                ) {
                    Text("Save provider")
                }
                if (hasKeyForSelectedProvider) {
                    OutlinedButton(onClick = {
                        onClearApiKey()
                        feedback = "Saved API key removed."
                    }) {
                        Text("Remove key")
                    }
                }
            }
        }
    }
}

@Composable
private fun SystemMetricCard(
    title: String,
    icon: ImageVector,
    details: List<Pair<String, String>>
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary
                )
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold
                )
            }

            Spacer(modifier = Modifier.height(12.dp))

            details.forEach { (label, value) ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 4.dp),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        text = label,
                        fontSize = 13.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(
                        text = value,
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Medium,
                        fontFamily = FontFamily.Monospace,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                }
            }
        }
    }
}

private fun String.capitalize(): String {
    return this.replaceFirstChar { if (it.isLowerCase()) it.titlecase() else it.toString() }
}
