package com.example.verb.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.example.verb.session.VerbSession
import com.example.verb.session.VerbSessionState
import com.example.verb.terminal.RuntimeProfileId
import com.example.verb.terminal.RuntimeProfileReport

/**
 * The admitted agent integrations Verb has actually implemented and verified.
 *
 * Admission is evidence-based, not package-discovery based: a profile the runtime layer knows how to
 * install is not the same claim as an agent Verb supports (`docs/ARCHITECTURE.md`, "Agent
 * boundary"). Declared once so the workspace's first action and the agents surface cannot drift
 * into offering different sets.
 */
internal val ADMITTED_AGENT_PROFILES: Set<RuntimeProfileId> = setOf(
    RuntimeProfileId.CLAUDE_CODE,
    RuntimeProfileId.CODEX,
    RuntimeProfileId.OPENCODE,
    RuntimeProfileId.ANTIGRAVITY,
    RuntimeProfileId.HERMES
)

/**
 * What the workspace should offer when the user has nothing running.
 *
 * An empty workspace whose most prominent affordance is help is a screen that answers a question
 * nobody asked. This picks the single most useful next move from evidence Verb already holds -- a
 * tracked session's state, and whether an admitted agent's runtime is actually ready -- and offers
 * exactly that one, never a menu.
 *
 * It deliberately never privileges an agent that cannot start: an unready profile is offered as an
 * install, and an install that can never succeed is not offered at all.
 */
sealed interface VerbFirstAction {

    /** Something is already hosted, or there is nothing honest to offer. The workspace stays quiet. */
    data object None : VerbFirstAction

    /**
     * A tracked session has positive recovery evidence. Resuming beats starting over, because
     * starting over abandons a conversation Verb knows it can pick back up.
     */
    data class Resume(val profileId: RuntimeProfileId, val displayName: String) : VerbFirstAction

    /** The agent's runtime is ready, so the useful move is to open it. */
    data class Start(
        val profileId: RuntimeProfileId,
        val displayName: String,
        val command: String
    ) : VerbFirstAction

    /** Nothing is ready yet, but this one can be made ready. */
    data class Install(val profileId: RuntimeProfileId, val displayName: String) : VerbFirstAction
}

/**
 * Chooses the workspace's first action.
 *
 * Pure, and deliberately so: the choice is testable at every combination of runtime readiness and
 * session state without a terminal, a device or a Compose tree.
 */
fun verbFirstAction(
    reports: List<RuntimeProfileReport>,
    sessions: Map<RuntimeProfileId, VerbSession>
): VerbFirstAction {
    val admitted = reports.filter { it.profile.id in ADMITTED_AGENT_PROFILES }

    // Something is hosted right now. The user does not need a suggestion; they need the terminal.
    if (admitted.any { sessions[it.profile.id]?.state == VerbSessionState.LIVE }) {
        return VerbFirstAction.None
    }

    // RECOVERABLE is the only state that carries positive recovery evidence, so it is the only one
    // that earns a Resume. INTERRUPTED means Verb does not know, and offering Resume there would
    // claim something it has not established.
    admitted.firstOrNull {
        it.isReady && sessions[it.profile.id]?.state == VerbSessionState.RECOVERABLE
    }
        ?.let { return VerbFirstAction.Resume(it.profile.id, it.profile.displayName) }

    admitted.firstOrNull { it.isReady && it.profile.launchLine != null }
        ?.let {
            return VerbFirstAction.Start(
                profileId = it.profile.id,
                displayName = it.profile.displayName,
                command = it.profile.launchLine!!
            )
        }

    // `isInstallable` is the report's own distinction between "not installed yet" and "no amount of
    // installing can fix this". Offering an install for the second kind is an invitation to fail.
    admitted.firstOrNull { it.isInstallable }
        ?.let { return VerbFirstAction.Install(it.profile.id, it.profile.displayName) }

    return VerbFirstAction.None
}

/**
 * One compact row above the terminal, offering that single action.
 *
 * Compact on purpose. The workspace is the terminal; this is chrome, and chrome that grows into a
 * panel of suggestions is how a terminal-first product stops being one. It disappears the moment an
 * agent is hosted, and the shell underneath is usable the entire time it is shown -- which is why
 * the dismissal reads as "use the shell" rather than "cancel".
 */
@Composable
fun VerbFirstActionRow(
    action: VerbFirstAction,
    onStart: (String) -> Unit,
    onResume: (RuntimeProfileId) -> Unit,
    onInstall: (RuntimeProfileId) -> Unit,
    onUseShell: () -> Unit,
    modifier: Modifier = Modifier
) {
    if (action is VerbFirstAction.None) return

    Surface(
        modifier = modifier
            .fillMaxWidth()
            .testTag("verb_first_action"),
        color = MaterialTheme.colorScheme.surfaceVariant
    ) {
        Column(modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp)) {
            Text(
                text = when (action) {
                    is VerbFirstAction.Resume ->
                        "${action.displayName} stopped, and Verb can pick that conversation back up."
                    is VerbFirstAction.Start ->
                        "No agent is running. The shell below is yours either way."
                    is VerbFirstAction.Install ->
                        "No agent is installed here yet. The shell below still works."
                    VerbFirstAction.None -> ""
                },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                when (action) {
                    is VerbFirstAction.Resume -> TextButton(
                        onClick = { onResume(action.profileId) },
                        modifier = Modifier.testTag("verb_first_action_primary")
                    ) { Text("Resume ${action.displayName}") }

                    is VerbFirstAction.Start -> TextButton(
                        onClick = { onStart(action.command) },
                        modifier = Modifier.testTag("verb_first_action_primary")
                    ) { Text("Start ${action.displayName}") }

                    is VerbFirstAction.Install -> TextButton(
                        onClick = { onInstall(action.profileId) },
                        modifier = Modifier.testTag("verb_first_action_primary")
                    ) { Text("Install ${action.displayName}") }

                    VerbFirstAction.None -> Unit
                }
                TextButton(
                    onClick = onUseShell,
                    modifier = Modifier.testTag("verb_first_action_shell")
                ) { Text("Use the shell") }
            }
        }
    }
}
