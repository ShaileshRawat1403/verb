package com.example

import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Chat
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.Dns
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.example.verb.ui.AskScreen
import com.example.verb.ui.AssistantScreen
import com.example.verb.ui.SemanticLensSheet
import com.example.verb.ui.SystemScreen
import com.example.verb.ui.TerminalScreen
import com.example.verb.ui.theme.VerbTheme
import com.example.verb.viewmodel.VerbTab
import com.example.verb.viewmodel.VerbViewModel

class MainActivity : ComponentActivity() {

    private val viewModel: VerbViewModel by viewModels()
    private var selectedRuntimeZip: Uri? = null
    private lateinit var runtimeChecksumPicker: ActivityResultLauncher<Array<String>>

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        runtimeChecksumPicker = registerForActivityResult(ActivityResultContracts.OpenDocument()) { checksumUri ->
            val zipUri = selectedRuntimeZip
            selectedRuntimeZip = null
            if (zipUri != null && checksumUri != null) viewModel.importRuntime(zipUri, checksumUri)
        }
        val runtimeZipPicker = registerForActivityResult(ActivityResultContracts.OpenDocument()) { zipUri ->
            if (zipUri != null) {
                selectedRuntimeZip = zipUri
                runtimeChecksumPicker.launch(arrayOf("text/plain", "application/octet-stream", "*/*"))
            }
        }

        setContent {
            VerbTheme {
                VerbAppContent(viewModel = viewModel, onImportRuntime = {
                    runtimeZipPicker.launch(arrayOf("application/zip", "application/octet-stream", "*/*"))
                })
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun VerbAppContent(viewModel: VerbViewModel, onImportRuntime: () -> Unit = {}) {
    val activeTab by viewModel.activeTab.collectAsStateWithLifecycle()
    val queryInput by viewModel.queryInput.collectAsStateWithLifecycle()
    val isExecuting by viewModel.isExecuting.collectAsStateWithLifecycle()
    val currentResult by viewModel.currentActionResult.collectAsStateWithLifecycle()
    val historyList by viewModel.historyList.collectAsStateWithLifecycle()
    val confirmationPending by viewModel.confirmationPendingResult.collectAsStateWithLifecycle()
    val semanticEntity by viewModel.activeSemanticEntity.collectAsStateWithLifecycle()
    val aiProviderSettings by viewModel.aiProviderSettings.collectAsStateWithLifecycle()
    val assistantInput by viewModel.assistantInput.collectAsStateWithLifecycle()
    val assistantState by viewModel.assistantState.collectAsStateWithLifecycle()
    val isImeVisible = WindowInsets.isImeVisible

    val terminalOutput by viewModel.terminalRuntime.terminalOutput.collectAsStateWithLifecycle()
    val isSessionActive by viewModel.terminalRuntime.isSessionActive.collectAsStateWithLifecycle()
    val terminalEnvironment by viewModel.terminalEnvironment.collectAsStateWithLifecycle()
    val runtimeImportState by viewModel.runtimeImportState.collectAsStateWithLifecycle()

    Scaffold(
        modifier = Modifier.fillMaxSize(),
        bottomBar = {
            // A terminal (or any text-entry surface) needs the limited portrait space above the
            // system keyboard. Removing navigation while the IME is visible keeps the active
            // command field docked directly above it rather than marooned mid-screen.
            if (!isImeVisible || activeTab != VerbTab.TERMINAL) NavigationBar(
                modifier = Modifier
                    .windowInsetsPadding(WindowInsets.navigationBars)
                    .testTag("verb_bottom_navigation")
            ) {
                NavigationBarItem(
                    selected = activeTab == VerbTab.ASK,
                    onClick = { viewModel.selectTab(VerbTab.ASK) },
                    icon = { Icon(Icons.AutoMirrored.Filled.Chat, contentDescription = "Ask") },
                    label = { Text("Ask") },
                    modifier = Modifier.testTag("tab_ask")
                )

                NavigationBarItem(
                    selected = activeTab == VerbTab.ASSISTANT,
                    onClick = viewModel::openAssistant,
                    icon = { Icon(Icons.Default.AutoAwesome, contentDescription = "Assistant") },
                    label = { Text("Assistant") },
                    modifier = Modifier.testTag("tab_assistant")
                )

                NavigationBarItem(
                    selected = activeTab == VerbTab.SYSTEM,
                    onClick = { viewModel.selectTab(VerbTab.SYSTEM) },
                    icon = { Icon(Icons.Default.Dns, contentDescription = "System") },
                    label = { Text("System") },
                    modifier = Modifier.testTag("tab_system")
                )

                NavigationBarItem(
                    selected = activeTab == VerbTab.TERMINAL,
                    onClick = { viewModel.selectTab(VerbTab.TERMINAL) },
                    icon = { Icon(Icons.Default.Terminal, contentDescription = "Terminal") },
                    label = { Text("Terminal") },
                    modifier = Modifier.testTag("tab_terminal")
                )
            }
        }
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
        ) {
            when (activeTab) {
                VerbTab.ASK -> AskScreen(
                    queryInput = queryInput,
                    isExecuting = isExecuting,
                    currentResult = currentResult,
                    historyList = historyList,
                    confirmationPending = confirmationPending,
                    onQueryChange = viewModel::updateQueryInput,
                    onSubmitQuery = viewModel::submitQuery,
                    onSubmitIntent = viewModel::submitIntent,
                    onConfirmAction = viewModel::confirmPendingAction,
                    onDismissConfirmation = viewModel::dismissConfirmation,
                    onOpenTerminal = viewModel::openTerminal,
                    onInspectText = viewModel::inspectSemanticText
                )

                VerbTab.ASSISTANT -> AssistantScreen(
                    providerSettings = aiProviderSettings,
                    prompt = assistantInput,
                    state = assistantState,
                    onPromptChange = viewModel::updateAssistantInput,
                    onSubmitPrompt = viewModel::submitAssistantPrompt,
                    onOpenProviderSettings = { viewModel.selectTab(VerbTab.SYSTEM) }
                )

                VerbTab.SYSTEM -> SystemScreen(
                    isTerminalSessionActive = isSessionActive,
                    terminalEnvironment = terminalEnvironment,
                    runtimeImportState = runtimeImportState,
                    onImportRuntime = onImportRuntime,
                    aiProviderSettings = aiProviderSettings,
                    onSaveAiProviderSettings = viewModel::saveAiProviderSettings,
                    onClearAiProviderApiKey = viewModel::clearAiProviderApiKey
                )

                VerbTab.TERMINAL -> TerminalScreen(
                    terminalOutput = terminalOutput,
                    terminalRuntime = viewModel.terminalRuntime,
                    onSendCommand = viewModel.terminalRuntime::sendCommand,
                    onSendKey = viewModel.terminalRuntime::sendControlKey,
                    onClearTerminal = viewModel.terminalRuntime::clearBuffer,
                    onInspectText = viewModel::inspectSemanticText,
                    onSubmitIntent = viewModel::submitIntent
                )
            }

            // Contextual Semantic Lens Bottom Sheet
            if (semanticEntity != null) {
                SemanticLensSheet(
                    entity = semanticEntity!!,
                    onDismiss = viewModel::closeSemanticLens,
                    onExecuteSuggestedAction = viewModel::submitQuery,
                    onExecuteSuggestedIntent = viewModel::submitIntent
                )
            }
        }
    }
}
