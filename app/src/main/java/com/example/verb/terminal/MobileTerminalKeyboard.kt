package com.example.verb.terminal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.ClipboardManager
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

enum class ControlStripMode {
    SHELL,
    GIT,
    ERROR
}

@Composable
fun MobileTerminalKeyboard(
    onSendKey: (String) -> Unit,
    onSendCommand: (String) -> Unit,
    terminalOutput: String,
    onInspectOutput: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    val clipboardManager: ClipboardManager = LocalClipboardManager.current
    val scrollState = rememberScrollState()

    // Auto-detect context from output
    val detectedMode = remember(terminalOutput) {
        val lower = terminalOutput.lowercase()
        when {
            lower.contains("eaddrinuse") || lower.contains("error:") || lower.contains("permission denied") || lower.contains("fatal:") -> ControlStripMode.ERROR
            lower.contains("git") || lower.contains("branch") || lower.contains("commit") -> ControlStripMode.GIT
            else -> ControlStripMode.SHELL
        }
    }

    var manualModeOverride by remember { mutableStateOf<ControlStripMode?>(null) }
    val activeMode = manualModeOverride ?: detectedMode

    Surface(
        modifier = modifier.fillMaxWidth(),
        color = Color(0xFF161820),
        tonalElevation = 4.dp
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            // Mode Indicator / Selector Header
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 10.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    ModeChip("SHELL", activeMode == ControlStripMode.SHELL) { manualModeOverride = ControlStripMode.SHELL }
                    ModeChip("GIT", activeMode == ControlStripMode.GIT) { manualModeOverride = ControlStripMode.GIT }
                    ModeChip("ERROR", activeMode == ControlStripMode.ERROR) { manualModeOverride = ControlStripMode.ERROR }
                }

                Text(
                    text = when (activeMode) {
                        ControlStripMode.SHELL -> "Shell Controls"
                        ControlStripMode.GIT -> "Git Context"
                        ControlStripMode.ERROR -> "Error Recovery"
                    },
                    fontSize = 10.sp,
                    fontFamily = FontFamily.Monospace,
                    color = Color(0xFF94A3B8)
                )
            }

            // Scrollable Key Strip
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(scrollState)
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                when (activeMode) {
                    ControlStripMode.SHELL -> {
                        KeyButton(label = "ESC", testTag = "key_esc") { onSendKey("ESC") }
                        KeyButton(label = "CTRL+C", testTag = "key_ctrl_c") { onSendKey("CTRL_C") }
                        KeyButton(label = "TAB", testTag = "key_tab") { onSendKey("TAB") }
                        KeyButton(label = "Inspect Selection", testTag = "key_inspect_selection", isAccent = true) {
                            val pasted = clipboardManager.getText()?.text
                            if (!pasted.isNullOrEmpty()) {
                                onInspectOutput(pasted)
                            } else {
                                onInspectOutput(terminalOutput.takeLast(300))
                            }
                        }
                        KeyButton(label = "▲", testTag = "key_up") { onSendKey("UP") }
                        KeyButton(label = "▼", testTag = "key_down") { onSendKey("DOWN") }
                        KeyButton(label = "◄", testTag = "key_left") { onSendKey("LEFT") }
                        KeyButton(label = "►", testTag = "key_right") { onSendKey("RIGHT") }
                        KeyButton(label = "PASTE", testTag = "key_paste") {
                            val pasted = clipboardManager.getText()?.text
                            if (!pasted.isNullOrEmpty()) {
                                onSendKey(pasted)
                            }
                        }
                        KeyButton(label = "/", testTag = "key_slash") { onSendKey("/") }
                        KeyButton(label = "|", testTag = "key_pipe") { onSendKey("|") }
                        KeyButton(label = "~", testTag = "key_tilde") { onSendKey("~") }
                        KeyButton(label = "-", testTag = "key_dash") { onSendKey("-") }
                    }

                    ControlStripMode.GIT -> {
                        KeyButton(label = "Status", testTag = "git_status") { onSendCommand("git status") }
                        KeyButton(label = "Diff", testTag = "git_diff") { onSendCommand("git diff") }
                        KeyButton(label = "Log", testTag = "git_log") { onSendCommand("git log -n 5 --oneline") }
                        KeyButton(label = "Branch", testTag = "git_branch") { onSendCommand("git branch -a") }
                        KeyButton(label = "Pull", testTag = "git_pull") { onSendCommand("git pull") }
                        KeyButton(label = "Push", testTag = "git_push") { onSendCommand("git push") }
                    }

                    ControlStripMode.ERROR -> {
                        KeyButton(label = "Explain", testTag = "err_explain", isAccent = true) {
                            onInspectOutput(terminalOutput.takeLast(500))
                        }
                        KeyButton(label = "Retry", testTag = "err_retry") { onSendKey("UP") }
                        KeyButton(label = "Inspect", testTag = "err_inspect") {
                            onInspectOutput(terminalOutput.takeLast(300))
                        }
                        KeyButton(label = "Clear", testTag = "err_clear") { onSendCommand("clear") }
                    }
                }
            }
        }
    }
}

@Composable
private fun ModeChip(
    label: String,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    Surface(
        shape = RoundedCornerShape(12.dp),
        color = if (isSelected) Color(0xFF6366F1) else Color(0xFF222630),
        modifier = Modifier.clickable { onClick() }
    ) {
        Text(
            text = label,
            fontSize = 10.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = FontFamily.Monospace,
            color = if (isSelected) Color.White else Color(0xFF94A3B8),
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp)
        )
    }
}

@Composable
private fun KeyButton(
    label: String,
    testTag: String,
    isAccent: Boolean = false,
    onClick: () -> Unit
) {
    OutlinedButton(
        onClick = onClick,
        modifier = Modifier
            .height(34.dp)
            .testTag(testTag),
        contentPadding = ButtonDefaults.ContentPadding,
        colors = ButtonDefaults.outlinedButtonColors(
            containerColor = if (isAccent) Color(0xFF6366F1) else Color(0xFF222630),
            contentColor = if (isAccent) Color.White else Color(0xFFE2E8F0)
        )
    ) {
        Text(
            text = label,
            fontSize = 11.sp,
            fontWeight = FontWeight.SemiBold,
            fontFamily = FontFamily.Monospace
        )
    }
}
