package com.example.verb.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.verb.ui.theme.VerbThemeChoice

/**
 * Three choices and nothing else.
 *
 * `docs/BACKLOG.md` §D0: configuration is found by name, never browsed. This is reached by typing
 * "theme" into the Verb sheet, so it does not need a heading that repeats what the person typed,
 * a settings tree to sit in, or anything to configure beyond the one thing they came for.
 *
 * The selected row carries a tick as well as a colour, because colour never carries meaning alone,
 * and a state description so a screen reader announces which one is on.
 */
@Composable
fun AppearanceScreen(
    choice: VerbThemeChoice,
    onChoose: (VerbThemeChoice) -> Unit,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .padding(20.dp)
            .testTag("appearance_screen")
    ) {
        Text(
            text = "Verb follows your device unless you tell it otherwise.",
            fontSize = 13.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(16.dp))

        VerbThemeChoice.entries.forEach { option ->
            val selected = option == choice
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onChoose(option) }
                    .semantics { stateDescription = if (selected) "Selected" else "Not selected" }
                    .padding(vertical = 12.dp)
                    .testTag("appearance_${option.name.lowercase()}"),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = option.label,
                        fontSize = 15.sp,
                        fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                    Text(
                        text = option.description,
                        fontSize = 12.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                if (selected) {
                    Spacer(modifier = Modifier.width(12.dp))
                    Icon(
                        imageVector = Icons.Default.Check,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(20.dp)
                    )
                }
            }
        }
    }
}
