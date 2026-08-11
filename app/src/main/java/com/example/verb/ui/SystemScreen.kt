package com.example.verb.ui

import android.os.Build
import android.os.Environment
import android.os.StatFs
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
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
import com.example.verb.ai.AiProviderConfig
import com.example.verb.ai.AiProviderId
import com.example.verb.ai.AiProviderSettings
import com.example.verb.terminal.TerminalEnvironment
import com.example.verb.viewmodel.RuntimeImportState
import com.example.verb.ui.theme.SecondaryCyan
import java.util.Locale

@Composable
fun SystemScreen(
    isTerminalSessionActive: Boolean,
    terminalEnvironment: TerminalEnvironment,
    runtimeImportState: RuntimeImportState = RuntimeImportState.Idle,
    onImportRuntime: () -> Unit = {},
    aiProviderSettings: AiProviderSettings = AiProviderSettings(),
    onSaveAiProviderSettings: (AiProviderConfig, String?) -> Result<Unit> = { _, _ -> Result.success(Unit) },
    onClearAiProviderApiKey: () -> Unit = {},
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
            text = "System Overview",
            style = MaterialTheme.typography.headlineMedium,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onBackground
        )

        Text(
            text = "Device specs, memory, storage & Verb runtime state.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp, bottom = 20.dp)
        )

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
                "Runtime Environment" to "Termux / Android PTY Adapter",
                "CLI Userland" to terminalEnvironment.displayName,
                "Shell Process" to terminalEnvironment.shellExecutable,
                "Session State" to if (isTerminalSessionActive) "ACTIVE (Running)" else "INACTIVE",
                "Root Access" to "No Root (Unprivileged Safe User)"
            )
        )

        Spacer(modifier = Modifier.height(16.dp))

        Card(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(16.dp),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
        ) {
            Column(modifier = Modifier.padding(16.dp)) {
                Text("Verb Runtime", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
                Text(
                    "Import the reviewed Verb aarch64 runtime ZIP and its SHA-256 file. The archive is verified before installation.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 4.dp, bottom = 12.dp)
                )
                Button(
                    onClick = onImportRuntime,
                    enabled = runtimeImportState !is RuntimeImportState.Importing,
                    modifier = Modifier.testTag("import_verb_runtime")
                ) {
                    Text(if (runtimeImportState is RuntimeImportState.Importing) "Importing…" else "Import Verb Runtime")
                }
                when (runtimeImportState) {
                    RuntimeImportState.Idle -> Unit
                    RuntimeImportState.Importing -> Text("Checking checksum and installing…", modifier = Modifier.padding(top = 8.dp))
                    RuntimeImportState.Success -> Text("Verb local CLI userland installed and terminal restarted.", modifier = Modifier.padding(top = 8.dp))
                    is RuntimeImportState.Failure -> Text(
                        "Import failed: ${runtimeImportState.message}",
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 8.dp)
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(16.dp))

        AiProviderSettingsCard(
            settings = aiProviderSettings,
            onSave = onSaveAiProviderSettings,
            onClearApiKey = onClearAiProviderApiKey
        )

        Spacer(modifier = Modifier.height(24.dp))
    }
}

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
    var apiKeyInput by remember { mutableStateOf("") }
    var feedback by remember { mutableStateOf<String?>(null) }

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

            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                AiProviderId.entries.take(2).forEach { provider ->
                    FilterChip(
                        selected = selectedProvider == provider,
                        onClick = {
                            selectedProvider = provider
                            baseUrl = provider.defaultBaseUrl
                            feedback = null
                        },
                        label = { Text(provider.displayName) }
                    )
                }
            }
            Spacer(modifier = Modifier.height(6.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                AiProviderId.entries.drop(2).forEach { provider ->
                    FilterChip(
                        selected = selectedProvider == provider,
                        onClick = {
                            selectedProvider = provider
                            baseUrl = provider.defaultBaseUrl
                            feedback = null
                        },
                        label = { Text(provider.displayName) }
                    )
                }
            }

            OutlinedTextField(
                value = model,
                onValueChange = { model = it },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 12.dp)
                    .testTag("ai_provider_model"),
                label = { Text("Model") },
                placeholder = { Text("Choose a model available to your account") },
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
                label = { Text(if (settings.hasApiKey) "New API key (optional)" else "API key") },
                placeholder = {
                    Text(if (settings.hasApiKey) "A key is already stored securely" else "Paste your provider API key")
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
                        if (!settings.hasApiKey && apiKeyInput.isBlank()) {
                            feedback = "An API key is required to enable this provider."
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
                if (settings.hasApiKey) {
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
