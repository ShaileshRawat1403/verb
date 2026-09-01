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
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import android.app.Activity
import androidx.core.view.WindowCompat
import androidx.compose.ui.platform.LocalView
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.net.toUri
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.example.verb.ui.AgentsScreen
import com.example.verb.ui.AskVerbScreen
import com.example.verb.ui.RunsSheet
import com.example.verb.ui.SemanticLensSheet
import com.example.verb.ui.SystemScreen
import com.example.verb.ui.SystemSection
import com.example.verb.ui.TerminalDiagnosticsSheet
import com.example.verb.ui.TerminalScreen
import com.example.verb.ui.VerbFirstActionRow
import com.example.verb.ui.VerbSheet
import com.example.verb.ui.theme.VerbTheme
import com.example.verb.ui.verbFirstAction
import com.example.verb.ui.AppearanceScreen
import com.example.verb.viewmodel.VerbSurface
import com.example.verb.viewmodel.VerbTask
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
            // The stored choice overrides the device; SYSTEM means keep following it.
            val themeChoice by viewModel.themeChoice.collectAsStateWithLifecycle()
    val terminalSessionIds by viewModel.terminalSessionIds.collectAsStateWithLifecycle()
    val activeTerminalSessionId by viewModel.activeTerminalSessionId.collectAsStateWithLifecycle()
            val dark = themeChoice.resolveDark(isSystemInDarkTheme())

            // The system bars draw over the app, so their icon colour has to follow the resolved
            // theme rather than the device's. Without this, choosing Light on a dark-mode phone
            // left white status icons on a white bar -- the clock and battery simply vanished.
            val view = LocalView.current
            LaunchedEffect(dark) {
                val window = (view.context as Activity).window
                WindowCompat.getInsetsController(window, view).apply {
                    isAppearanceLightStatusBars = !dark
                    isAppearanceLightNavigationBars = !dark
                }
            }

            VerbTheme(darkTheme = dark) {
                VerbAppContent(viewModel = viewModel)
            }
        }
    }
}

/**
 * One terminal-first workspace, with everything else reached by name.
 *
 * The five-tab `NavigationBar` this replaces organised the app by subsystem -- Agents, Ask,
 * Assistant, System, Terminal -- which is the shape `docs/UX_FOUNDATION.md` prohibits outright
 * ("there is no permanent navigation chrome... one key opens everything") and `docs/BACKLOG.md` D0
 * restates for agents specifically. It also cost the product its own thesis: the terminal, which is
 * supposed to be most of the screen most of the time, was one fifth of a tab strip and listed last.
 *
 * Two structural consequences worth naming, because they are the point rather than side effects:
 *
 * * [TerminalScreen] is composed unconditionally and is never swapped out. Under the tab model,
 *   navigating away disposed it; the PTY survived in [com.example.verb.session.VerbTerminalSessionHolder]
 *   but the view did not. Now Verb's surfaces are drawn *over* a workspace that stays mounted, so the
 *   hosted session keeps the keyboard whenever nothing is deliberately open in front of it.
 * * Nothing appears here on Verb's initiative. The sheet and every task are the user's move.
 */
@Composable
fun VerbAppContent(viewModel: VerbViewModel) {
    val surface by viewModel.surface.collectAsStateWithLifecycle()
    val queryInput by viewModel.queryInput.collectAsStateWithLifecycle()
    val isExecuting by viewModel.isExecuting.collectAsStateWithLifecycle()
    val currentResult by viewModel.currentActionResult.collectAsStateWithLifecycle()
    val historyList by viewModel.historyList.collectAsStateWithLifecycle()
    val confirmationPending by viewModel.confirmationPendingResult.collectAsStateWithLifecycle()
    val semanticEntity by viewModel.activeSemanticEntity.collectAsStateWithLifecycle()
    val aiProviderSettings by viewModel.aiProviderSettings.collectAsStateWithLifecycle()
    val isKeyboardVisible by viewModel.isKeyboardVisible.collectAsStateWithLifecycle()

    val terminalOutput by viewModel.terminalRuntime.terminalOutput.collectAsStateWithLifecycle()
    val isSessionActive by viewModel.terminalRuntime.isSessionActive.collectAsStateWithLifecycle()
    val terminalSessionState by viewModel.terminalRuntime.sessionState.collectAsStateWithLifecycle()
    val terminalAiExplanation by viewModel.terminalAiExplanation.collectAsStateWithLifecycle()
    val isTerminalAiExplaining by viewModel.isTerminalAiExplaining.collectAsStateWithLifecycle()
    val terminalAiEvidence by viewModel.terminalAiEvidence.collectAsStateWithLifecycle()
    val terminalAiThread by viewModel.terminalAiThread.collectAsStateWithLifecycle()
    val assistantStageRequested by viewModel.assistantStageRequested.collectAsStateWithLifecycle()
    val themeChoice by viewModel.themeChoice.collectAsStateWithLifecycle()
    val terminalSessionIds by viewModel.terminalSessionIds.collectAsStateWithLifecycle()
    val activeTerminalSessionId by viewModel.activeTerminalSessionId.collectAsStateWithLifecycle()
    val terminalBootstrapState by viewModel.terminalBootstrapState.collectAsStateWithLifecycle()
    val terminalLaunchNotice by viewModel.terminalLaunchNotice.collectAsStateWithLifecycle()
    val runtimeProfileReports by viewModel.runtimeProfileReports.collectAsStateWithLifecycle()
    val installingRuntimeProfile by viewModel.runtimeInstallingProfile.collectAsStateWithLifecycle()
    val runtimeInstallMessage by viewModel.runtimeInstallMessage.collectAsStateWithLifecycle()
    val agentRuntimeStatus by viewModel.agentRuntimeStatus.collectAsStateWithLifecycle()
    val agentKeyStatus by viewModel.agentKeyStatus.collectAsStateWithLifecycle()
    val agentSessions by viewModel.agentSessions.collectAsStateWithLifecycle()
    val worldArchiveName by viewModel.worldArchiveName.collectAsStateWithLifecycle()
    val worldArchiveMessage by viewModel.worldArchiveMessage.collectAsStateWithLifecycle()
    val continuityMessage by viewModel.continuityMessage.collectAsStateWithLifecycle()
    val continuityPreviewReady by viewModel.continuityPreviewReady.collectAsStateWithLifecycle()
    val importedContinuitySessions by viewModel.importedContinuitySessions.collectAsStateWithLifecycle()
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
    // Bringing a world archive back in is a file the user chooses, not a path Verb guesses at.
    val worldArchivePicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri?.let(viewModel::stageWorldArchive)
    }
    val continuityPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri?.let(viewModel::previewContinuity)
    }
    val agentManifestPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) {
        agentManifestUri = it
    }

    // Back resolves Verb's own surfaces innermost-first -- an open task, then the sheet -- and only
    // exits once the terminal already owns the screen. Screen-level BackHandlers (the terminal's own
    // sheets, dialogs, the IME) register deeper in the tree and win before this one.
    val activity = LocalActivity.current
    BackHandler {
        if (activity == null) return@BackHandler
        if (!viewModel.dismissVerbSurface()) {
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
                    Intent(Intent.ACTION_VIEW, url.toUri()).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
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

    // "Use the shell" is a UI-only preference for this visit: the shell was always usable, so
    // dismissing the offer must not record anything about the session or the agent.
    var firstActionDismissed by rememberSaveable { mutableStateOf(false) }
    val firstAction = verbFirstAction(reports = runtimeProfileReports, sessions = agentSessions)

    Scaffold(
        modifier = Modifier.fillMaxSize(),
        snackbarHost = { SnackbarHost(snackbarHostState) }
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .imePadding()
        ) {
            // The workspace. Always composed, never replaced -- see this function's own note.
            TerminalScreen(
                terminalOutput = terminalOutput,
                terminalRuntime = viewModel.terminalRuntime,
                sessionState = terminalSessionState,
                bootstrapState = terminalBootstrapState,
                isKeyboardVisible = isKeyboardVisible,
                onRetryBootstrap = viewModel::retryTermuxBootstrap,
                onSendCommand = viewModel::sendTerminalCommand,
                onSendKey = viewModel.terminalRuntime::sendControlKey,
                onSendText = viewModel.terminalRuntime::sendText,
                onClearTerminal = viewModel.terminalRuntime::clearBuffer,
                onInspectText = viewModel::inspectSemanticText,
                onSubmitIntent = viewModel::submitIntent,
                onOpenVerb = viewModel::openVerbSheet,
                verbSurfaceOpen = surface != VerbSurface.None,
                terminalLaunchNotice = terminalLaunchNotice,
                onOpenAssistant = viewModel::openAssistant,
                terminalSessionIds = terminalSessionIds,
                activeTerminalSessionId = activeTerminalSessionId,
                agentInTerminal = viewModel::agentInTerminalSession,
                onSwitchTerminalSession = viewModel::activateTerminalSession,
                onOpenTerminalSession = { viewModel.openTerminalSession() },
                canOpenMoreTerminals = terminalSessionIds.size < com.example.verb.session.VerbTerminalSessionHolder.MAX_SESSIONS,
                projects = projects,
                selectedProject = selectedProject,
                onCreateProject = viewModel::createProject,
                onSelectProject = viewModel::selectProject,
                verbFirstAction = if (firstActionDismissed) {
                    null
                } else {
                    {
                        VerbFirstActionRow(
                            action = firstAction,
                            onStart = viewModel::launchAgent,
                            onResume = viewModel::resumeAgentSession,
                            onInstall = { profileId ->
                                // Installing is long and its progress is only legible on the agents
                                // surface, so the user is taken to where the result will appear
                                // rather than left watching a workspace that says nothing.
                                viewModel.openTask(VerbTask.AGENTS)
                                viewModel.installRuntimeProfile(profileId)
                            },
                            onUseShell = { firstActionDismissed = true }
                        )
                    }
                }
            )

            // Everything Verb puts in front of the workspace, and nothing that puts itself there.
            when (val current = surface) {
                VerbSurface.None -> Unit

                VerbSurface.Sheet -> VerbSheet(
                    onDismiss = { viewModel.dismissVerbSurface() },
                    onOpenTask = { task -> viewModel.openTask(task, fromSheet = true) }
                )

                is VerbSurface.Task -> VerbTaskSurface(
                    task = current.task,
                    viewModel = viewModel,
                    queryInput = queryInput,
                    isExecuting = isExecuting,
                    currentResult = currentResult,
                    historyList = historyList,
                    confirmationPending = confirmationPending,
                    aiProviderSettings = aiProviderSettings,
                    terminalAiExplanation = terminalAiExplanation,
                    isTerminalAiExplaining = isTerminalAiExplaining,
                    terminalAiEvidence = terminalAiEvidence,
                    terminalAiThread = terminalAiThread,
                    assistantStageRequested = assistantStageRequested,
                    themeChoice = themeChoice,
                    terminalSessionIds = terminalSessionIds,
                    activeTerminalSessionId = activeTerminalSessionId,
                    isKeyboardVisible = isKeyboardVisible,
                    isSessionActive = isSessionActive,
                    runtimeProfileReports = runtimeProfileReports,
                    agentKeyStatus = agentKeyStatus,
                    agentSignInStates = agentSignInStates,
                    agentSessions = agentSessions,
                    installingRuntimeProfile = installingRuntimeProfile,
                    runtimeInstallMessage = runtimeInstallMessage,
                    agentRuntimeStatus = agentRuntimeStatus,
                    agentRuntimeImporting = agentRuntimeImporting,
                    agentRuntimeMessage = agentRuntimeMessage,
                    agentArchiveName = agentArchiveUri?.lastPathSegment,
                    agentChecksumName = agentChecksumUri?.lastPathSegment,
                    agentManifestName = agentManifestUri?.lastPathSegment,
                    worldArchiveName = worldArchiveName,
                    worldArchiveMessage = worldArchiveMessage,
                    continuityMessage = continuityMessage,
                    continuityPreviewReady = continuityPreviewReady,
                    importedContinuitySessions = importedContinuitySessions,
                    onPickAgentArchive = {
                        agentArchivePicker.launch(
                            arrayOf("application/gzip", "application/octet-stream", "*/*")
                        )
                    },
                    onPickAgentChecksum = { agentChecksumPicker.launch(arrayOf("text/plain", "*/*")) },
                    onPickAgentManifest = { agentManifestPicker.launch(arrayOf("text/plain", "*/*")) },
                    onPickWorldArchive = {
                        worldArchivePicker.launch(arrayOf("application/octet-stream", "*/*"))
                    },
                    onPickContinuity = {
                        continuityPicker.launch(
                            arrayOf("application/x-verb-continuity", "application/json", "*/*")
                        )
                    },
                    onImportAgentRuntime = {
                        val archive = agentArchiveUri
                        val checksum = agentChecksumUri
                        val manifest = agentManifestUri
                        if (archive != null && checksum != null && manifest != null) {
                            viewModel.importAgentRuntime(archive, checksum, manifest)
                        }
                    }
                )
            }

            // Contextual, and the one thing here driven by something the user did in the terminal
            // rather than by a navigation choice: it appears because text was selected.
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

/**
 * One named task, drawn over the workspace.
 *
 * Several tasks share a destination on purpose. "Add a provider key", "Install a runtime", "Import
 * an agent runtime", "Save or restore my world" and "Move a session to another device" all land on
 * [SystemScreen], because what makes a task a task is that a person can *find it by the name they
 * already use* -- not that it owns a screen. Splitting that surface into five is visual work for a
 * later, separate phase; inventing five half-screens now would be a rewrite pretending to be an
 * information-architecture fix.
 */
@Composable
private fun VerbTaskSurface(
    task: VerbTask,
    viewModel: VerbViewModel,
    queryInput: String,
    isExecuting: Boolean,
    currentResult: com.example.verb.model.ActionResult?,
    historyList: List<com.example.verb.model.ActionResult>,
    confirmationPending: com.example.verb.model.ActionResult?,
    aiProviderSettings: com.example.verb.ai.AiProviderSettings,
    terminalAiExplanation: String?,
    isTerminalAiExplaining: Boolean,
    terminalAiEvidence: com.example.verb.terminal.TerminalEvidence?,
    terminalAiThread: List<com.example.verb.terminal.TerminalAiExchange>,
    assistantStageRequested: Boolean,
    themeChoice: com.example.verb.ui.theme.VerbThemeChoice,
    terminalSessionIds: List<String>,
    activeTerminalSessionId: String?,
    isKeyboardVisible: Boolean,
    isSessionActive: Boolean,
    runtimeProfileReports: List<com.example.verb.terminal.RuntimeProfileReport>,
    agentKeyStatus: List<com.example.verb.ui.AgentKeyStatus>,
    agentSignInStates: Map<com.example.verb.terminal.RuntimeProfileId, com.example.verb.terminal.AgentSignInState>,
    agentSessions: Map<com.example.verb.terminal.RuntimeProfileId, com.example.verb.session.VerbSession>,
    installingRuntimeProfile: com.example.verb.terminal.RuntimeProfileId?,
    runtimeInstallMessage: String?,
    agentRuntimeStatus: com.example.verb.terminal.AgentRuntimeStatus,
    agentRuntimeImporting: Boolean,
    agentRuntimeMessage: String?,
    agentArchiveName: String?,
    agentChecksumName: String?,
    agentManifestName: String?,
    worldArchiveName: String?,
    worldArchiveMessage: String?,
    continuityMessage: String?,
    continuityPreviewReady: Boolean,
    importedContinuitySessions: Int,
    onPickAgentArchive: () -> Unit,
    onPickAgentChecksum: () -> Unit,
    onPickAgentManifest: () -> Unit,
    onPickWorldArchive: () -> Unit,
    onPickContinuity: () -> Unit,
    onImportAgentRuntime: () -> Unit
) {
    when (task) {
        // Already a modal sheet, so it needs no opaque backing of its own.
        VerbTask.EVIDENCE -> TerminalDiagnosticsSheet(
            terminalRuntime = viewModel.terminalRuntime,
            onDismiss = { viewModel.dismissVerbSurface() }
        )

        VerbTask.RUNS -> RunsSheet(
            terminalRuntime = viewModel.terminalRuntime,
            onDismiss = { viewModel.dismissVerbSurface() }
        )

        // Full-height destinations. The Surface is what stops the live terminal showing through and
        // what stops a stray tap reaching it.
        else -> Surface(
            modifier = Modifier.fillMaxSize(),
            color = MaterialTheme.colorScheme.background
        ) {
            when (task) {
                VerbTask.ASK_VERB -> AskVerbScreen(
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
                    onInspectText = viewModel::inspectSemanticText,
                    providerSettings = aiProviderSettings,
                    onOpenProviderSettings = { viewModel.openTask(VerbTask.PROVIDER) },
                    aiExplanation = terminalAiExplanation,
                    isAiExplaining = isTerminalAiExplaining,
                    aiEvidence = terminalAiEvidence,
                    aiThread = terminalAiThread,
                    onAskVerbAi = viewModel::askTerminalAi,
                    onExplainEvidence = viewModel::explainTerminalOutput,
                    onClearAiThread = viewModel::clearTerminalAiThread,
                    startOnAssistant = assistantStageRequested,
                    onAssistantStageConsumed = viewModel::assistantStageConsumed,
                    isKeyboardVisible = isKeyboardVisible
                )

                // Two names, one surface: agents and their sessions are the same card, and a person
                // looking for "resume" should not have to know that Verb files it under "agents".
                VerbTask.AGENTS, VerbTask.SESSIONS -> AgentsScreen(
                    reports = runtimeProfileReports,
                    keyStatus = agentKeyStatus,
                    signInStates = agentSignInStates,
                    onLaunch = viewModel::launchAgent,
                    onInstall = viewModel::installRuntimeProfile,
                    onEditKeys = viewModel::editAgentKeys,
                    installingProfile = installingRuntimeProfile,
                    message = runtimeInstallMessage,
                    agentSessions = agentSessions,
                    onResumeSession = viewModel::resumeAgentSession,
                    onStartNewSession = viewModel::startNewAgentSession,
                    terminalSessionIds = terminalSessionIds,
                    activeTerminalSessionId = activeTerminalSessionId,
                    agentInTerminal = viewModel::agentInTerminalSession,
                    canOpenMoreTerminals = terminalSessionIds.size < com.example.verb.session.VerbTerminalSessionHolder.MAX_SESSIONS,
                    onOpenTerminalSession = { viewModel.openTerminalSession() },
                    onSwitchTerminalSession = viewModel::activateTerminalSession,
                    onCloseTerminalSession = { viewModel.closeTerminalSession(it) }
                )

                VerbTask.APPEARANCE -> AppearanceScreen(
                    choice = themeChoice,
                    onChoose = viewModel::setThemeChoice
                )

                else -> {
                    // The archive list is read from disk, and `verb export` writes to that disk
                    // from the terminal, behind Verb's back. Reading it once when the ViewModel was
                    // built meant the card named whichever archive happened to exist at app start
                    // -- so a person who had just made a fresh export was offered an older one to
                    // save, under the name of a file they had already moved on from.
                    LaunchedEffect(Unit) { viewModel.refreshWorldArchive() }
                    SystemScreen(
                        isTerminalSessionActive = isSessionActive,
                        terminalEnvironment = viewModel.terminalRuntime.environment,
                        aiProviderSettings = aiProviderSettings,
                        onSaveAiProviderSettings = viewModel::saveAiProviderSettings,
                        onClearAiProviderApiKey = viewModel::clearAiProviderApiKey,
                        onOpenTerminal = viewModel::openTerminal,
                        distributionName = if (BuildConfig.FULL_CLI) {
                            "Full CLI (direct distribution)"
                        } else {
                            "Play-safe system shell"
                        },
                        runtimeProfileReports = runtimeProfileReports,
                        installingRuntimeProfile = installingRuntimeProfile,
                        runtimeInstallMessage = runtimeInstallMessage,
                        onInstallRuntimeProfile = viewModel::installRuntimeProfile,
                        agentRuntimeStatus = agentRuntimeStatus,
                        agentRuntimeImporting = agentRuntimeImporting,
                        agentRuntimeMessage = agentRuntimeMessage,
                        agentArchiveName = agentArchiveName,
                        agentChecksumName = agentChecksumName,
                        agentManifestName = agentManifestName,
                        onPickAgentArchive = onPickAgentArchive,
                        onPickAgentChecksum = onPickAgentChecksum,
                        onPickAgentManifest = onPickAgentManifest,
                        worldArchiveName = worldArchiveName,
                        worldArchiveMessage = worldArchiveMessage,
                        onSaveWorldToDownloads = viewModel::saveWorldToDownloads,
                        onPickWorldArchive = onPickWorldArchive,
                        continuityMessage = continuityMessage,
                        continuityPreviewReady = continuityPreviewReady,
                        importedContinuitySessions = importedContinuitySessions,
                        onExportContinuity = viewModel::exportContinuity,
                        onPickContinuity = onPickContinuity,
                        onApplyContinuity = viewModel::applyContinuityPreview,
                        onImportAgentRuntime = onImportAgentRuntime,
                        onOpenAgentRuntime = viewModel::openAgentRuntime,
                        onCheckAgentRuntime = viewModel::checkAgentRuntimeCompatibility,
                        onReturnToVerbRuntime = viewModel::returnToVerbRuntime,
                        initialSection = systemSectionFor(task)
                    )
                }
            }
        }
    }
}

/** The searchable name and the first visible System card must describe the same task. */
internal fun systemSectionFor(task: VerbTask): SystemSection = when (task) {
    VerbTask.PROVIDER -> SystemSection.PROVIDER
    VerbTask.WORKING_WORLD -> SystemSection.WORKING_WORLD
    VerbTask.CONTINUITY -> SystemSection.CONTINUITY
    VerbTask.RUNTIMES -> SystemSection.RUNTIMES
    VerbTask.AGENT_RUNTIME -> SystemSection.AGENT_RUNTIME
    VerbTask.SYSTEM -> SystemSection.OVERVIEW
    else -> SystemSection.OVERVIEW
}
