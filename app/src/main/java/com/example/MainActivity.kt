package com.example

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Chat
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
        enableEdgeToEdge()

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

    val terminalOutput by viewModel.terminalRuntime.terminalOutput.collectAsStateWithLifecycle()
    val isSessionActive by viewModel.terminalRuntime.isSessionActive.collectAsStateWithLifecycle()

    Scaffold(
        modifier = Modifier.fillMaxSize(),
        bottomBar = {
            NavigationBar(
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

                VerbTab.SYSTEM -> SystemScreen(
                    isTerminalSessionActive = isSessionActive
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
