package com.example.verb.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.UnfoldMore
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Where you are: which project, and which terminal inside it.
 *
 * The header already carries a project chip, but `docs/UX_FOUNDATION.md` makes that chip the first
 * thing to give way when the row runs out of width -- and the row runs out of width at exactly the
 * moment a second terminal appears. So the one time a person most needs to know which project and
 * which terminal they are typing into is the one time the header stops saying. On a physical Vivo
 * I2202 with two terminals open, the project name was replaced by a folder glyph and nothing named
 * the terminal at all.
 *
 * This line is the answer, and it is deliberately *not* in the header: it never competes for that
 * row's width, so the degrade order above it is left exactly as designed.
 *
 * It is orientation, not navigation. It names the current state and offers one way to change it;
 * it is not a tab bar, and it does not grow a row per destination. That is the distinction
 * `docs/UX_FOUNDATION.md` draws when it refuses permanent chrome -- what it refuses is *browsing*
 * surfaces, not the workspace saying where it is. A terminal that will not tell you which directory
 * it is in is the thing every desktop terminal already knows better than to be.
 */
@Composable
fun WorkspaceContextBar(
    projectLabel: String?,
    terminalLabel: String?,
    occupant: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val project = projectLabel ?: "No project"
    val readOut = buildString {
        append("Workspace. Project $project.")
        terminalLabel?.let { append(" $it.") }
        occupant?.let { append(" Running $it.") }
        append(" Activate to switch project or terminal.")
    }
    Surface(
        color = MaterialTheme.colorScheme.surfaceVariant,
        onClick = onClick,
        modifier = modifier
            .fillMaxWidth()
            .semantics { contentDescription = readOut }
            .testTag("workspace_context_bar")
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 7.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Icon(
                imageVector = Icons.Default.FolderOpen,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(14.dp)
            )
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = project,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.testTag("workspace_context_project")
                )
                // The second line is what the header never had room to say. Naming the occupant
                // rather than only the terminal number is the difference between "Terminal 2" and
                // "the one Codex is in" -- which is how a person actually remembers where they were.
                Text(
                    text = listOfNotNull(terminalLabel, occupant?.let { "$it running" })
                        .joinToString("  ·  ")
                        .ifEmpty { "No terminal open" },
                    fontSize = 11.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.testTag("workspace_context_terminal")
                )
            }
            Icon(
                imageVector = Icons.Default.UnfoldMore,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(16.dp)
            )
        }
    }
}
