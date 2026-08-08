package com.example.verb.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.CleaningServices
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Info
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
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
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.text.TextRange
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
    modifier: Modifier = Modifier
) {
    var commandInput by remember { mutableStateOf("") }
    var showNaturalLanguageSheet by remember { mutableStateOf(false) }
    val scrollState = rememberScrollState()

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
        // Terminal Top Header Bar matching user's design diagram
        Surface(
            modifier = Modifier.fillMaxWidth(),
            color = Color(0xFF161820)
        ) {
            Column(modifier = Modifier.fillMaxWidth()) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 14.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    // Title and connection indicator
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Text(
                            text = "Verb",
                            fontSize = 16.sp,
                            fontWeight = FontWeight.Bold,
                            color = Color.White
                        )

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
                        // Natural Language Assistant Trigger
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

                // Current Working Directory Sub-bar
                Surface(
                    modifier = Modifier.fillMaxWidth(),
                    color = Color(0xFF101216)
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 14.dp, vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Icon(
                            imageVector = Icons.Default.Folder,
                            contentDescription = null,
                            tint = Color(0xFF38BDF8),
                            modifier = Modifier.size(14.dp)
                        )
                        Spacer(modifier = Modifier.width(6.dp))
                        Text(
                            text = terminalRuntime?.currentWorkingDirectory() ?: "~/projects/verb",
                            fontSize = 11.sp,
                            fontFamily = FontFamily.Monospace,
                            color = Color(0xFF64748B)
                        )
                    }
                }
            }
        }

        // Real Terminal Canvas View boundary
        val termuxAdapter = terminalRuntime as? TermuxTerminalRuntimeAdapter
        if (termuxAdapter != null) {
            AndroidView(
                factory = { ctx ->
                    termuxAdapter.terminalView ?: TerminalView(ctx).also {
                        it.viewClient = termuxAdapter
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

        // Contextual Touch Control Strip
        MobileTerminalKeyboard(
            onSendKey = onSendKey,
            onSendCommand = onSendCommand,
            terminalOutput = terminalOutput,
            onInspectOutput = onInspectText
        )

        // Command Entry Box with Embedded Verb Trigger
        Surface(
            modifier = Modifier.fillMaxWidth(),
            color = Color(0xFF161820)
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                // Verb NL Button
                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = Color(0xFF6366F1),
                    modifier = Modifier
                        .clickable { showNaturalLanguageSheet = true }
                        .testTag("verb_nl_prompt_button")
                ) {
                    Icon(
                        imageVector = Icons.Default.AutoAwesome,
                        contentDescription = "Ask Verb",
                        tint = Color.White,
                        modifier = Modifier
                            .padding(8.dp)
                            .size(16.dp)
                    )
                }

                Spacer(modifier = Modifier.width(6.dp))

                Text(
                    text = "$ ",
                    fontFamily = FontFamily.Monospace,
                    fontSize = 16.sp,
                    color = Color(0xFF38BDF8)
                )

                OutlinedTextField(
                    value = commandInput,
                    onValueChange = { commandInput = it },
                    modifier = Modifier
                        .weight(1f)
                        .testTag("terminal_input_field"),
                    placeholder = {
                        Text(
                            "Type command or ask Verb...",
                            fontFamily = FontFamily.Monospace,
                            fontSize = 12.sp,
                            color = Color(0xFF64748B)
                        )
                    },
                    singleLine = true,
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = Color(0xFF6366F1),
                        unfocusedBorderColor = Color(0xFF334155),
                        focusedTextColor = Color.White,
                        unfocusedTextColor = Color.White
                    )
                )

                IconButton(
                    onClick = {
                        if (commandInput.isNotBlank()) {
                            onSendCommand(commandInput)
                            commandInput = ""
                        }
                    },
                    modifier = Modifier.testTag("terminal_send_button")
                ) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.Send,
                        contentDescription = "Send Command",
                        tint = Color(0xFF6366F1)
                    )
                }
            }
        }
    }

    // Natural Language Sheet Modal
    if (showNaturalLanguageSheet) {
        VerbNaturalLanguageSheet(
            onDismiss = { showNaturalLanguageSheet = false },
            onExecuteCommand = { cmd ->
                onSendCommand(cmd)
            }
        )
    }
}
