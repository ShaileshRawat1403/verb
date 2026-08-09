package com.example.verb.ui

import com.example.verb.model.VerbIntent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.CleaningServices
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import com.example.verb.terminal.MobileTerminalKeyboard
import com.example.verb.terminal.SelectionChangeListener
import com.example.verb.terminal.TerminalRuntimeAdapter
import com.example.verb.terminal.TermuxTerminalRuntimeAdapter
import com.termux.view.TerminalView

@Composable
fun TerminalScreen(
    terminalOutput: String,
    terminalRuntime: TerminalRuntimeAdapter? = null,
    onSendCommand: (String) -> Unit,
    onSendKey: (String) -> Unit,
    onClearTerminal: () -> Unit,
    onInspectText: (String) -> Unit,
    onSubmitIntent: (VerbIntent) -> Unit,
    modifier: Modifier = Modifier
) {
    var showNaturalLanguageSheet by remember { mutableStateOf(false) }
    val scrollState = rememberScrollState()
    val focusManager: FocusManager = LocalFocusManager.current
    val termuxAdapter = terminalRuntime as? TermuxTerminalRuntimeAdapter

    // A Compose text field on another tab may retain IME focus across navigation. Release it as
    // Terminal opens so the explicit terminal input field can claim text focus when the user taps.
    LaunchedEffect(terminalRuntime) {
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
                            shape = RoundedCornerShape(6.dp),
                            color = Color(0xFF222630),
                            modifier = Modifier.clickable { /* Session Selector */ }
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = "local",
                                    fontSize = 12.sp,
                                    fontFamily = FontFamily.Monospace,
                                    color = Color(0xFF94A3B8)
                                )
                                Icon(
                                    imageVector = Icons.Default.ArrowDropDown,
                                    contentDescription = null,
                                    tint = Color(0xFF94A3B8),
                                    modifier = Modifier.size(16.dp)
                                )
                            }
                        }

                        // Active status indicator dot
                        val statusColor = when (terminalRuntime?.sessionState?.value) {
                            com.example.verb.terminal.TerminalSessionState.RUNNING -> Color(0xFF22C55E)
                            com.example.verb.terminal.TerminalSessionState.STARTING -> Color(0xFFEAB308)
                            else -> Color(0xFFEF4444)
                        }
                        Box(
                            modifier = Modifier
                                .size(8.dp)
                                .background(statusColor, CircleShape)
                        )
                    }

                    // Quick Action Buttons
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        IconButton(
                            onClick = onClearTerminal,
                            modifier = Modifier.size(32.dp)
                        ) {
                            Icon(
                                imageVector = Icons.Default.CleaningServices,
                                contentDescription = "Clear terminal",
                                tint = Color(0xFF94A3B8),
                                modifier = Modifier.size(18.dp)
                            )
                        }
                    }
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
            terminalOutput = terminalOutput,
            onInspectOutput = onInspectText
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
}
