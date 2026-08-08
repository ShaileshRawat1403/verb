package com.example.verb.terminal

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.ClipboardManager
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
fun MobileTerminalKeyboard(
    onSendKey: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    val clipboardManager: ClipboardManager = LocalClipboardManager.current
    val scrollState = rememberScrollState()

    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surfaceVariant,
        tonalElevation = 4.dp
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(scrollState)
                .padding(horizontal = 8.dp, vertical = 6.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            KeyButton(label = "ESC", testTag = "key_esc") { onSendKey("ESC") }
            KeyButton(label = "CTRL+C", testTag = "key_ctrl_c") { onSendKey("CTRL_C") }
            KeyButton(label = "TAB", testTag = "key_tab") { onSendKey("TAB") }
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
            KeyButton(label = "_", testTag = "key_underscore") { onSendKey("_") }
            KeyButton(label = "\\", testTag = "key_backslash") { onSendKey("\\") }
        }
    }
}

@Composable
private fun KeyButton(
    label: String,
    testTag: String,
    onClick: () -> Unit
) {
    OutlinedButton(
        onClick = onClick,
        modifier = Modifier
            .height(36.dp)
            .testTag(testTag),
        contentPadding = ButtonDefaults.ContentPadding,
        colors = ButtonDefaults.outlinedButtonColors(
            containerColor = MaterialTheme.colorScheme.surface,
            contentColor = MaterialTheme.colorScheme.onSurface
        )
    ) {
        Text(
            text = label,
            fontSize = 12.sp,
            fontWeight = FontWeight.SemiBold,
            fontFamily = FontFamily.Monospace
        )
    }
}
