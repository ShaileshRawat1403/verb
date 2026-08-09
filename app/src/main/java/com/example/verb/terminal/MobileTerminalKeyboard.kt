package com.example.verb.terminal

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
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
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.edit

val DEFAULT_QUICK_KEYS = listOf("/", "|", "~", "-", "_", "\\", ":", ";", "&", "#")

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun MobileTerminalKeyboard(
    onSendKey: (String) -> Unit,
    onSendCommand: (String) -> Unit,
    terminalOutput: String,
    onInspectOutput: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    val clipboardManager = LocalClipboardManager.current
    val context = LocalContext.current
    val prefs = remember { context.getSharedPreferences("verb_quick_keys", Context.MODE_PRIVATE) }
    
    var quickKeys by remember { 
        mutableStateOf(
            prefs.getString("keys", null)?.split(",") ?: DEFAULT_QUICK_KEYS
        ) 
    }
    
    fun saveQuickKeys(keys: List<String>) {
        quickKeys = keys
        prefs.edit { putString("keys", keys.joinToString(",")) }
    }

    var ctrlActive by remember { mutableStateOf(false) }
    var shiftActive by remember { mutableStateOf(false) }
    var isSheetOpen by remember { mutableStateOf(false) }
    
    val scrollState1 = rememberScrollState()
    val scrollState2 = rememberScrollState()

    Surface(
        modifier = modifier.fillMaxWidth(),
        color = Color(0xFF161820),
        tonalElevation = 4.dp
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            if (ctrlActive) {
                // Ctrl combinations row
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .background(Color(0xFF222630))
                        .padding(horizontal = 8.dp, vertical = 6.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    val ctrlKeys = listOf("C", "D", "L", "U", "Z", "A", "R", "W", "K")
                    for (k in ctrlKeys) {
                        KeyButton(label = "^$k", testTag = "key_ctrl_$k", isAccent = true) {
                            onSendKey("CTRL_$k")
                            ctrlActive = false
                        }
                    }
                }
            } else {
                // Quick keys row
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(scrollState2)
                        .padding(horizontal = 8.dp, vertical = 6.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    quickKeys.forEach { keyStr ->
                        KeyButton(label = keyStr, testTag = "key_quick_$keyStr") { 
                            onSendKey(keyStr) 
                        }
                    }
                    IconButton(
                        onClick = { isSheetOpen = true },
                        modifier = Modifier.size(34.dp).testTag("btn_edit_quick_keys")
                    ) {
                        Icon(
                            imageVector = Icons.Default.Settings,
                            contentDescription = "Edit Quick Keys",
                            tint = Color(0xFF94A3B8),
                            modifier = Modifier.size(16.dp)
                        )
                    }
                }
            }
            
            // Core power strip
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(scrollState1)
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                KeyButton(label = "ESC", testTag = "key_esc") { onSendKey("ESC") }
                KeyButton(label = "CTRL", testTag = "key_ctrl", isAccent = ctrlActive) { 
                    ctrlActive = !ctrlActive 
                    shiftActive = false
                }
                KeyButton(label = "SHIFT", testTag = "key_shift", isAccent = shiftActive) {
                    shiftActive = !shiftActive
                    ctrlActive = false
                }
                KeyButton(label = "TAB", testTag = "key_tab") { 
                    onSendKey(if (shiftActive) "SHIFT_TAB" else "TAB")
                    shiftActive = false
                }
                KeyButton(label = "PASTE", testTag = "key_paste") {
                    // PASTE action is routed to TermuxTerminalRuntimeAdapter
                    onSendKey("PASTE")
                }
                KeyButton(label = "▲", testTag = "key_up") { onSendKey("UP") }
                KeyButton(label = "▼", testTag = "key_down") { onSendKey("DOWN") }
                KeyButton(label = "◄", testTag = "key_left") { onSendKey("LEFT") }
                KeyButton(label = "►", testTag = "key_right") { onSendKey("RIGHT") }
            }
        }
    }

    if (isSheetOpen) {
        ModalBottomSheet(
            onDismissRequest = { isSheetOpen = false },
            containerColor = Color(0xFF161820)
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Quick Keys", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 18.sp)
                    IconButton(onClick = { saveQuickKeys(DEFAULT_QUICK_KEYS) }, modifier = Modifier.testTag("btn_reset_quick_keys")) {
                        Icon(Icons.Default.Refresh, "Reset Defaults", tint = Color.White)
                    }
                }

                Spacer(modifier = Modifier.height(16.dp))
                
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    quickKeys.forEachIndexed { index, keyStr ->
                        Surface(
                            shape = RoundedCornerShape(8.dp),
                            color = Color(0xFF222630),
                            modifier = Modifier.testTag("quick_key_edit_$index")
                        ) {
                            Row(
                                modifier = Modifier.padding(start = 12.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(keyStr, color = Color.White, fontFamily = FontFamily.Monospace)
                                Spacer(modifier = Modifier.width(8.dp))
                                IconButton(
                                    onClick = { 
                                        val m = quickKeys.toMutableList()
                                        m.removeAt(index)
                                        saveQuickKeys(m)
                                    },
                                    modifier = Modifier.size(24.dp).testTag("btn_remove_quick_key_$index")
                                ) {
                                    Icon(Icons.Default.Close, "Remove", tint = Color(0xFFEF4444), modifier = Modifier.size(16.dp))
                                }
                            }
                        }
                    }
                }
                
                Spacer(modifier = Modifier.height(24.dp))
                
                var newKey by remember { mutableStateOf("") }
                Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    OutlinedTextField(
                        value = newKey,
                        onValueChange = { newKey = it },
                        modifier = Modifier.weight(1f).testTag("input_new_quick_key"),
                        placeholder = { Text("New symbol...", color = Color(0xFF64748B)) },
                        singleLine = true,
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedTextColor = Color.White,
                            unfocusedTextColor = Color.White,
                            focusedBorderColor = Color(0xFF6366F1)
                        )
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Surface(
                        shape = RoundedCornerShape(8.dp),
                        color = Color(0xFF6366F1),
                        modifier = Modifier.clickable {
                            val k = newKey.trim()
                            if (k.isNotEmpty() && k.length <= 8 && !quickKeys.contains(k)) {
                                val m = quickKeys.toMutableList()
                                m.add(k)
                                saveQuickKeys(m)
                                newKey = ""
                            }
                        }.padding(12.dp).testTag("btn_add_quick_key")
                    ) {
                        Icon(Icons.Default.Add, "Add", tint = Color.White)
                    }
                }
                Spacer(modifier = Modifier.height(24.dp))
            }
        }
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
