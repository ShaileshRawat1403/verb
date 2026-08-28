package com.example.verb.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * The terminals open in this project, and which one you are typing into.
 *
 * A project has sessions. Before this there was one terminal, so running an agent and running your
 * own commands were the same slot and you had to choose -- which is the thing that actually stopped
 * people using Verb the way they use a desktop terminal.
 *
 * This is also why the sessions surface exists at all. It used to be the agents card wearing a
 * second name, which is why nothing here answered *"where was I working?"* -- the ORIENT moment
 * `docs/UX_FOUNDATION.md` names and nothing served. Switching is reached by name through the Verb
 * sheet rather than by a tab strip, because D0 says configuration and navigation are searched, not
 * browsed, and a tab bar is exactly the permanent chrome the design refuses.
 */
@Composable
fun TerminalSessionsCard(
    sessionIds: List<String>,
    activeId: String?,
    agentIn: (String) -> String?,
    canOpenMore: Boolean,
    onOpen: () -> Unit,
    onSwitch: (String) -> Unit,
    onClose: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    Card(
        modifier = modifier.fillMaxWidth().testTag("card_terminal_sessions"),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text("Terminals", style = MaterialTheme.typography.titleSmall)
            Text(
                "One for the agent, one for you. Each keeps its own directory.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp)
            )
            Spacer(modifier = Modifier.height(10.dp))

            sessionIds.forEachIndexed { index, id ->
                val isActive = id == activeId
                val agent = agentIn(id)
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable(enabled = !isActive) { onSwitch(id) }
                        .semantics {
                            stateDescription = if (isActive) "In front" else "Not in front"
                        }
                        .padding(vertical = 10.dp)
                        .testTag("terminal_session_$id"),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    // Glyph and word together, never colour alone.
                    Text(
                        text = if (isActive) "●" else "○",
                        color = if (isActive) {
                            MaterialTheme.colorScheme.primary
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                        fontSize = 13.sp
                    )
                    Spacer(modifier = Modifier.width(10.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = "Terminal ${index + 1}",
                            fontSize = 14.sp,
                            fontWeight = if (isActive) FontWeight.SemiBold else FontWeight.Normal,
                            color = MaterialTheme.colorScheme.onSurface
                        )
                        Text(
                            // What is in it, or that it is yours. Saying "empty" would be wrong:
                            // a shell prompt is not nothing.
                            text = agent?.let { "$it is running here" }
                                ?: if (isActive) "your shell, in front" else "your shell",
                            fontSize = 12.sp,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    if (sessionIds.size > 1) {
                        TextButton(
                            onClick = { onClose(id) },
                            modifier = Modifier.testTag("btn_close_$id")
                        ) {
                            Text("Close", fontSize = 12.sp)
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(4.dp))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                TextButton(
                    onClick = onOpen,
                    enabled = canOpenMore,
                    modifier = Modifier.testTag("btn_new_terminal")
                ) {
                    Text("New terminal")
                }
                if (!canOpenMore) {
                    // The ceiling is stated where it is reached, not discovered by the phone
                    // running out of memory.
                    Text(
                        "That is as many as this device should host at once.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}
