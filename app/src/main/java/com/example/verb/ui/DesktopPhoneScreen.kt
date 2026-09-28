package com.example.verb.ui

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import com.example.verb.mobile.DesktopPairingLink
import com.example.verb.mobile.DesktopBridgeClient
import com.example.verb.viewmodel.DesktopPhoneViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.nio.charset.StandardCharsets

/** Deliberately separate from the phone's own terminal: this is the desktop PTY, viewed live. */
@Composable
fun DesktopPhoneScreen(state: DesktopPhoneViewModel, initialLink: String? = null,
                       onLinkConsumed: () -> Unit = {}) {
    DisposableEffect(state) {
        onDispose {
            if (state.token != null) {
                state.connected = false
                state.status = "View paused. Reconnect to refresh the desktop screen."
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
            try { block() }
            catch (error: CancellationException) { throw error }
            catch (error: Exception) { state.status = error.message ?: "Desktop connection failed." }
            finally { state.busy = false }
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

    Column(modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("Control a desktop session", style = MaterialTheme.typography.headlineSmall)
        Text("Your agent and files stay on the desktop. Connect both devices to the same network.",
            style = MaterialTheme.typography.bodySmall)
        if (state.token == null) {
            OutlinedTextField(value = state.linkText, onValueChange = { state.linkText = it },
                modifier = Modifier.fillMaxWidth(), label = { Text("Pairing link") },
                placeholder = { Text("verb://pair#...") }, minLines = 2)
            Button(onClick = {
                runAction {
                    val parsed = DesktopPairingLink.parse(state.linkText)
                    val next = DesktopBridgeClient(parsed)
                    val paired = withContext(Dispatchers.IO) { next.pair() }
                    state.client = next; state.token = paired; state.connected = true
                    state.pairedLink = state.linkText.trim()
                    state.linkText = ""
                    state.status = "Paired. Loading the desktop screen…"
                }
            }, enabled = !state.busy && state.linkText.isNotBlank()) { Text("Connect") }
        } else {
            Text(state.status, color = if (state.connected) MaterialTheme.colorScheme.primary
                else MaterialTheme.colorScheme.error)
            val current = state.screen
            if (current != null) {
                Text("Session ${current.sessionId.take(8)} · screen ${current.revision}",
                    style = MaterialTheme.typography.labelSmall)
                Surface(modifier = Modifier.fillMaxWidth().heightIn(min = 240.dp),
                    color = MaterialTheme.colorScheme.surfaceContainerLow) {
                    Text(current.text, modifier = Modifier.horizontalScroll(rememberScrollState())
                        .padding(12.dp), fontFamily = FontFamily.Monospace,
                        style = MaterialTheme.typography.bodySmall, softWrap = false)
                }
            }
            if (!state.connected) {
                Button(onClick = {
                    val activeClient = state.client ?: return@Button
                    val activeToken = state.token ?: return@Button
                    runAction {
                        withContext(Dispatchers.IO) { activeClient.reconnect(activeToken) }
                        state.connected = true
                        state.status = "Reconnected. Loading the desktop screen…"
                    }
                }, enabled = !state.busy) { Text("Reconnect") }
            } else if (current?.controller != "phone") {
                Button(onClick = {
                    val activeClient = state.client ?: return@Button
                    val activeToken = state.token ?: return@Button
                    runAction {
                        withContext(Dispatchers.IO) { activeClient.take(activeToken) }
                        state.status = "Phone controls input."
                    }
                }, enabled = !state.busy) { Text("Take input control") }
            } else {
                OutlinedTextField(value = state.command, onValueChange = { state.command = it },
                    modifier = Modifier.fillMaxWidth(), label = { Text("Type in desktop terminal") })
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(onClick = {
                        val activeClient = state.client ?: return@Button
                        val activeToken = state.token ?: return@Button
                        val bytes = (state.command + "\r").toByteArray(StandardCharsets.UTF_8)
                        runAction {
                            withContext(Dispatchers.IO) { activeClient.input(activeToken, bytes) }
                            state.command = ""
                        }
                    }, enabled = !state.busy && state.command.isNotBlank()) { Text("Send ↵") }
                    OutlinedButton(onClick = {
                        val activeClient = state.client ?: return@OutlinedButton
                        val activeToken = state.token ?: return@OutlinedButton
                        runAction { withContext(Dispatchers.IO) {
                            activeClient.input(activeToken, byteArrayOf(3)) } }
                    }, enabled = !state.busy) { Text("Ctrl-C") }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("Esc" to byteArrayOf(27), "↑" to byteArrayOf(27, 91, 65),
                        "↓" to byteArrayOf(27, 91, 66)).forEach { (label, bytes) ->
                        OutlinedButton(onClick = {
                            val activeClient = state.client ?: return@OutlinedButton
                            val activeToken = state.token ?: return@OutlinedButton
                            runAction { withContext(Dispatchers.IO) {
                                activeClient.input(activeToken, bytes) } }
                        }, enabled = !state.busy) { Text(label) }
                    }
                }
            }
            if (state.connected) {
                OutlinedButton(onClick = {
                    val activeClient = state.client ?: return@OutlinedButton
                    val activeToken = state.token ?: return@OutlinedButton
                    state.connected = false
                    state.status = "Disconnecting. The desktop session keeps running."
                    runAction {
                        withContext(Dispatchers.IO) { activeClient.disconnect(activeToken) }
                        state.status = "Disconnected. Reconnect whenever the desktop is available."
                    }
                }, enabled = !state.busy) { Text("Disconnect phone") }
            } else {
                OutlinedButton(onClick = {
                    state.client = null; state.token = null; state.screen = null
                    state.pairedLink = null
                    state.status = "Pairing forgotten. Create a new link on the desktop to connect again."
                }, enabled = !state.busy) { Text("Forget pairing") }
            }
        }
    }
}
