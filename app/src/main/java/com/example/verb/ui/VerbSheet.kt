package com.example.verb.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.example.verb.viewmodel.VerbTask

/**
 * Everything Verb can do, by name.
 *
 * This is the touch equivalent of the desktop leader and command palette, and it exists so the app
 * needs no permanent navigation at all. It is *searched, not browsed*: the list is flat, every row
 * is a human task rather than a subsystem, and typing narrows it. That is the shape
 * `docs/UX_FOUNDATION.md` prescribes for configuration and capability alike, and the reason a
 * growing set of capabilities does not cost the workspace a single row of chrome.
 *
 * It is a modal sheet on purpose. It appears because the user asked for it, it takes no permanent
 * space, and dismissing it hands the keyboard straight back to the terminal.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VerbSheet(
    onDismiss: () -> Unit,
    onOpenTask: (VerbTask) -> Unit,
    tasks: List<VerbTask> = VerbTask.entries.toList()
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var query by remember { mutableStateOf("") }
    val matches = remember(query, tasks) { tasks.filter { it.matches(query) } }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        modifier = Modifier.testTag("verb_sheet")
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp)
                .padding(bottom = 20.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                text = "Verb",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold
            )
            Text(
                text = "Type what you are trying to do.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                modifier = Modifier
                    .fillMaxWidth()
                    .testTag("verb_sheet_search"),
                singleLine = true,
                placeholder = { Text("Search Verb") },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search)
            )

            if (matches.isEmpty()) {
                // Naming the absence, rather than leaving a blank panel that reads as a bug.
                Text(
                    text = "Nothing here matches \"${query.trim()}\". Verb only lists what it can " +
                        "actually do on this device.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.testTag("verb_sheet_no_matches")
                )
            } else {
                LazyColumn(
                    modifier = Modifier.heightIn(max = 420.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    items(matches, key = { it.name }) { task ->
                        VerbSheetRow(task = task, onClick = { onOpenTask(task) })
                    }
                }
            }
        }
    }
}

@Composable
private fun VerbSheetRow(task: VerbTask, onClick: () -> Unit) {
    Surface(
        onClick = onClick,
        modifier = Modifier
            .fillMaxWidth()
            .testTag("verb_task_${task.name.lowercase()}"),
        color = MaterialTheme.colorScheme.surface
    ) {
        Column(modifier = Modifier.padding(vertical = 10.dp, horizontal = 4.dp)) {
            Text(text = task.title, style = MaterialTheme.typography.bodyLarge)
            Text(
                text = task.subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
