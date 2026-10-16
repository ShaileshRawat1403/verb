package com.example.verb.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewModelScope
import com.example.verb.mobile.DesktopBridgeClient
import com.example.verb.mobile.DesktopPairingLink
import com.example.verb.model.ChatMessage
import com.example.verb.model.ChatSender
import com.example.verb.viewmodel.DesktopPhoneViewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.nio.charset.StandardCharsets

/** Dedicated conversational continuation experience for live desktop agent sessions. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DesktopPhoneScreen(
    state: DesktopPhoneViewModel,
    initialLink: String? = null,
    onLinkConsumed: () -> Unit = {}
) {
    DisposableEffect(state) {
        onDispose {
            if (state.token != null) {
                state.connected = false
                state.status = "Continuation paused. Reconnect to resume."
                val activeClient = state.client
                val activeToken = state.token
                if (activeClient != null && activeToken != null) {
                    state.viewModelScope.launch(Dispatchers.IO) {
                        runCatching { activeClient.disconnect(activeToken) }
                    }
                }
            }
        }
    }

    LaunchedEffect(initialLink) {
        if (initialLink != null) {
            if (state.token != null && initialLink != state.pairedLink) {
                val oldClient = state.client
                val oldToken = state.token
                state.connected = false
                state.client = null
                state.token = null
                state.screen = null
                if (oldClient != null && oldToken != null) {
                    state.viewModelScope.launch(Dispatchers.IO) {
                        runCatching { oldClient.disconnect(oldToken) }
                    }
                }
            }
            if (state.token == null) state.linkText = initialLink
            onLinkConsumed()
        }
    }

    fun runAction(block: suspend () -> Unit) {
        if (state.busy) return
        state.busy = true
        state.viewModelScope.launch {
            try {
                block()
            } catch (error: CancellationException) {
                throw error
            } catch (error: Exception) {
                state.status = error.message ?: "Desktop connection failed."
            } finally {
                state.busy = false
            }
        }
    }

    LaunchedEffect(state.client, state.token, state.connected) {
        val activeClient = state.client ?: return@LaunchedEffect
        val activeToken = state.token ?: return@LaunchedEffect
        if (!state.connected) return@LaunchedEffect
        while (isActive) {
            try {
                val latest = withContext(Dispatchers.IO) { activeClient.snapshot(activeToken) }
                state.screen = latest
                state.agent = latest.agent
                state.agentState = latest.agentState
                state.canSendPrompt = latest.canSendPrompt
                if (latest.messages.isNotEmpty()) {
                    state.messages = latest.messages
                }
                state.status = "Live · ${if (latest.controller == "phone") "Phone controls input" else "Desktop controls input"}"
            } catch (error: Exception) {
                if (error is CancellationException) throw error
                state.connected = false
                state.status = "Connection interrupted: ${error.message ?: "desktop unavailable"}"
                break
            }
            delay(500)
        }
    }

    if (state.token == null) {
        // Pairing Screen
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Spacer(modifier = Modifier.height(24.dp))
            Box(
                modifier = Modifier
                    .size(64.dp)
                    .clip(CircleShape)
                    .background(MaterialTheme.colorScheme.primaryContainer),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = Icons.Default.Link,
                    contentDescription = null,
                    modifier = Modifier.size(32.dp),
                    tint = MaterialTheme.colorScheme.onPrimaryContainer
                )
            }
            Text("Verb Mobile Continuation", style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.Bold)
            Text(
                "Continue a live agent session running on your MacBook. Your files, environment, and credentials stay securely on the desktop.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            OutlinedTextField(
                value = state.linkText,
                onValueChange = { state.linkText = it },
                modifier = Modifier.fillMaxWidth(),
                label = { Text("Pairing link or QR code") },
                placeholder = { Text("verb://pair#host=...") },
                minLines = 3
            )

            Button(
                onClick = {
                    runAction {
                        val parsed = DesktopPairingLink.parse(state.linkText)
                        val next = DesktopBridgeClient(parsed)
                        val paired = withContext(Dispatchers.IO) { next.pair() }
                        state.client = next
                        state.token = paired
                        state.connected = true
                        state.pairedLink = state.linkText.trim()
                        state.linkText = ""
                        state.status = "Paired. Connecting to agent session…"
                    }
                },
                enabled = !state.busy && state.linkText.isNotBlank(),
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Connect to Session")
            }

            Text(
                "Make sure both devices are on the same Wi-Fi network.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    } else {
        // Paired Conversational Screen
        val currentScreen = state.screen
        val isPhoneController = currentScreen?.controller == "phone"
        val agentDisplayName = state.agent?.replaceFirstChar { it.uppercase() } ?: "Agent"
        val listState = rememberLazyListState()

        // Auto-scroll on new message
        LaunchedEffect(state.messages.size) {
            if (state.messages.isNotEmpty()) {
                listState.animateScrollToItem(state.messages.size - 1)
            }
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.background)
        ) {
            // Header Bar
            Surface(
                modifier = Modifier.fillMaxWidth(),
                color = MaterialTheme.colorScheme.surfaceContainer,
                shadowElevation = 2.dp
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 10.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Box(
                            modifier = Modifier
                                .size(10.dp)
                                .clip(CircleShape)
                                .background(if (state.connected) Color(0xFF10B981) else MaterialTheme.colorScheme.error)
                        )
                        Column {
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                Text(agentDisplayName, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
                                Surface(
                                    shape = RoundedCornerShape(4.dp),
                                    color = if (isPhoneController) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh
                                ) {
                                    Text(
                                        if (isPhoneController) "Phone Control" else "Desktop Control",
                                        modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp),
                                        style = MaterialTheme.typography.labelSmall,
                                        color = if (isPhoneController) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                }
                            }
                            Text(
                                when (state.agentState) {
                                    "working" -> "Claude is working…"
                                    "awaiting_approval" -> "Waiting for approval on desktop"
                                    "waiting" -> "Ready for instruction"
                                    else -> state.status
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = when (state.agentState) {
                                    "working" -> MaterialTheme.colorScheme.primary
                                    "awaiting_approval" -> Color(0xFFF59E0B)
                                    else -> MaterialTheme.colorScheme.onSurfaceVariant
                                }
                            )
                        }
                    }

                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        IconButton(
                            onClick = { state.showTerminalInspector = true }
                        ) {
                            Icon(Icons.Default.Terminal, contentDescription = "Terminal Inspector")
                        }
                        IconButton(
                            onClick = {
                                val activeClient = state.client
                                val activeToken = state.token
                                if (activeClient != null && activeToken != null) {
                                    runAction {
                                        withContext(Dispatchers.IO) { activeClient.disconnect(activeToken) }
                                        state.connected = false
                                        state.status = "Disconnected."
                                    }
                                }
                            }
                        ) {
                            Icon(Icons.Default.Close, contentDescription = "Disconnect")
                        }
                    }
                }
            }

            // Connection warning banner if disconnected
            if (!state.connected) {
                Surface(
                    modifier = Modifier.fillMaxWidth(),
                    color = MaterialTheme.colorScheme.errorContainer
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            "Disconnected from desktop session",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onErrorContainer
                        )
                        Button(
                            onClick = {
                                val activeClient = state.client ?: return@Button
                                val activeToken = state.token ?: return@Button
                                runAction {
                                    withContext(Dispatchers.IO) { activeClient.reconnect(activeToken) }
                                    state.connected = true
                                    state.status = "Reconnected."
                                }
                            },
                            enabled = !state.busy
                        ) {
                            Text("Reconnect")
                        }
                    }
                }
            }

            // Awaiting approval card
            if (state.agentState == "awaiting_approval") {
                Card(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.tertiaryContainer)
                ) {
                    Row(
                        modifier = Modifier.padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp)
                    ) {
                        Icon(
                            Icons.Default.Warning,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onTertiaryContainer
                        )
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                "Action awaiting confirmation",
                                style = MaterialTheme.typography.titleSmall,
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onTertiaryContainer
                            )
                            Text(
                                "Claude is asking for approval on your MacBook. Complete the prompt on desktop or inspect the terminal.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onTertiaryContainer
                            )
                        }
                        OutlinedButton(onClick = { state.showTerminalInspector = true }) {
                            Text("Inspect")
                        }
                    }
                }
            }

            // Chat Messages Feed
            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
            ) {
                if (state.messages.isEmpty()) {
                    Column(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(32.dp),
                        verticalArrangement = Arrangement.Center,
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        Icon(
                            Icons.Default.Computer,
                            contentDescription = null,
                            modifier = Modifier.size(48.dp),
                            tint = MaterialTheme.colorScheme.primary
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                        Text(
                            "Continuing ${agentDisplayName} Session",
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.Bold
                        )
                        Spacer(modifier = Modifier.height(6.dp))
                        Text(
                            "Waiting for session messages… Say hello or send an instruction below.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                } else {
                    LazyColumn(
                        state = listState,
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(horizontal = 16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp)
                    ) {
                        item { Spacer(modifier = Modifier.height(8.dp)) }
                        items(state.messages, key = { it.id }) { msg ->
                            val isUser = msg.sender == ChatSender.USER
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = if (isUser) Arrangement.End else Arrangement.Start
                            ) {
                                Card(
                                    modifier = Modifier.widthIn(max = 320.dp),
                                    shape = if (isUser) {
                                        RoundedCornerShape(16.dp, 16.dp, 4.dp, 16.dp)
                                    } else {
                                        RoundedCornerShape(16.dp, 16.dp, 16.dp, 4.dp)
                                    },
                                    colors = CardDefaults.cardColors(
                                        containerColor = if (isUser) {
                                            MaterialTheme.colorScheme.primaryContainer
                                        } else {
                                            MaterialTheme.colorScheme.surfaceContainerHigh
                                        }
                                    )
                                ) {
                                    SelectionContainer {
                                        Text(
                                            text = msg.text,
                                            modifier = Modifier.padding(14.dp),
                                            style = MaterialTheme.typography.bodyMedium,
                                            color = if (isUser) {
                                                MaterialTheme.colorScheme.onPrimaryContainer
                                            } else {
                                                MaterialTheme.colorScheme.onSurface
                                            }
                                        )
                                    }
                                }
                            }
                        }

                        if (state.agentState == "working") {
                            item {
                                Row(
                                    modifier = Modifier.padding(start = 8.dp, top = 4.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                                ) {
                                    CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                                    Text(
                                        "${agentDisplayName} is thinking…",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.primary
                                    )
                                }
                            }
                        }

                        item { Spacer(modifier = Modifier.height(8.dp)) }
                    }
                }
            }

            // Input Composer Bar
            Surface(
                modifier = Modifier
                    .fillMaxWidth()
                    .imePadding(),
                color = MaterialTheme.colorScheme.surfaceContainerLow,
                shadowElevation = 8.dp
            ) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(12.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    if (!isPhoneController) {
                        Button(
                            onClick = {
                                val activeClient = state.client ?: return@Button
                                val activeToken = state.token ?: return@Button
                                runAction {
                                    withContext(Dispatchers.IO) { activeClient.take(activeToken) }
                                    state.status = "Phone controls input."
                                }
                            },
                            modifier = Modifier.fillMaxWidth(),
                            enabled = !state.busy
                        ) {
                            Text("Take Control to Message")
                        }
                    } else {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            OutlinedTextField(
                                value = state.command,
                                onValueChange = { state.command = it },
                                modifier = Modifier.weight(1f),
                                placeholder = {
                                    Text(
                                        when (state.agentState) {
                                            "working" -> "Claude is working…"
                                            "awaiting_approval" -> "Approval required on desktop"
                                            else -> "Message ${agentDisplayName}…"
                                        }
                                    )
                                },
                                enabled = state.canSendPrompt && !state.busy && state.agentState != "working" && state.agentState != "awaiting_approval",
                                maxLines = 4
                            )

                            if (state.agentState == "working") {
                                IconButton(
                                    onClick = {
                                        val activeClient = state.client ?: return@IconButton
                                        val activeToken = state.token ?: return@IconButton
                                        runAction {
                                            withContext(Dispatchers.IO) {
                                                activeClient.input(activeToken, byteArrayOf(3))
                                            }
                                        }
                                    }
                                ) {
                                    Icon(Icons.Default.Stop, contentDescription = "Interrupt", tint = MaterialTheme.colorScheme.error)
                                }
                            } else {
                                IconButton(
                                    onClick = {
                                        val activeClient = state.client ?: return@IconButton
                                        val activeToken = state.token ?: return@IconButton
                                        val prompt = state.command.trim()
                                        if (prompt.isNotBlank()) {
                                            val bytes = (prompt + "\n").toByteArray(StandardCharsets.UTF_8)
                                            runAction {
                                                withContext(Dispatchers.IO) {
                                                    activeClient.input(activeToken, bytes)
                                                }
                                                state.command = ""
                                            }
                                        }
                                    },
                                    enabled = state.canSendPrompt && !state.busy && state.command.isNotBlank() && state.agentState != "working" && state.agentState != "awaiting_approval"
                                ) {
                                    Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Send", tint = MaterialTheme.colorScheme.primary)
                                }
                            }
                        }
                    }
                }
            }
        }

        // Terminal Inspector Bottom Sheet
        if (state.showTerminalInspector) {
            val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
            ModalBottomSheet(
                onDismissRequest = { state.showTerminalInspector = false },
                sheetState = sheetState
            ) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(16.dp)
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text("Terminal Inspector", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
                        IconButton(onClick = { state.showTerminalInspector = false }) {
                            Icon(Icons.Default.Close, contentDescription = "Close Inspector")
                        }
                    }
                    Text(
                        "Live PTY mirror on MacBook. Use this to view detailed command output or interactive CLI prompts.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(modifier = Modifier.height(10.dp))
                    Surface(
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = 260.dp, max = 400.dp),
                        color = MaterialTheme.colorScheme.surfaceContainerLowest,
                        shape = RoundedCornerShape(8.dp)
                    ) {
                        SelectionContainer {
                            Text(
                                currentScreen?.text ?: "Terminal output unavailable.",
                                modifier = Modifier
                                    .horizontalScroll(rememberScrollState())
                                    .verticalScroll(rememberScrollState())
                                    .padding(12.dp),
                                fontFamily = FontFamily.Monospace,
                                style = MaterialTheme.typography.bodySmall,
                                softWrap = false
                            )
                        }
                    }
                    Spacer(modifier = Modifier.height(16.dp))
                }
            }
        }
    }
}
