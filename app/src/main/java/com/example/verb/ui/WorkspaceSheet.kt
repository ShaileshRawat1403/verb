package com.example.verb.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CreateNewFolder
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.verb.project.VerbProject

/**
 * One surface answering "where was I, and how do I get back there".
 *
 * Projects and terminals used to be two separate places: a project sheet reached from a header chip
 * that disappears under width pressure, and a terminals card living on another screen entirely. A
 * person returning to Verb after a day away therefore had to already know both the name of the
 * project they wanted and the fact that terminals were listed somewhere else. That is the ORIENT
 * failure `docs/UX_FOUNDATION.md` names, and splitting the answer across two surfaces is what
 * caused it.
 *
 * The order is deliberate. Terminals come first because switching between the agent's terminal and
 * your own is the move made many times an hour; projects come second because changing project is
 * the move made a few times a day, and it restarts the session.
 *
 * It remains a modal sheet, opened on request and dismissed straight back to the terminal, so the
 * workspace still pays nothing for it in permanent chrome.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WorkspaceSheet(
    projects: List<VerbProject>,
    selectedProject: VerbProject?,
    terminalSessionIds: List<String>,
    activeTerminalSessionId: String?,
    agentInTerminal: (String) -> String?,
    canOpenMoreTerminals: Boolean,
    onDismiss: () -> Unit,
    onCreateProject: (String) -> Boolean,
    onSelectProject: (String) -> Unit,
    onSwitchTerminal: (String) -> Unit,
    onOpenTerminal: () -> Unit,
    onCloseTerminal: (String) -> Unit
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        containerColor = MaterialTheme.colorScheme.surface,
        contentColor = MaterialTheme.colorScheme.onSurface,
        modifier = Modifier.testTag("workspace_sheet")
    ) {
        WorkspaceSheetContent(
            projects = projects,
            selectedProject = selectedProject,
            terminalSessionIds = terminalSessionIds,
            activeTerminalSessionId = activeTerminalSessionId,
            agentInTerminal = agentInTerminal,
            canOpenMoreTerminals = canOpenMoreTerminals,
            onCreateProject = onCreateProject,
            onSelectProject = onSelectProject,
            onSwitchTerminal = onSwitchTerminal,
            onOpenTerminal = onOpenTerminal,
            onCloseTerminal = onCloseTerminal
        )
    }
}

/**
 * The sheet's body, separated from the sheet itself.
 *
 * Not a stylistic split. A `ModalBottomSheet` renders into its own dialog window, which Robolectric
 * drives unreliably, so a test written against the whole sheet tests the host as much as the
 * content. The list logic -- which terminal is in front, which project is current, what is
 * selectable -- is what has behaviour worth pinning, and this is the seam that lets it be pinned
 * directly.
 */
@Composable
internal fun WorkspaceSheetContent(
    projects: List<VerbProject>,
    selectedProject: VerbProject?,
    terminalSessionIds: List<String>,
    activeTerminalSessionId: String?,
    agentInTerminal: (String) -> String?,
    canOpenMoreTerminals: Boolean,
    onCreateProject: (String) -> Boolean,
    onSelectProject: (String) -> Unit,
    onSwitchTerminal: (String) -> Unit,
    onOpenTerminal: () -> Unit,
    onCloseTerminal: (String) -> Unit
) {
    var name by remember { mutableStateOf("") }
    var createFailed by remember { mutableStateOf(false) }
    val scrollState = rememberScrollState()

    fun createProject() {
        if (name.isNotBlank()) {
            createFailed = !onCreateProject(name)
            if (!createFailed) name = ""
        }
    }

    Column(
        modifier = Modifier
            .padding(horizontal = 20.dp)
            .padding(bottom = 20.dp)
            .heightIn(max = 560.dp)
            .verticalScroll(scrollState)
    ) {
        Text("Workspace", fontSize = 17.sp, fontWeight = FontWeight.Bold)
        Text(
            "The project you are in, and the terminals open inside it.",
            fontSize = 12.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 2.dp)
        )

        Spacer(Modifier.height(16.dp))
        Text(
            "Terminals",
            fontSize = 13.sp,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        if (terminalSessionIds.isEmpty()) {
            Text(
                "No terminal is open in this project yet.",
                fontSize = 12.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 8.dp)
            )
        }
        terminalSessionIds.forEachIndexed { index, id ->
            val isActive = id == activeTerminalSessionId
            val occupant = agentInTerminal(id)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(enabled = !isActive) { onSwitchTerminal(id) }
                    .semantics {
                        stateDescription = if (isActive) "In front" else "Not in front"
                    }
                    .padding(vertical = 10.dp)
                    .testTag("workspace_terminal_$id"),
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
                Spacer(Modifier.width(10.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "Terminal ${index + 1}",
                        fontSize = 14.sp,
                        fontWeight = if (isActive) FontWeight.SemiBold else FontWeight.Normal
                    )
                    Text(
                        // "Empty" would be wrong: a shell prompt is not nothing.
                        text = occupant?.let { "$it is running here" }
                            ?: if (isActive) "your shell, in front" else "your shell",
                        fontSize = 12.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                if (terminalSessionIds.size > 1) {
                    TextButton(
                        onClick = { onCloseTerminal(id) },
                        modifier = Modifier.testTag("workspace_close_$id")
                    ) {
                        Text("Close", fontSize = 12.sp)
                    }
                }
            }
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            TextButton(
                onClick = onOpenTerminal,
                enabled = canOpenMoreTerminals,
                modifier = Modifier.testTag("workspace_new_terminal")
            ) {
                Text("New terminal")
            }
            if (!canOpenMoreTerminals) {
                // The ceiling is stated where it is reached, not discovered by the phone
                // running out of memory.
                Text(
                    "That is as many as this device should host at once.",
                    fontSize = 11.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }

        HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))

        Text(
            "Projects",
            fontSize = 13.sp,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Text(
            "Switching project starts a fresh terminal in that directory.",
            fontSize = 11.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 2.dp)
        )
        projects.forEach { project ->
            val isSelected = project.id == selectedProject?.id
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(enabled = !isSelected) { onSelectProject(project.id) }
                    .semantics {
                        stateDescription = if (isSelected) "Current project" else "Not current"
                    }
                    .padding(vertical = 10.dp)
                    .testTag("workspace_project_${project.id}"),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = if (isSelected) "●" else "○",
                    color = if (isSelected) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    fontSize = 13.sp
                )
                Spacer(Modifier.width(10.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        // The name the person typed, not the disambiguating suffix Verb
                        // appended. An id shown whole reads as machine output and is the
                        // reason a project list was hard to recognise your own work in.
                        text = project.displayName,
                        fontSize = 14.sp,
                        fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Normal,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Text(
                        // The suffix is still shown, because two projects may share a name and
                        // this is the only thing that tells them apart.
                        text = if (isSelected) "${project.shortId}  ·  current" else project.shortId,
                        fontSize = 11.sp,
                        fontFamily = FontFamily.Monospace,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }

        OutlinedTextField(
            value = name,
            onValueChange = { name = it },
            label = { Text("New project") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { createProject() }),
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 8.dp)
                .testTag("workspace_new_project_name")
        )
        Button(
            onClick = ::createProject,
            enabled = name.isNotBlank(),
            modifier = Modifier
                .padding(top = 8.dp)
                .testTag("workspace_create_project")
        ) {
            Icon(Icons.Default.CreateNewFolder, contentDescription = null, modifier = Modifier.size(16.dp))
            Spacer(Modifier.width(6.dp))
            Text("Create")
        }
        if (createFailed) {
            Text(
                "Verb could not create that project. Your existing projects were not changed.",
                color = MaterialTheme.colorScheme.error,
                fontSize = 12.sp,
                modifier = Modifier
                    .padding(top = 8.dp)
                    .testTag("workspace_create_failed")
            )
        }
    }
}
