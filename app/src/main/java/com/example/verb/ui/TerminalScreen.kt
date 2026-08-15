package com.example.verb.ui

import com.example.verb.model.VerbIntent
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.BugReport
import androidx.compose.material.icons.filled.CleaningServices
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.CreateNewFolder
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Psychology
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.OutlinedTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.focus.FocusManager
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import com.example.verb.terminal.MobileTerminalKeyboard
import com.example.verb.terminal.SelectionChangeListener
import com.example.verb.terminal.TerminalRuntime
import com.example.verb.terminal.TerminalRuntimeAdapter
import com.example.verb.terminal.TermuxBootstrapInstaller
import com.example.verb.terminal.TermuxTerminalRuntimeAdapter
import com.example.verb.project.VerbProject
import com.termux.view.TerminalView

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TerminalScreen(
    terminalOutput: String,
    terminalRuntime: TerminalRuntimeAdapter? = null,
    bootstrapState: TermuxBootstrapInstaller.State = TermuxBootstrapInstaller.State.Ready,
    isKeyboardVisible: Boolean = false,
    onRetryBootstrap: () -> Unit = {},
    onSendCommand: (String) -> Unit,
    onSendKey: (String) -> Unit,
    onSendText: (String) -> Unit,
    onCommandExecuted: (String) -> Unit = {},
    onClearTerminal: () -> Unit,
    onInspectText: (String) -> Unit,
    onSubmitIntent: (VerbIntent) -> Unit,
    aiExplanation: String? = null,
    isAiExplaining: Boolean = false,
    onExplainOutput: () -> Unit = {},
    onDismissAiExplanation: () -> Unit = {},
    projects: List<VerbProject> = emptyList(),
    selectedProject: VerbProject? = null,
    onCreateProject: (String) -> Unit = {},
    onSelectProject: (String) -> Unit = {},
    modifier: Modifier = Modifier
) {
    var showNaturalLanguageSheet by remember { mutableStateOf(false) }
    var showDiagnosticsSheet by remember { mutableStateOf(false) }
    var showFileExplorerSheet by remember { mutableStateOf(false) }
    var showProjectSheet by remember { mutableStateOf(false) }
    var showRunsSheet by remember { mutableStateOf(false) }
    var showOverflowMenu by remember { mutableStateOf(false) }
    val scrollState = rememberScrollState()
    val focusManager: FocusManager = LocalFocusManager.current
    val clipboardManager = LocalClipboardManager.current
    val termuxAdapter = when (terminalRuntime) {
        is TermuxTerminalRuntimeAdapter -> terminalRuntime
        is TerminalRuntime -> terminalRuntime.termuxDelegate
        else -> null
    }
    val sessionState = terminalRuntime?.sessionState?.value

    // A Compose text field on another tab may retain IME focus across navigation. Release it as
    // Terminal opens so the explicit terminal input field can claim text focus when the user taps.
    LaunchedEffect(terminalRuntime) {
        focusManager.clearFocus(force = true)
    }

    // Back gestures resolve overlays top-down, then dismiss the keyboard, before the app-level
    // handler retraces tab history. Enables are mutually exclusive so the innermost surface wins.
    val keyboardController = LocalSoftwareKeyboardController.current
    val anyOverlayOpen = showFileExplorerSheet || showProjectSheet || showDiagnosticsSheet || showNaturalLanguageSheet ||
        showRunsSheet || showOverflowMenu || (aiExplanation != null || isAiExplaining)
    // Overflow menu is the innermost surface (a DropdownMenu can appear over any sheet's trigger),
    // so it takes priority over every other back handler below.
    BackHandler(enabled = showOverflowMenu) { showOverflowMenu = false }
    BackHandler(enabled = !showOverflowMenu && showFileExplorerSheet) { showFileExplorerSheet = false }
    BackHandler(enabled = !showOverflowMenu && !showFileExplorerSheet && showProjectSheet) { showProjectSheet = false }
    BackHandler(
        enabled = !showOverflowMenu && !showFileExplorerSheet && !showProjectSheet && showDiagnosticsSheet
    ) { showDiagnosticsSheet = false }
    BackHandler(
        enabled = !showOverflowMenu && !showFileExplorerSheet && !showProjectSheet && !showDiagnosticsSheet && showRunsSheet
    ) {
        showRunsSheet = false
    }
    BackHandler(
        enabled = !showOverflowMenu && !showFileExplorerSheet && !showProjectSheet && !showDiagnosticsSheet && !showRunsSheet &&
            showNaturalLanguageSheet
    ) {
        showNaturalLanguageSheet = false
    }
    BackHandler(
        enabled = !showOverflowMenu && !showFileExplorerSheet && !showProjectSheet && !showDiagnosticsSheet && !showRunsSheet &&
            !showNaturalLanguageSheet && (aiExplanation != null || isAiExplaining)
    ) { onDismissAiExplanation() }
    BackHandler(enabled = !anyOverlayOpen && isKeyboardVisible) {
        keyboardController?.hide()
        focusManager.clearFocus(force = true)
    }

    // Register SelectionChangeListener with TerminalRuntime for active exact selection monitoring
    DisposableEffect(terminalRuntime, onInspectText) {
        val listener = SelectionChangeListener { _, selectedText ->
            if (selectedText.isNotBlank()) {
                onInspectText(selectedText)
            }
        }
        terminalRuntime?.addSelectionChangeListener(listener)
        onDispose {
            terminalRuntime?.removeSelectionChangeListener(listener)
        }
    }

    // Auto-scroll terminal to bottom when new output arrives
    LaunchedEffect(terminalOutput) {
        scrollState.animateScrollTo(scrollState.maxValue)
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Color(0xFF0D0E12))
    ) {
        // Thin Terminal Header Bar
        Surface(
            modifier = Modifier.fillMaxWidth(),
            color = Color(0xFF161820)
        ) {
            Column(modifier = Modifier.fillMaxWidth()) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 14.dp, vertical = 6.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    // Title and connection indicator
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Surface(
                            shape = RoundedCornerShape(12.dp),
                            color = Color(0xFF6366F1),
                            modifier = Modifier
                                .clickable { showNaturalLanguageSheet = true }
                                .testTag("verb_nl_trigger_top")
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Icon(
                                    imageVector = Icons.Default.AutoAwesome,
                                    contentDescription = null,
                                    tint = Color.White,
                                    modifier = Modifier.size(14.dp)
                                )
                                Spacer(modifier = Modifier.width(4.dp))
                                Text(
                                    text = "Verb",
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.Bold,
                                    color = Color.White
                                )
                            }
                        }
                        Surface(
                            shape = RoundedCornerShape(999.dp),
                            color = Color(0xFF222630),
                            onClick = { showProjectSheet = true },
                            modifier = Modifier.testTag("terminal_project_selector")
                        ) {
                            // Capped and ellipsized -- an untruncated long project id (e.g.
                            // "mobile-kit-30603ae7") was measured on the Vivo I2202 to consume
                            // over a third of the header's width by itself, leaving no room for
                            // the action row on the right (Runs' and the overflow menu's touch
                            // targets both measured 0x0 before this cap was added).
                            Text(
                                text = selectedProject?.id ?: "project",
                                modifier = Modifier
                                    .padding(horizontal = 8.dp, vertical = 4.dp)
                                    .widthIn(max = 56.dp),
                                fontSize = 11.sp,
                                color = Color(0xFFCBD5E1),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                        }

                        // Active status indicator; tap to restart a stopped/failed session
                        val statusColor = when (sessionState) {
                            com.example.verb.terminal.TerminalSessionState.RUNNING -> Color(0xFF22C55E)
                            com.example.verb.terminal.TerminalSessionState.STARTING,
                            com.example.verb.terminal.TerminalSessionState.STOPPING -> Color(0xFFEAB308)
                            com.example.verb.terminal.TerminalSessionState.EXITED -> Color(0xFF94A3B8)
                            else -> Color(0xFFEF4444)
                        }
                        val statusLabel = when (sessionState) {
                            com.example.verb.terminal.TerminalSessionState.RUNNING -> "running"
                            com.example.verb.terminal.TerminalSessionState.STARTING -> "starting"
                            com.example.verb.terminal.TerminalSessionState.STOPPING -> "stopping"
                            com.example.verb.terminal.TerminalSessionState.EXITED -> "exited"
                            com.example.verb.terminal.TerminalSessionState.FAILED -> "failed"
                            null -> "not ready"
                        }
                        Surface(
                            shape = RoundedCornerShape(999.dp),
                            color = Color(0xFF222630),
                            onClick = {
                                if (sessionState != com.example.verb.terminal.TerminalSessionState.RUNNING) {
                                    terminalRuntime?.restartSession()
                                }
                            },
                            modifier = Modifier
                                .padding(start = 2.dp)
                                .testTag("verb_session_status")
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Box(
                                    modifier = Modifier
                                        .size(8.dp)
                                        .background(statusColor, CircleShape)
                                )
                                Spacer(modifier = Modifier.width(6.dp))
                                Text(
                                    text = statusLabel,
                                    fontSize = 11.sp,
                                    color = Color(0xFF94A3B8)
                                )
                            }
                        }
                    }

                    // Quick Action Buttons. Only Runs stays directly visible alongside the
                    // overflow menu -- on-device verification found the project-id pill on the
                    // left already consumes most of this header's width on typical devices, so a
                    // full row of direct icons is not reliably reachable (see PR discussion). Every
                    // other action lives in the overflow menu with a text label, not icon-only.
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        IconButton(
                            onClick = { showRunsSheet = true },
                            modifier = Modifier
                                .size(48.dp)
                                .testTag("btn_terminal_runs")
                        ) {
                            Icon(
                                imageVector = Icons.Default.History,
                                contentDescription = "Command run history",
                                tint = Color(0xFF94A3B8),
                                modifier = Modifier.size(18.dp)
                            )
                        }
                        Box {
                            IconButton(
                                onClick = { showOverflowMenu = true },
                                modifier = Modifier
                                    .size(32.dp)
                                    .testTag("btn_terminal_overflow")
                            ) {
                                Icon(
                                    imageVector = Icons.Default.MoreVert,
                                    contentDescription = "More terminal actions",
                                    tint = Color(0xFF94A3B8),
                                    modifier = Modifier.size(18.dp)
                                )
                            }
                            DropdownMenu(
                                expanded = showOverflowMenu,
                                onDismissRequest = { showOverflowMenu = false },
                                modifier = Modifier.testTag("terminal_overflow_menu")
                            ) {
                                DropdownMenuItem(
                                    text = { Text("Diagnostics") },
                                    leadingIcon = {
                                        Icon(imageVector = Icons.Default.BugReport, contentDescription = null)
                                    },
                                    onClick = {
                                        showOverflowMenu = false
                                        showDiagnosticsSheet = true
                                    },
                                    modifier = Modifier.testTag("btn_diagnostics_overflow")
                                )
                                DropdownMenuItem(
                                    text = { Text("Browse Files") },
                                    leadingIcon = {
                                        Icon(imageVector = Icons.Default.Folder, contentDescription = null)
                                    },
                                    onClick = {
                                        showOverflowMenu = false
                                        showFileExplorerSheet = true
                                    },
                                    modifier = Modifier.testTag("btn_files_overflow")
                                )
                                DropdownMenuItem(
                                    text = { Text("Explain with AI") },
                                    leadingIcon = {
                                        Icon(imageVector = Icons.Default.Psychology, contentDescription = null)
                                    },
                                    enabled = !isAiExplaining,
                                    onClick = {
                                        showOverflowMenu = false
                                        onExplainOutput()
                                    },
                                    modifier = Modifier.testTag("btn_ai_explain_overflow")
                                )
                                DropdownMenuItem(
                                    text = { Text("Restart Session") },
                                    leadingIcon = {
                                        Icon(imageVector = Icons.Default.Refresh, contentDescription = null)
                                    },
                                    enabled = terminalRuntime != null,
                                    onClick = {
                                        showOverflowMenu = false
                                        terminalRuntime?.restartSession()
                                    },
                                    modifier = Modifier.testTag("btn_restart_overflow")
                                )
                                DropdownMenuItem(
                                    text = { Text("Clear Terminal") },
                                    leadingIcon = {
                                        Icon(imageVector = Icons.Default.CleaningServices, contentDescription = null)
                                    },
                                    onClick = {
                                        showOverflowMenu = false
                                        onClearTerminal()
                                    },
                                    modifier = Modifier.testTag("btn_clear_terminal_overflow")
                                )
                            }
                        }
                    }
                }
            }
        }

        // Verb CLI userland install progress / error card
        when (bootstrapState) {
            TermuxBootstrapInstaller.State.Ready,
            TermuxBootstrapInstaller.State.NotStarted -> Unit

            is TermuxBootstrapInstaller.State.Downloading -> {
                val totalMb = if (bootstrapState.totalBytes > 0) bootstrapState.totalBytes / (1024 * 1024) else 0
                val doneMb = bootstrapState.bytesRead / (1024 * 1024)
                BootstrapProgressCard(
                    title = "Installing Verb CLI userland",
                    stepLabel = "Step 1 of 3 · Downloading",
                    subtitle = if (bootstrapState.totalBytes > 0) {
                        "Downloading bootstrap… $doneMb MB / $totalMb MB"
                    } else {
                        "Downloading bootstrap… $doneMb MB"
                    },
                    progress = if (bootstrapState.totalBytes > 0) {
                        bootstrapState.bytesRead.toFloat() / bootstrapState.totalBytes.toFloat()
                    } else {
                        0f
                    }
                )
            }

            TermuxBootstrapInstaller.State.Extracting -> BootstrapProgressCard(
                title = "Installing Verb CLI userland",
                stepLabel = "Step 2 of 3 · Extracting",
                subtitle = "Extracting Verb runtime…",
                progress = null
            )

            TermuxBootstrapInstaller.State.Finalizing -> BootstrapProgressCard(
                title = "Installing Verb CLI userland",
                stepLabel = "Step 3 of 3 · Configuring",
                subtitle = "Configuring Verb runtime…",
                progress = null
            )

            is TermuxBootstrapInstaller.State.Error -> BootstrapErrorCard(
                message = bootstrapState.message,
                onRetry = onRetryBootstrap
            )
        }

        // Stopped-session guidance: typing a command now auto-restarts the session, so tell the
        // user instead of leaving them staring at a dead terminal.
        val bootstrapReady = bootstrapState is TermuxBootstrapInstaller.State.Ready ||
            bootstrapState is TermuxBootstrapInstaller.State.NotStarted
        if (bootstrapReady && (sessionState == com.example.verb.terminal.TerminalSessionState.EXITED ||
                sessionState == com.example.verb.terminal.TerminalSessionState.FAILED)
        ) {
            Surface(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp, vertical = 4.dp)
                    .testTag("session_stopped_banner"),
                shape = RoundedCornerShape(10.dp),
                color = Color(0xFF2A1A1C)
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = if (sessionState == com.example.verb.terminal.TerminalSessionState.FAILED) {
                            "Session failed to start. Tap the status pill or type a command to retry."
                        } else {
                            "Session stopped. Type a command below to start a new one."
                        },
                        fontSize = 12.sp,
                        color = Color(0xFFFCA5A5)
                    )
                }
            }
        }

        // First-run hint so the empty canvas is obviously a terminal you type into.
        if (bootstrapReady && terminalOutput.isBlank() &&
            sessionState == com.example.verb.terminal.TerminalSessionState.RUNNING
        ) {
            Surface(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp, vertical = 4.dp)
                    .testTag("first_run_terminal_hint"),
                shape = RoundedCornerShape(10.dp),
                color = Color(0xFF1A1D27)
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = "Type a command below — try \"git --version\"",
                        fontSize = 12.sp,
                        color = Color(0xFF94A3B8)
                    )
                }
            }
        }

        // Real Terminal Canvas View boundary
        if (termuxAdapter != null) {
            AndroidView(
                factory = { ctx ->
                    termuxAdapter.terminalView ?: TerminalView(ctx, null).also {
                        termuxAdapter.bindTerminalView(it)
                    }
                },
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(12.dp)
                    .testTag("termux_terminal_view")
            )
        } else {
            // Compose selection view fallback for unit tests and headless environments
            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(12.dp)
                    .verticalScroll(scrollState)
            ) {
                SelectionContainer {
                    Text(
                        text = terminalOutput.ifEmpty { "$ " },
                        fontFamily = FontFamily.Monospace,
                        fontSize = 13.sp,
                        color = Color(0xFFE2E8F0),
                        lineHeight = 18.sp,
                        modifier = Modifier
                            .fillMaxWidth()
                            .testTag("terminal_output_text")
                    )
                }
            }
        }

        // Contextual Touch Control Strip for P0.3
        MobileTerminalKeyboard(
            onSendKey = onSendKey,
            onSendCommand = onSendCommand,
            onSendText = onSendText,
            terminalOutput = terminalOutput,
            isKeyboardVisible = isKeyboardVisible,
            onInspectOutput = onInspectText,
            onCommandExecuted = onCommandExecuted
        )
    }

    // Natural Language Sheet Modal
    if (showNaturalLanguageSheet) {
        VerbNaturalLanguageSheet(
            onDismiss = { showNaturalLanguageSheet = false },
            onSubmitIntent = { intent ->
                showNaturalLanguageSheet = false
                onSubmitIntent(intent)
            }
        )
    }

    // Terminal Session Diagnostics Sheet
    if (showDiagnosticsSheet) {
        TerminalDiagnosticsSheet(
            terminalRuntime = terminalRuntime,
            onDismiss = { showDiagnosticsSheet = false }
        )
    }

    // Command Runs Sheet
    if (showRunsSheet) {
        RunsSheet(
            terminalRuntime = terminalRuntime,
            onDismiss = { showRunsSheet = false }
        )
    }

    // File Explorer Sheet
    if (showFileExplorerSheet) {
        ModalBottomSheet(
            onDismissRequest = { showFileExplorerSheet = false },
            containerColor = Color(0xFF12141C),
            contentColor = Color(0xFFE2E8F0),
            modifier = Modifier.testTag("terminal_file_explorer_sheet")
        ) {
            FileExplorerDrawer(
                terminalRuntime = terminalRuntime,
                isDark = true,
                onFileClicked = { path -> clipboardManager.setText(AnnotatedString(path)) }
            )
        }
    }

    if (showProjectSheet) {
        ProjectSheet(
            projects = projects,
            selectedProject = selectedProject,
            onDismiss = { showProjectSheet = false },
            onCreate = { name ->
                onCreateProject(name)
                showProjectSheet = false
            },
            onSelect = { id -> onSelectProject(id); showProjectSheet = false }
        )
    }

    // AI Explanation Sheet
    if (aiExplanation != null || isAiExplaining) {
        ModalBottomSheet(
            onDismissRequest = onDismissAiExplanation,
            containerColor = Color(0xFF12141C),
            contentColor = Color(0xFFE2E8F0),
            modifier = Modifier.testTag("terminal_ai_explanation_sheet")
        ) {
            Column(modifier = Modifier.padding(20.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(
                        imageVector = Icons.Default.Psychology,
                        contentDescription = null,
                        tint = Color(0xFF6366F1),
                        modifier = Modifier.size(22.dp)
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = "Terminal Analysis",
                        fontSize = 17.sp,
                        fontWeight = FontWeight.Bold,
                        color = Color.White
                    )
                }
                Spacer(modifier = Modifier.height(14.dp))
                when {
                    isAiExplaining -> Row(verticalAlignment = Alignment.CenterVertically) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(18.dp),
                            strokeWidth = 2.dp,
                            color = Color(0xFF6366F1)
                        )
                        Spacer(modifier = Modifier.width(10.dp))
                        Text(
                            text = "Analyzing recent terminal output…",
                            fontSize = 13.sp,
                            color = Color(0xFF94A3B8)
                        )
                    }
                    !aiExplanation.isNullOrBlank() -> Text(
                        text = aiExplanation,
                        fontSize = 14.sp,
                        color = Color(0xFFE2E8F0),
                        modifier = Modifier.padding(bottom = 16.dp)
                    )
                    else -> Button(
                        onClick = onExplainOutput,
                        modifier = Modifier.testTag("btn_retry_ai_explain")
                    ) {
                        Text("Retry")
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ProjectSheet(
    projects: List<VerbProject>,
    selectedProject: VerbProject?,
    onDismiss: () -> Unit,
    onCreate: (String) -> Unit,
    onSelect: (String) -> Unit
) {
    var name by remember { mutableStateOf("") }
    fun createProject() {
        if (name.isNotBlank()) {
            onCreate(name)
            name = ""
        }
    }
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = Color(0xFF12141C), contentColor = Color(0xFFE2E8F0)) {
        Column(modifier = Modifier.padding(20.dp)) {
            Text("Project", fontSize = 17.sp, fontWeight = FontWeight.Bold)
            Text("Launches a new terminal session in the selected directory.", fontSize = 12.sp, color = Color(0xFF94A3B8))
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("New project") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { createProject() })
            )
            Button(onClick = ::createProject, enabled = name.isNotBlank(), modifier = Modifier.padding(top = 8.dp)) {
                Icon(Icons.Default.CreateNewFolder, contentDescription = null, modifier = Modifier.size(16.dp))
                Spacer(Modifier.width(6.dp))
                Text("Create")
            }
            projects.forEach { project ->
                Text(
                    text = if (project.id == selectedProject?.id) "${project.id}  active" else project.id,
                    modifier = Modifier.fillMaxWidth().clickable { onSelect(project.id) }.padding(vertical = 12.dp),
                    fontFamily = FontFamily.Monospace,
                    fontSize = 13.sp
                )
            }
        }
    }
}

@Composable
private fun BootstrapProgressCard(
    title: String,
    stepLabel: String,
    subtitle: String,
    progress: Float?
) {
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 14.dp, vertical = 8.dp)
            .testTag("bootstrap_progress_card"),
        shape = RoundedCornerShape(12.dp),
        color = Color(0xFF161820)
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            if (progress == null) {
                CircularProgressIndicator(
                    modifier = Modifier.size(20.dp),
                    strokeWidth = 2.dp,
                    color = Color(0xFF6366F1)
                )
            } else {
                LinearProgressIndicator(
                    progress = { progress },
                    modifier = Modifier.width(48.dp),
                    color = Color(0xFF6366F1),
                    trackColor = Color(0xFF222630),
                    strokeCap = androidx.compose.ui.graphics.StrokeCap.Round
                )
            }
            Spacer(modifier = Modifier.width(12.dp))
            Column {
                Text(
                    text = title,
                    fontSize = 13.sp,
                    fontWeight = FontWeight.Bold,
                    color = Color.White
                )
                Text(
                    text = stepLabel,
                    fontSize = 11.sp,
                    color = Color(0xFF818CF8)
                )
                Spacer(modifier = Modifier.height(2.dp))
                Text(
                    text = subtitle,
                    fontSize = 12.sp,
                    color = Color(0xFF94A3B8)
                )
            }
        }
    }
}

@Composable
private fun BootstrapErrorCard(
    message: String,
    onRetry: () -> Unit
) {
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 14.dp, vertical = 8.dp)
            .testTag("bootstrap_error_card"),
        shape = RoundedCornerShape(12.dp),
        color = Color(0xFF2A1A1C)
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "Verb CLI userland install failed",
                    fontSize = 13.sp,
                    fontWeight = FontWeight.Bold,
                    color = Color(0xFFFCA5A5)
                )
                Spacer(modifier = Modifier.height(2.dp))
                Text(
                    text = message,
                    fontSize = 12.sp,
                    color = Color(0xFFE2E8F0)
                )
                Spacer(modifier = Modifier.height(2.dp))
                Text(
                    text = "Check your internet connection and storage, then try again.",
                    fontSize = 11.sp,
                    color = Color(0xFF94A3B8)
                )
            }
            Spacer(modifier = Modifier.width(12.dp))
            Button(
                onClick = onRetry,
                modifier = Modifier.testTag("btn_retry_bootstrap")
            ) {
                Text("Retry")
            }
        }
    }
}
