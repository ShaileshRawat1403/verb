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
import com.example.verb.terminal.RuntimeProfileId

private val VERIFIED_AGENT_PROFILES = setOf(
    RuntimeProfileId.CLAUDE_CODE,
    RuntimeProfileId.CODEX,
    RuntimeProfileId.OPENCODE
)

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
    // The terminals open in this project. Empty only before the first one exists, which in practice
    // is never -- the workspace always has one.
    terminalSessionIds: List<String> = emptyList(),
    activeTerminalSessionId: String? = null,
    agentInTerminal: (String) -> String? = { null },
    canOpenMoreTerminals: Boolean = false,
    onOpenTerminalSession: () -> Unit = {},
    onSwitchTerminalSession: (String) -> Unit = {},
    onCloseTerminalSession: (String) -> Unit = {},
    modifier: Modifier = Modifier
) {
    // Admission is evidence-based, not package-discovery based. Profiles remain available to the
    // runtime layer for future validation work, but the product surface shows only integrations
    // Verb has actually implemented and tested.
    val agents = reports.filter { it.profile.id in VERIFIED_AGENT_PROFILES }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp)
            .testTag("agents_screen")
    ) {
        // Terminals first: "where was I working?" is the question a person arrives with, and the
        // agents below are a source underneath it rather than the top of the screen.
        if (terminalSessionIds.isNotEmpty()) {
            TerminalSessionsCard(
                sessionIds = terminalSessionIds,
                activeId = activeTerminalSessionId,
                agentIn = agentInTerminal,
                canOpenMore = canOpenMoreTerminals,
                onOpen = onOpenTerminalSession,
                onSwitch = onSwitchTerminalSession,
                onClose = onCloseTerminalSession
            )
            Spacer(modifier = Modifier.height(16.dp))
        }

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
                session = agentSessions[report.profile.id],
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
    session: com.example.verb.session.VerbSession? = null,
    onResumeSession: () -> Unit = {},
    onStartNewSession: () -> Unit = {}
) {
    val profile = report.profile
    val launch = profile.launchLine ?: return

    // One resolver decides what this card says. The screen renders a status; it does not assemble
    // one from four different sources, which is how the same wrong label kept reappearing in
    // different places. See AgentStatusResolver.
    val status = com.example.verb.session.AgentStatusResolver.resolve(
        com.example.verb.session.AgentStatusResolver.Evidence(
            report = report,
            session = session,
            signedIn = when (signInState) {
                com.example.verb.terminal.AgentSignInState.SIGNED_IN -> true
                com.example.verb.terminal.AgentSignInState.SIGNED_OUT -> false
                com.example.verb.terminal.AgentSignInState.UNKNOWN -> null
            },
            installing = installing,
            otherInstallRunning = anyInstalling
        )
    )
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
                    status.label,
                    style = MaterialTheme.typography.labelMedium,
                    color = when (status.action) {
                        com.example.verb.session.AgentStatusResolver.Action.RESUME,
                        com.example.verb.session.AgentStatusResolver.Action.OPEN ->
                            MaterialTheme.colorScheme.primary
                        com.example.verb.session.AgentStatusResolver.Action.NONE ->
                            if (status.label == "Unavailable") {
                                MaterialTheme.colorScheme.error
                            } else {
                                MaterialTheme.colorScheme.onSurfaceVariant
                            }
                        else -> MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    modifier = Modifier.testTag("agent_session_status_${profile.id.name.lowercase()}")
                )
            }

            status.detail?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (status.label == "Unavailable") {
                        MaterialTheme.colorScheme.error
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    modifier = Modifier
                        .padding(top = 2.dp)
                        .testTag("agent_detail_${profile.id.name.lowercase()}")
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

            // "Ready" only ever means the binary runs. A credential file is evidence that login
            // material was saved, not proof the provider still accepts it. Show that weaker fact
            // only when the agent is installed and Verb actually knows it.
            if (
                report.isReady &&
                status.detail == null &&
                signInState != com.example.verb.terminal.AgentSignInState.UNKNOWN
            ) {
                val signedIn = signInState == com.example.verb.terminal.AgentSignInState.SIGNED_IN
                Text(
                    com.example.verb.session.AgentStatusResolver.signedInDetail(signedIn).orEmpty(),
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

            Spacer(modifier = Modifier.height(10.dp))
            when (status.action) {
                com.example.verb.session.AgentStatusResolver.Action.RESUME -> Button(
                    onClick = onResumeSession,
                    modifier = Modifier.testTag("agent_resume_${profile.id.name.lowercase()}")
                ) { Text("Resume") }

                com.example.verb.session.AgentStatusResolver.Action.START_NEW -> OutlinedButton(
                    onClick = onStartNewSession,
                    modifier = Modifier.testTag("agent_start_new_${profile.id.name.lowercase()}")
                ) { Text("Start new") }

                com.example.verb.session.AgentStatusResolver.Action.NONE -> Unit

                com.example.verb.session.AgentStatusResolver.Action.OPEN -> Button(
                    onClick = { onLaunch(launch) },
                    modifier = Modifier.testTag("agent_open_${profile.id.name.lowercase()}")
                ) { Text("Open ${profile.displayName}") }

                com.example.verb.session.AgentStatusResolver.Action.INSTALL -> OutlinedButton(
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
