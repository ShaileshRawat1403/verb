package com.example.verb.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import com.example.verb.terminal.RuntimeProfileReport

/**
 * The agents surface: what you open, separated from what you install.
 *
 * Every agent used to sit in one flat list of thirteen runtime profiles, alongside Core CLI, the
 * package repository and the native toolchain, all at equal weight and eight scrolls deep. That
 * ordering described how Verb is built rather than what it is for. Agents are the product, so they
 * get their own surface and the toolchains move behind them.
 *
 * Launching writes the command into the terminal and runs it there, rather than starting anything
 * invisibly. The user sees exactly what was run, can interrupt it, can retype it, and learns the
 * command by watching it happen -- which is also the difference between a launcher and a black box.
 */
@Composable
fun AgentsScreen(
    reports: List<RuntimeProfileReport>,
    keyStatus: List<AgentKeyStatus>,
    signInStates: Map<com.example.verb.terminal.RuntimeProfileId, com.example.verb.terminal.AgentSignInState> = emptyMap(),
    onLaunch: (String) -> Unit,
    onInstall: (com.example.verb.terminal.RuntimeProfileId) -> Unit,
    onEditKeys: () -> Unit,
    installingProfile: com.example.verb.terminal.RuntimeProfileId? = null,
    message: String? = null,
    // Empty until the user has launched an agent at least once, per agentSessionDisplay()'s own
    // contract -- a card with no tracked session falls back to its normal install/ready display.
    // Keyed by profile so every agent Verb can recover reads the same way; nothing here is
    // Claude-specific.
    agentSessions: Map<com.example.verb.terminal.RuntimeProfileId, com.example.verb.session.VerbSession> = emptyMap(),
    onResumeSession: (com.example.verb.terminal.RuntimeProfileId) -> Unit = {},
    onStartNewSession: (com.example.verb.terminal.RuntimeProfileId) -> Unit = {},
    modifier: Modifier = Modifier
) {
    val agents = reports.filter { it.profile.isAgent }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp)
            .testTag("agents_screen")
    ) {
        Text("Agents", style = MaterialTheme.typography.headlineSmall)
        Text(
            "Opening an agent types its command into the terminal, so you can always see and stop what is running.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp, bottom = 12.dp)
        )

        agents.forEach { report ->
            AgentRow(
                report = report,
                signInState = signInStates[report.profile.id]
                    ?: com.example.verb.terminal.AgentSignInState.UNKNOWN,
                installing = installingProfile == report.profile.id,
                anyInstalling = installingProfile != null,
                onLaunch = onLaunch,
                onInstall = onInstall,
                sessionDisplay = com.example.verb.session.agentSessionDisplay(agentSessions[report.profile.id]),
                onResumeSession = { onResumeSession(report.profile.id) },
                onStartNewSession = { onStartNewSession(report.profile.id) }
            )
            Spacer(modifier = Modifier.height(10.dp))
        }

        message?.let {
            Text(
                it,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(vertical = 8.dp).testTag("agents_message")
            )
        }

        Spacer(modifier = Modifier.height(8.dp))
        KeysCard(keyStatus = keyStatus, onEditKeys = onEditKeys)
    }
}

@Composable
private fun AgentRow(
    report: RuntimeProfileReport,
    signInState: com.example.verb.terminal.AgentSignInState,
    installing: Boolean,
    anyInstalling: Boolean,
    onLaunch: (String) -> Unit,
    onInstall: (com.example.verb.terminal.RuntimeProfileId) -> Unit,
    sessionDisplay: com.example.verb.session.AgentSessionDisplay? = null,
    onResumeSession: () -> Unit = {},
    onStartNewSession: () -> Unit = {}
) {
    val profile = report.profile
    val launch = profile.launchLine ?: return
    Card(
        modifier = Modifier.fillMaxWidth().testTag("agent_${profile.id.name.lowercase()}"),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(modifier = Modifier.padding(14.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(profile.displayName, style = MaterialTheme.typography.titleSmall)
                Text(
                    when {
                        sessionDisplay != null -> sessionDisplay.statusLabel
                        installing -> "Installing"
                        report.isReady -> "Ready"
                        report.isUnsatisfiable -> "Unavailable"
                        else -> "Not installed"
                    },
                    style = MaterialTheme.typography.labelMedium,
                    color = when {
                        sessionDisplay?.showResume == true -> MaterialTheme.colorScheme.primary
                        sessionDisplay != null -> MaterialTheme.colorScheme.onSurfaceVariant
                        report.isReady -> MaterialTheme.colorScheme.primary
                        report.isUnsatisfiable -> MaterialTheme.colorScheme.error
                        else -> MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    modifier = Modifier.testTag("agent_session_status_${profile.id.name.lowercase()}")
                )
            }

            sessionDisplay?.detailLabel?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 2.dp)
                )
            }

            // The command is shown whether or not it is ready: it is how the agent is actually
            // started, and seeing it is what makes the button non-magical.
            Text(
                launch,
                style = MaterialTheme.typography.bodySmall,
                fontFamily = FontFamily.Monospace,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 2.dp)
            )

            // "Ready" only ever meant the binary runs. Whether you are signed in is a separate
            // fact, and the one that decides what happens when you tap Open. Shown only once the
            // agent is installed -- sign-in is not a question you can act on before then -- and
            // only when Verb actually knows: an agent whose credential location has not been
            // observed says nothing rather than implying you are signed out.
            if (report.isReady && signInState != com.example.verb.terminal.AgentSignInState.UNKNOWN) {
                val signedIn = signInState == com.example.verb.terminal.AgentSignInState.SIGNED_IN
                Text(
                    if (signedIn) "Signed in" else "Not signed in — run $launch to sign in",
                    style = MaterialTheme.typography.bodySmall,
                    color = if (signedIn) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    modifier = Modifier
                        .padding(top = 2.dp)
                        .testTag("agent_signin_${profile.id.name.lowercase()}")
                )
            }

            if (report.isUnsatisfiable) {
                Text(
                    "Cannot run on this device. No install will resolve this.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 4.dp)
                )
            }

            Spacer(modifier = Modifier.height(10.dp))
            when {
                // A tracked session takes over the action slot entirely: requirement 1 is that this
                // reads only VerbSession.state (via agentSessionDisplay), never report.isReady.
                sessionDisplay?.showResume == true -> Button(
                    onClick = onResumeSession,
                    modifier = Modifier.testTag("agent_resume_${profile.id.name.lowercase()}")
                ) { Text("Resume") }

                sessionDisplay?.showStartNew == true -> OutlinedButton(
                    onClick = onStartNewSession,
                    modifier = Modifier.testTag("agent_start_new_${profile.id.name.lowercase()}")
                ) { Text("Start new") }

                sessionDisplay != null -> Unit // LIVE / INTERRUPTED: nothing to tap yet.

                report.isReady -> Button(
                    onClick = { onLaunch(launch) },
                    modifier = Modifier.testTag("agent_open_${profile.id.name.lowercase()}")
                ) { Text("Open ${profile.displayName}") }

                report.isInstallable -> OutlinedButton(
                    onClick = { onInstall(profile.id) },
                    enabled = !anyInstalling,
                    modifier = Modifier.testTag("agent_install_${profile.id.name.lowercase()}")
                ) { Text(if (installing) "Installing…" else "Install") }
            }
        }
    }
}

/** Whether a key is present. Deliberately never its value. */
data class AgentKeyStatus(val variable: String, val isSet: Boolean)

@Composable
private fun KeysCard(keyStatus: List<AgentKeyStatus>, onEditKeys: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth().testTag("agent_keys_card"),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(modifier = Modifier.padding(14.dp)) {
            Text("API keys", style = MaterialTheme.typography.titleSmall)
            Text(
                // Presence only. A value is never rendered, so a screenshot of this screen can
                // never leak a key.
                "Whether a key is set, never its value. Stored in ~/.env, readable only by you.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 2.dp, bottom = 8.dp)
            )
            keyStatus.forEach { status ->
                Row(
                    modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        status.variable,
                        style = MaterialTheme.typography.bodySmall,
                        fontFamily = FontFamily.Monospace
                    )
                    Text(
                        if (status.isSet) "set" else "not set",
                        style = MaterialTheme.typography.bodySmall,
                        color = if (status.isSet) {
                            MaterialTheme.colorScheme.primary
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        }
                    )
                }
            }
            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))
            OutlinedButton(
                onClick = onEditKeys,
                modifier = Modifier.testTag("agent_keys_edit")
            ) { Text("Edit keys in terminal") }
        }
    }
}
