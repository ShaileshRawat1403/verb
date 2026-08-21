package com.example

import android.os.Bundle
import android.content.Intent
import android.net.Uri
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.LocalActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
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
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.example.verb.ui.AgentsScreen
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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Edge-to-edge makes the IME insets dispatch reliably so keyboard visibility can be
        // tracked from the decor view instead of guessing from Compose's isImeVisible read.
        enableEdgeToEdge()
        ViewCompat.setOnApplyWindowInsetsListener(window.decorView) { _, insets ->
            val imeBottom = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
            viewModel.setKeyboardVisible(imeBottom > 0)
            insets
        }

        setContent {
            VerbTheme {
                VerbAppContent(viewModel = viewModel)
            }
        }
    }
}

@Composable
fun VerbAppContent(viewModel: VerbViewModel) {
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
    val isKeyboardVisible by viewModel.isKeyboardVisible.collectAsStateWithLifecycle()

    val terminalOutput by viewModel.terminalRuntime.terminalOutput.collectAsStateWithLifecycle()
    val isSessionActive by viewModel.terminalRuntime.isSessionActive.collectAsStateWithLifecycle()
    val terminalSessionState by viewModel.terminalRuntime.sessionState.collectAsStateWithLifecycle()
    val terminalAiExplanation by viewModel.terminalAiExplanation.collectAsStateWithLifecycle()
    val isTerminalAiExplaining by viewModel.isTerminalAiExplaining.collectAsStateWithLifecycle()
    val terminalBootstrapState by viewModel.terminalBootstrapState.collectAsStateWithLifecycle()
    val runtimeProfileReports by viewModel.runtimeProfileReports.collectAsStateWithLifecycle()
    val installingRuntimeProfile by viewModel.runtimeInstallingProfile.collectAsStateWithLifecycle()
    val runtimeInstallMessage by viewModel.runtimeInstallMessage.collectAsStateWithLifecycle()
    val agentRuntimeStatus by viewModel.agentRuntimeStatus.collectAsStateWithLifecycle()
    val agentKeyStatus by viewModel.agentKeyStatus.collectAsStateWithLifecycle()
    val claudeSession by viewModel.claudeSession.collectAsStateWithLifecycle()
    val agentSignInStates by viewModel.agentSignInStates.collectAsStateWithLifecycle()
    val agentRuntimeImporting by viewModel.agentRuntimeImporting.collectAsStateWithLifecycle()
    val agentRuntimeMessage by viewModel.agentRuntimeMessage.collectAsStateWithLifecycle()
    val projects by viewModel.projects.collectAsStateWithLifecycle()
    val selectedProject by viewModel.selectedProject.collectAsStateWithLifecycle()

    var agentArchiveUri by remember { mutableStateOf<Uri?>(null) }
    var agentChecksumUri by remember { mutableStateOf<Uri?>(null) }
    var agentManifestUri by remember { mutableStateOf<Uri?>(null) }
    val agentArchivePicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) {
        agentArchiveUri = it
    }
    val agentChecksumPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) {
        agentChecksumUri = it
    }
    val agentManifestPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) {
        agentManifestUri = it
    }

    // System back at the root tab exits; otherwise it retraces visited tabs. Screen-level
    // BackHandlers (sheets, dialogs, IME) register deeper in the tree and win first.
    val activity = LocalActivity.current
    BackHandler {
        if (activity == null) return@BackHandler
        if (!viewModel.navigateBack()) {
            activity.finish()
        }
    }

    // Lightweight feedback channel for one-shot events (failed link opens, clipboard copies)
    // that don't warrant a dialog but should still be visible to the person acting on them.
    val snackbarHostState = remember { SnackbarHostState() }

    // A tap on a URL in the terminal canvas surfaces it here for the browser. Launching from the
    // host activity also keeps the terminal session alive behind the browser.
    val terminalUrlToOpen by viewModel.terminalRuntime.urlToOpen.collectAsStateWithLifecycle()
    LaunchedEffect(terminalUrlToOpen) {
        val url = terminalUrlToOpen
        if (url != null) {
            try {
                activity?.startActivity(
                    Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                )
            } catch (e: Exception) {
                android.util.Log.e("Verb", "Could not open terminal URL $url", e)
                snackbarHostState.showSnackbar("Couldn't open link")
            }
            viewModel.terminalRuntime.consumeUrlToOpen()
        }
    }

    // Confirms a terminal selection landed on the system clipboard, since there is otherwise no
    // visible feedback for that action.
    val clipboardCopyEvent by viewModel.terminalRuntime.clipboardCopyEvent.collectAsStateWithLifecycle()
    LaunchedEffect(clipboardCopyEvent) {
        val message = clipboardCopyEvent
        if (message != null) {
            snackbarHostState.showSnackbar(message)
            viewModel.terminalRuntime.consumeClipboardCopyEvent()
        }
    }

    // Terminal session color shown as a status dot on the Terminal tab.
    val terminalStatusColor = when (terminalSessionState) {
        com.example.verb.terminal.TerminalSessionState.RUNNING -> Color(0xFF22C55E)
        com.example.verb.terminal.TerminalSessionState.STARTING,
        com.example.verb.terminal.TerminalSessionState.STOPPING -> Color(0xFFEAB308)
        com.example.verb.terminal.TerminalSessionState.EXITED -> Color(0xFF64748B)
        else -> Color(0xFFEF4444)
    }

    Scaffold(
        modifier = Modifier.fillMaxSize(),
        snackbarHost = { SnackbarHost(snackbarHostState) },
        bottomBar = {
            // A terminal (or any text-entry surface) needs the limited portrait space above the
            // system keyboard. Removing navigation while the IME is visible keeps the active
            // command field docked directly above it rather than marooned mid-screen.
            if (!isKeyboardVisible || activeTab != VerbTab.TERMINAL) NavigationBar(
                modifier = Modifier
                    .windowInsetsPadding(WindowInsets.navigationBars)
                    .testTag("verb_bottom_navigation")
            ) {
                NavigationBarItem(
                    selected = activeTab == VerbTab.AGENTS,
                    onClick = { viewModel.selectTab(VerbTab.AGENTS) },
                    icon = {
                        Icon(
                            imageVector = Icons.Default.AutoAwesome,
                            contentDescription = "Agents",
                            modifier = Modifier.size(22.dp)
                        )
                    },
                    label = { Text("Agents") },
                    modifier = Modifier.testTag("tab_agents")
                )

                NavigationBarItem(
                    selected = activeTab == VerbTab.ASK,
                    onClick = { viewModel.selectTab(VerbTab.ASK) },
                    icon = {
                        TabIconWithDot(
                            icon = Icons.AutoMirrored.Filled.Chat,
                            contentDescription = "Ask",
                            dotColor = if (confirmationPending != null) Color(0xFFF59E0B) else null
                        )
                    },
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
                    icon = {
                        TabIconWithDot(
                            icon = Icons.Default.Terminal,
                            contentDescription = "Terminal",
                            dotColor = terminalStatusColor
                        )
                    },
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
                .imePadding()
        ) {
            when (activeTab) {
                VerbTab.AGENTS -> AgentsScreen(
                    reports = runtimeProfileReports,
                    keyStatus = agentKeyStatus,
                    signInStates = agentSignInStates,
                    onLaunch = viewModel::launchAgent,
                    onInstall = viewModel::installRuntimeProfile,
                    onEditKeys = viewModel::editAgentKeys,
                    installingProfile = installingRuntimeProfile,
                    message = runtimeInstallMessage,
                    claudeSession = claudeSession,
                    onResumeClaudeSession = viewModel::resumeClaudeSession,
                    onStartNewClaudeSession = viewModel::startNewClaudeSession
                )

                VerbTab.ASK -> AskScreen(
                    queryInput = queryInput,
                    isExecuting = isExecuting,
                    currentResult = currentResult,
                    historyList = historyList,
                    confirmationPending = confirmationPending,
                    isKeyboardVisible = isKeyboardVisible,
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
                    isKeyboardVisible = isKeyboardVisible,
                    onPromptChange = viewModel::updateAssistantInput,
                    onSubmitPrompt = viewModel::submitAssistantPrompt,
                    onOpenProviderSettings = { viewModel.selectTab(VerbTab.SYSTEM) }
                )

                VerbTab.SYSTEM -> SystemScreen(
                    isTerminalSessionActive = isSessionActive,
                    terminalEnvironment = viewModel.terminalRuntime.environment,
                    aiProviderSettings = aiProviderSettings,
                    onSaveAiProviderSettings = viewModel::saveAiProviderSettings,
                    onClearAiProviderApiKey = viewModel::clearAiProviderApiKey,
                    onOpenTerminal = viewModel::openTerminal,
                    distributionName = if (BuildConfig.FULL_CLI) "Full CLI (direct distribution)" else "Play-safe system shell",
                    runtimeProfileReports = runtimeProfileReports,
                    installingRuntimeProfile = installingRuntimeProfile,
                    runtimeInstallMessage = runtimeInstallMessage,
                    onInstallRuntimeProfile = viewModel::installRuntimeProfile,
                    agentRuntimeStatus = agentRuntimeStatus,
                    agentRuntimeImporting = agentRuntimeImporting,
                    agentRuntimeMessage = agentRuntimeMessage,
                    agentArchiveName = agentArchiveUri?.lastPathSegment,
                    agentChecksumName = agentChecksumUri?.lastPathSegment,
                    agentManifestName = agentManifestUri?.lastPathSegment,
                    onPickAgentArchive = { agentArchivePicker.launch(arrayOf("application/gzip", "application/octet-stream", "*/*")) },
                    onPickAgentChecksum = { agentChecksumPicker.launch(arrayOf("text/plain", "*/*")) },
                    onPickAgentManifest = { agentManifestPicker.launch(arrayOf("text/plain", "*/*")) },
                    onImportAgentRuntime = {
                        val archive = agentArchiveUri
                        val checksum = agentChecksumUri
                        val manifest = agentManifestUri
                        if (archive != null && checksum != null && manifest != null) {
                            viewModel.importAgentRuntime(archive, checksum, manifest)
                        }
                    },
                    onOpenAgentRuntime = viewModel::openAgentRuntime,
                    onCheckAgentRuntime = viewModel::checkAgentRuntimeCompatibility,
                    onReturnToVerbRuntime = viewModel::returnToVerbRuntime
                )

                VerbTab.TERMINAL -> TerminalScreen(
                    terminalOutput = terminalOutput,
                    terminalRuntime = viewModel.terminalRuntime,
                    sessionState = terminalSessionState,
                    bootstrapState = terminalBootstrapState,
                    isKeyboardVisible = isKeyboardVisible,
                    onRetryBootstrap = viewModel::retryTermuxBootstrap,
                    onSendCommand = viewModel::sendTerminalCommand,
                    onSendKey = viewModel.terminalRuntime::sendControlKey,
                    onSendText = viewModel.terminalRuntime::sendText,
                    onCommandExecuted = viewModel::recordTerminalCommand,
                    onClearTerminal = viewModel.terminalRuntime::clearBuffer,
                    onInspectText = viewModel::inspectSemanticText,
                    onSubmitIntent = viewModel::submitIntent,
                    aiExplanation = terminalAiExplanation,
                    isAiExplaining = isTerminalAiExplaining,
                    onExplainOutput = viewModel::explainTerminalOutput,
                    onDismissAiExplanation = viewModel::dismissTerminalAiExplanation,
                    projects = projects,
                    selectedProject = selectedProject,
                    onCreateProject = viewModel::createProject,
                    onSelectProject = viewModel::selectProject
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

@Composable
private fun TabIconWithDot(
    icon: ImageVector,
    contentDescription: String?,
    dotColor: Color?
) {
    Box {
        Icon(icon, contentDescription)
        if (dotColor != null) {
            Box(
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .padding(top = 1.dp)
                    .size(7.dp)
                    .background(dotColor, CircleShape)
            )
        }
    }
}
