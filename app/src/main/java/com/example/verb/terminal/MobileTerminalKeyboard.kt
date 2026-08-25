package com.example.verb.terminal

import android.content.Context
import android.view.HapticFeedbackConstants
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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalTextStyle
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
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.edit

val DEFAULT_QUICK_KEYS = listOf("/", "|", "~", "-", "_", "\\", ":", ";", "&", "#")

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun MobileTerminalKeyboard(
    onSendKey: (String) -> Unit,
    onSendCommand: (String) -> Unit,
    onSendText: (String) -> Unit,
    terminalOutput: String,
    isKeyboardVisible: Boolean = false,
    onInspectOutput: (String) -> Unit,
    onCommandExecuted: (String) -> Unit = {},
    inputFocusRequester: FocusRequester? = null,
    /** False while a deliberate Verb surface owns input in front of the mounted terminal. */
    enabled: Boolean = true,
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
    // Survives rotation and process recreation: a user who opened the panel should not have to
    // reopen it because the screen turned.
    var keysExpanded by rememberSaveable { mutableStateOf(false) }
    // Saveable, not merely remembered: switching to Agents and back used to drop this while the
    // characters were still sitting on the shell line, after which the field and the line disagreed
    // about what had been typed and every edit was computed against the wrong text.
    var terminalInput by rememberSaveable { mutableStateOf("") }
    
    val scrollState1 = rememberScrollState()
    val scrollState2 = rememberScrollState()

    // Characters are forwarded to the PTY as they are typed so they echo at the shell prompt on
    // the terminal canvas instead of being trapped in the field. The field keeps a copy as an
    // editable buffer; submitting only sends a newline because the text is already live.
    fun handleInputChange(new: String) {
        val old = terminalInput
        when {
            new.length > old.length && new.startsWith(old) -> {
                onSendText(new.substring(old.length))
            }
            old.length > new.length && old.startsWith(new) -> {
                repeat(old.length - new.length) { onSendKey("BACKSPACE") }
            }
            new.isNotEmpty() -> {
                // Mid-line edit: clear the echoed line, then re-type it to resync the shell.
                repeat(old.length) { onSendKey("BACKSPACE") }
                onSendText(new)
            }
            else -> {
                // Full clear.
                repeat(old.length) { onSendKey("BACKSPACE") }
            }
        }
        terminalInput = new
    }

    // The line belongs to whatever program owns the PTY -- a shell prompt, Claude's composer,
    // Codex's -- and this field only mirrors what was typed through it. So a backspace with nothing
    // left to mirror is still a real backspace: it is the only way to delete text the field did not
    // put there, which is exactly the case after a resumed agent restores its own input, or after
    // the mirror and the line have drifted apart for any other reason.
    fun deleteOneCharacter() {
        if (terminalInput.isEmpty()) {
            onSendKey("BACKSPACE")
        } else {
            handleInputChange(terminalInput.dropLast(1))
        }
    }

    fun submitTerminalInput() {
        // The command is already on the shell line (live echo), so Enter just completes it.
        // An empty submission is a real Enter key, needed for interactive terminal programs.
        if (terminalInput.isEmpty()) {
            onSendText("\r")
        } else {
            // onSendCommand carries no text (already echoed); the real text goes to the caller only
            // for the current in-memory execution boundary. It is never durably recorded.
            onCommandExecuted(terminalInput)
            onSendCommand("")
        }
        terminalInput = ""
    }

    Surface(
        modifier = modifier
            .fillMaxWidth(),
        color = Color(0xFF161820),
        tonalElevation = 4.dp
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(start = 10.dp, end = 4.dp, top = 4.dp, bottom = 2.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                OutlinedTextField(
                    value = terminalInput,
                    onValueChange = ::handleInputChange,
                    modifier = Modifier
                        .weight(1f)
                        .heightIn(min = 46.dp)
                        .let { base ->
                            inputFocusRequester?.let { base.focusRequester(it) } ?: base
                        }
                        // An empty field reports no change when the IME sends backspace, so without
                        // this the key silently does nothing at the exact moment a person is trying
                        // to clear a line they can see on screen.
                        .onPreviewKeyEvent { event ->
                            if (event.type == KeyEventType.KeyDown &&
                                event.key == Key.Backspace &&
                                terminalInput.isEmpty()
                            ) {
                                onSendKey("BACKSPACE")
                                true
                            } else {
                                false
                            }
                        }
                        .testTag("terminal_input_field"),
                    placeholder = {
                        Text(
                            "$ type a command",
                            color = Color(0xFF94A3B8),
                            fontSize = 14.sp,
                            fontFamily = FontFamily.Monospace
                        )
                    },
                    textStyle = LocalTextStyle.current.copy(
                        fontSize = 14.sp,
                        fontFamily = FontFamily.Monospace
                    ),
                    singleLine = true,
                    enabled = enabled,
                    shape = RoundedCornerShape(10.dp),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                    keyboardActions = KeyboardActions(onSend = { submitTerminalInput() }),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = Color.White,
                        unfocusedTextColor = Color.White,
                        focusedBorderColor = Color(0xFF6366F1),
                        unfocusedBorderColor = Color(0xFF3B4252)
                    )
                )
                IconButton(
                    onClick = ::submitTerminalInput,
                    enabled = enabled,
                    modifier = Modifier.size(44.dp).testTag("terminal_input_submit")
                ) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.Send,
                        contentDescription = "Run typed terminal input",
                        tint = Color(0xFF818CF8)
                    )
                }
                // One control for the whole auxiliary panel. Collapsed is the default because the
                // dock used to occupy roughly 40% of the screen at rest, which is space the
                // terminal output needs far more than a permanently visible symbol row does.
                IconButton(
                    onClick = { keysExpanded = !keysExpanded },
                    modifier = Modifier.size(36.dp).testTag("btn_toggle_key_panel")
                ) {
                    Icon(
                        imageVector = if (keysExpanded) Icons.Default.KeyboardArrowDown else Icons.Default.KeyboardArrowUp,
                        contentDescription = if (keysExpanded) "Hide extra keys" else "Show extra keys",
                        tint = Color(0xFF94A3B8),
                        modifier = Modifier.size(20.dp)
                    )
                }
            }

            // The keys worth permanent space, and no more: history, completion, interrupt, escape,
            // the CTRL modifier and paste. Seven fit without scrolling on a normal phone, which is
            // the point -- a resting row you have to scroll is a row you stop using. SHIFT, the
            // arrows and the symbol keys live one tap away in the panel above. It stays
            // mounted whether or not the IME is up: history recall, tab completion, interrupt and
            // ESC are wanted *while* typing, and hiding them behind the soft keyboard -- which is
            // what the previous layout did -- removed them at exactly the moment they were needed.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(scrollState1)
                    .fadingHorizontalEdges(20.dp)
                    .padding(horizontal = 8.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(5.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                KeyButton(label = "▲", testTag = "key_up") { onSendKey("UP") }
                KeyButton(label = "▼", testTag = "key_down") { onSendKey("DOWN") }
                KeyButton(label = "TAB", testTag = "key_tab") {
                    onSendKey(if (shiftActive) "SHIFT_TAB" else "TAB")
                    shiftActive = false
                }
                // Deleting is not an auxiliary key. Whatever owns the line -- a shell, an agent's
                // composer -- deleting from it is as basic as typing into it, and the soft
                // keyboard's own backspace only reaches text this field is mirroring.
                KeyButton(label = "DEL", testTag = "key_backspace") { deleteOneCharacter() }
                KeyButton(label = "^C", testTag = "key_essential_ctrl_c", isAccent = true) {
                    onSendKey("CTRL_C")
                }
                KeyButton(label = "ESC", testTag = "key_esc") { onSendKey("ESC") }
                KeyButton(label = "CTRL", testTag = "key_ctrl", isAccent = ctrlActive) {
                    ctrlActive = !ctrlActive
                    shiftActive = false
                }
                KeyButton(label = "PASTE", testTag = "key_paste") {
                    // PASTE action is routed to TermuxTerminalRuntimeAdapter.
                    onSendKey("PASTE")
                }
            }

            // Arming CTRL is a request for the key that follows it, so the combinations appear
            // regardless of whether the panel is expanded, and disappear again once one is sent.
            if (ctrlActive) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .background(Color(0xFF222630))
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                    horizontalArrangement = Arrangement.spacedBy(5.dp),
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
            }

            if (keysExpanded) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(scrollState2)
                        .fadingHorizontalEdges(20.dp)
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                    horizontalArrangement = Arrangement.spacedBy(5.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    KeyButton(label = "SHIFT", testTag = "key_shift", isAccent = shiftActive) {
                        shiftActive = !shiftActive
                        ctrlActive = false
                    }
                    KeyButton(label = "◄", testTag = "key_left") { onSendKey("LEFT") }
                    KeyButton(label = "►", testTag = "key_right") { onSendKey("RIGHT") }
                    quickKeys.forEach { keyStr ->
                        KeyButton(label = keyStr, testTag = "key_quick_$keyStr") {
                            onSendKey(keyStr)
                        }
                    }
                    IconButton(
                        onClick = { isSheetOpen = true },
                        modifier = Modifier.size(32.dp).testTag("btn_edit_quick_keys")
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

/**
 * A terminal key.
 *
 * Deliberately not an [OutlinedButton]: Material's button enforces a 58dp minimum width, which made
 * a two-character key as wide as a five-character one and meant only five keys fitted across a
 * normal phone -- CTRL and PASTE were cut off the edge of the resting row, which is the same as not
 * shipping them. Sizing to content instead fits the whole row without scrolling, and the taller
 * 38dp target is easier to hit than the 34dp it replaces despite the row being narrower overall.
 */
@Composable
private fun KeyButton(
    label: String,
    testTag: String,
    isAccent: Boolean = false,
    onClick: () -> Unit
) {
    // Terminal keys are pressed by thumb while the eyes are on the output, not on the finger.
    // The tick is the only acknowledgement that a key registered -- without it every tap feels
    // like a guess, and a missed CTRL is felt minutes later as a hung process.
    val view = LocalView.current
    Surface(
        shape = RoundedCornerShape(9.dp),
        color = if (isAccent) Color(0xFF6366F1) else Color(0xFF222630),
        modifier = Modifier
            .height(38.dp)
            .clickable {
                view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
                onClick()
            }
            .testTag(testTag)
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 10.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = label,
                color = if (isAccent) Color.White else Color(0xFFE2E8F0),
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold,
                fontFamily = FontFamily.Monospace
            )
        }
    }
}

/**
 * Fades the visible left and right edges of a horizontally scrolling key row.
 *
 * The essential-keys row is wider than most phones; where it ran off-screen there was previously
 * no signal at all that more keys existed past the edge -- PASTE was not "hidden", it was
 * *unimaginable*. Applied after [androidx.compose.foundation.horizontalScroll] so the gradient
 * tracks the viewport rather than the scrolled content, and drawn offscreen with DstIn so the
 * fade multiplies the row's own alpha instead of punching a hole through what sits behind it.
 */
private fun Modifier.fadingHorizontalEdges(edgeWidth: Dp): Modifier =
    graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
        .drawWithContent {
            drawContent()
            val width = edgeWidth.toPx()
            drawRect(
                brush = Brush.horizontalGradient(
                    colors = listOf(Color.Black, Color.Transparent),
                    startX = 0f,
                    endX = width
                ),
                blendMode = BlendMode.DstIn
            )
            drawRect(
                brush = Brush.horizontalGradient(
                    colors = listOf(Color.Transparent, Color.Black),
                    startX = size.width - width,
                    endX = size.width
                ),
                blendMode = BlendMode.DstIn
            )
        }
