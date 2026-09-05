package com.example.verb.ui

import android.os.Build
import android.os.Environment
import android.os.StatFs
import androidx.compose.foundation.background
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.foundation.relocation.BringIntoViewRequester
import androidx.compose.foundation.relocation.bringIntoViewRequester
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.BuildConfig
import com.example.verb.ai.AiModelPresets
import com.example.verb.ai.AiProviderConfig
import com.example.verb.ai.AiProviderId
import com.example.verb.ai.AiProviderSettings
import com.example.verb.terminal.TerminalEnvironment
import com.example.verb.terminal.AgentRuntimeStatus
import com.example.verb.terminal.RuntimeProfileId
import com.example.verb.terminal.RuntimeProfileReport
import com.example.verb.ui.theme.SecondaryCyan
import java.util.Locale

/** A named task may share this screen while still landing at the section the user selected. */
enum class SystemSection {
    OVERVIEW,
    PROVIDER,
    WORKING_WORLD,
    CONTINUITY,
    RUNTIMES,
    AGENT_RUNTIME
}

@OptIn(ExperimentalFoundationApi::class)
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
    agentRuntimeStatus: AgentRuntimeStatus = AgentRuntimeStatus(),
    agentRuntimeImporting: Boolean = false,
    agentRuntimeMessage: String? = null,
    agentArchiveName: String? = null,
    agentChecksumName: String? = null,
    agentManifestName: String? = null,
    onPickAgentArchive: () -> Unit = {},
    onPickAgentChecksum: () -> Unit = {},
    onPickAgentManifest: () -> Unit = {},
    worldArchiveName: String? = null,
    worldArchiveMessage: String? = null,
    onSaveWorldToDownloads: () -> Unit = {},
    onPickWorldArchive: () -> Unit = {},
    continuityMessage: String? = null,
    continuityPreviewReady: Boolean = false,
    importedContinuitySessions: Int = 0,
    onExportContinuity: () -> Unit = {},
    onPickContinuity: () -> Unit = {},
    onApplyContinuity: () -> Unit = {},
    onImportAgentRuntime: () -> Unit = {},
    onOpenAgentRuntime: () -> Unit = {},
    onCheckAgentRuntime: () -> Unit = {},
    onReturnToVerbRuntime: () -> Unit = {},
    initialSection: SystemSection = SystemSection.OVERVIEW,
    modifier: Modifier = Modifier
) {
    val scrollState = rememberScrollState()
    val initialSectionRequester = remember(initialSection) { BringIntoViewRequester() }

    fun sectionModifier(section: SystemSection): Modifier = if (initialSection == section) {
        Modifier.bringIntoViewRequester(initialSectionRequester)
    } else {
        Modifier
    }

    // The whole screen remains one existing destination, but selecting a named task must land on
    // the named capability rather than at an unrelated card several screens above it.
    LaunchedEffect(initialSection) {
        if (initialSection != SystemSection.OVERVIEW) {
            // The requester is attached during layout, one frame after this effect is scheduled.
            // Waiting for that frame makes the target deterministic on both a device and the
            // Robolectric Compose host instead of racing initial composition.
            withFrameNanos { }
            initialSectionRequester.bringIntoView()
        }
    }

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

        Box(
            modifier = sectionModifier(SystemSection.PROVIDER)
                .fillMaxWidth()
                .testTag("system_section_provider")
        ) {
            AiProviderSettingsCard(
                settings = aiProviderSettings,
                onSave = onSaveAiProviderSettings,
                onClearApiKey = onClearAiProviderApiKey
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        // Device Specs Card
        SystemMetricCard(
            title = "Device Information",
            icon = Icons.Default.PhoneAndroid,
            details = listOf(
                "Verb Version" to "${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})",
                // Which installed app this is. Debug and Play builds carry their own application
                // ids and therefore their own private storage -- their own projects, runtime and
                // agent logins. Someone who signs in, installs a differently-suffixed build and
                // finds themselves signed out has not lost anything; they are in a different app,
                // and this is the line that says so.
                "Package" to BuildConfig.APPLICATION_ID,
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

        Box(
            modifier = sectionModifier(SystemSection.WORKING_WORLD)
                .fillMaxWidth()
                .testTag("system_section_working_world")
        ) {
            WorldArchiveCard(
                archiveName = worldArchiveName,
                message = worldArchiveMessage,
                onSaveToDownloads = onSaveWorldToDownloads,
                onPickArchive = onPickWorldArchive
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        Box(
            modifier = sectionModifier(SystemSection.CONTINUITY)
                .fillMaxWidth()
                .testTag("system_section_continuity")
        ) {
            ContinuityCard(
                message = continuityMessage,
                previewReady = continuityPreviewReady,
                importedSessions = importedContinuitySessions,
                onExport = onExportContinuity,
                onPick = onPickContinuity,
                onApply = onApplyContinuity
            )
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

        Box(
            modifier = sectionModifier(SystemSection.RUNTIMES)
                .fillMaxWidth()
                .testTag("system_section_runtimes")
        ) {
            RuntimeProfilesCard(
                reports = runtimeProfileReports,
                installingProfile = installingRuntimeProfile,
                message = runtimeInstallMessage,
                onInstall = onInstallRuntimeProfile
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        Box(
            modifier = sectionModifier(SystemSection.AGENT_RUNTIME)
                .fillMaxWidth()
                .testTag("system_section_agent_runtime")
        ) {
            AgentRuntimeCard(
                status = agentRuntimeStatus,
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
                onCheckCompatibility = onCheckAgentRuntime,
                onReturnToVerb = onReturnToVerbRuntime
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        UsbDebuggingDiagnosticCard()

        Spacer(modifier = Modifier.height(24.dp))
    }
}

/** Evidence moves; processes, current state, transcripts, credentials and authority do not. */
@Composable
internal fun ContinuityCard(
    message: String?,
    previewReady: Boolean,
    importedSessions: Int,
    onExport: () -> Unit,
    onPick: () -> Unit,
    onApply: () -> Unit
) {
    Card(
        modifier = Modifier.fillMaxWidth().testTag("card_continuity"),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text("Mobile ↔ desktop continuity", style = MaterialTheme.typography.titleSmall)
            Text(
                "Move structural session evidence with a user-owned .vcont file. Imported state is " +
                    "dated history, never proof that a process or conversation can resume here.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp)
            )
            Text(
                "$importedSessions imported session record${if (importedSessions == 1) "" else "s"}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 8.dp).testTag("continuity_imported_count")
            )
            message?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(top = 6.dp).testTag("continuity_message")
                )
            }
            Spacer(modifier = Modifier.height(10.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = onExport, modifier = Modifier.testTag("btn_continuity_export")) {
                    Text("Export evidence")
                }
                OutlinedButton(onClick = onPick, modifier = Modifier.testTag("btn_continuity_preview")) {
                    Text("Preview import")
                }
            }
            if (previewReady) {
                Button(
                    onClick = onApply,
                    modifier = Modifier.padding(top = 8.dp).testTag("btn_continuity_apply")
                ) { Text("Apply read-only import") }
            }
        }
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

/**
 * Getting a world archive on and off the device.
 *
 * The archive itself is made in the terminal, by a command the user runs, because it contains their
 * agents' logins and it should be obvious that it was created. This card only moves the result:
 * out to Downloads, where an uninstall cannot reach it, and back in for `verb import` to inspect.
 */
@Composable
internal fun WorldArchiveCard(
    archiveName: String?,
    message: String?,
    onSaveToDownloads: () -> Unit,
    onPickArchive: () -> Unit
) {
    Card(
        modifier = Modifier.fillMaxWidth().testTag("card_world_archive"),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text("Working world", style = MaterialTheme.typography.titleSmall)
            Text(
                "Your agent logins, keys and session records. An uninstall deletes them; a saved " +
                    "archive survives it.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp)
            )
            Text(
                "In the terminal:  verb export ~/world.vbak",
                style = MaterialTheme.typography.bodySmall,
                fontFamily = FontFamily.Monospace,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 8.dp)
            )

            Text(
                archiveName?.let { "Ready to save: $it" }
                    ?: "No archive yet — run the command above first.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier
                    .padding(top = 8.dp)
                    .testTag("world_archive_state")
            )

            message?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier
                        .padding(top = 6.dp)
                        .testTag("world_archive_message")
                )
            }

            Spacer(modifier = Modifier.height(10.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = onSaveToDownloads,
                    enabled = archiveName != null,
                    modifier = Modifier.testTag("btn_world_save")
                ) { Text("Save to Downloads") }

                OutlinedButton(
                    onClick = onPickArchive,
                    modifier = Modifier.testTag("btn_world_restore")
                ) { Text("Bring an archive in") }
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

            // Weighted, with a real spacer between the two columns.
            //
            // `SpaceBetween` alone only separates these while the value is short enough to leave
            // free space in the row. A monospace value like the shell process path wraps, takes
            // every remaining pixel, and leaves SpaceBetween nothing to distribute -- which on a
            // Vivo I2202 rendered as "Runtime EnvironmentVerb / Android PTY Adapter" and
            // "Root AccessNo Root (Unprivileged Safe User)", with four of six rows running their
            // label straight into their value.
            details.forEach { (label, value) ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 4.dp)
                ) {
                    Text(
                        text = label,
                        fontSize = 13.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(0.42f)
                    )
                    Spacer(modifier = Modifier.width(12.dp))
                    Text(
                        text = value,
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Medium,
                        fontFamily = FontFamily.Monospace,
                        color = MaterialTheme.colorScheme.onSurface,
                        textAlign = TextAlign.End,
                        modifier = Modifier.weight(0.58f)
                    )
                }
            }
        }
    }
}

private fun String.capitalize(): String {
    return this.replaceFirstChar { if (it.isLowerCase()) it.titlecase() else it.toString() }
}
