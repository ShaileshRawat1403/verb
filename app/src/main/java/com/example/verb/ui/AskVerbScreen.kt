package com.example.verb.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.example.verb.ai.AiAssistantState
import com.example.verb.ai.AiProviderSettings
import com.example.verb.model.ActionResult
import com.example.verb.model.VerbIntent

/**
 * The one place a person types a sentence at Verb.
 *
 * Before this existed there were four: an Ask tab backed by the deterministic intent engine, an
 * Assistant tab backed by a configured model provider, a natural-language sheet reachable from the
 * terminal header, and a semantic lens over selected text. Two of them were permanent tabs that
 * looked identical and answered differently, so there was no way for a user to know which one would
 * understand their question -- the exact ambiguity Verb exists to remove.
 *
 * The stages are ordered, and the order is the product's architectural rule made visible
 * (`docs/PRD.md`: `OBSERVED FACT -> ... -> AI INTERPRETATION -> USER-APPROVED ACTION`):
 *
 * * **Verb actions** is first and is the default. It resolves what you type to a capability Verb
 *   actually has, shows what it will do, and still requires confirmation for anything that changes
 *   state. No model is involved.
 * * **Interpretation** is second, optional, and visibly separate. It is a model talking, it can
 *   execute nothing, and it is labelled as such.
 *
 * Nothing is merged into one answer box, because a deterministic result and a model's opinion are
 * different kinds of claim and the interface must not blur them.
 */
@Composable
fun AskVerbScreen(
    // Stage one: the deterministic intent/action path.
    queryInput: String,
    isExecuting: Boolean,
    currentResult: ActionResult?,
    historyList: List<ActionResult>,
    confirmationPending: ActionResult?,
    onQueryChange: (String) -> Unit,
    onSubmitQuery: (String) -> Unit,
    onSubmitIntent: (VerbIntent) -> Unit,
    onConfirmAction: () -> Unit,
    onDismissConfirmation: () -> Unit,
    onOpenTerminal: () -> Unit,
    onInspectText: (String) -> Unit,
    // Stage two: the optional, clearly-labelled provider interpretation.
    providerSettings: AiProviderSettings,
    assistantPrompt: String,
    assistantState: AiAssistantState,
    onAssistantPromptChange: (String) -> Unit,
    onSubmitAssistantPrompt: (String) -> Unit,
    onOpenProviderSettings: () -> Unit,
    isKeyboardVisible: Boolean = false,
    modifier: Modifier = Modifier
) {
    var stage by rememberSaveable { mutableStateOf(AskVerbStage.ACTIONS) }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .testTag("ask_verb_screen")
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            FilterChip(
                selected = stage == AskVerbStage.ACTIONS,
                onClick = { stage = AskVerbStage.ACTIONS },
                label = { Text("Verb actions") },
                modifier = Modifier.testTag("ask_stage_actions")
            )
            FilterChip(
                selected = stage == AskVerbStage.INTERPRETATION,
                onClick = { stage = AskVerbStage.INTERPRETATION },
                label = { Text("Interpretation") },
                modifier = Modifier.testTag("ask_stage_interpretation")
            )
        }

        when (stage) {
            AskVerbStage.ACTIONS -> AskScreen(
                queryInput = queryInput,
                isExecuting = isExecuting,
                currentResult = currentResult,
                historyList = historyList,
                confirmationPending = confirmationPending,
                isKeyboardVisible = isKeyboardVisible,
                onQueryChange = onQueryChange,
                onSubmitQuery = onSubmitQuery,
                onSubmitIntent = onSubmitIntent,
                onConfirmAction = onConfirmAction,
                onDismissConfirmation = onDismissConfirmation,
                onOpenTerminal = onOpenTerminal,
                onInspectText = onInspectText
            )

            AskVerbStage.INTERPRETATION -> Column(modifier = Modifier.fillMaxSize()) {
                // Said before the input, not after: a person deciding whether to type something
                // needs to know where it goes while they are still deciding.
                Text(
                    text = "A model you configured answers here. It cannot run anything, and Verb " +
                        "does not attach terminal output, command text, file contents, transcripts " +
                        "or credentials — only the words you type below are sent.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier
                        .padding(horizontal = 16.dp)
                        .testTag("ask_interpretation_boundary")
                )
                AssistantScreen(
                    providerSettings = providerSettings,
                    prompt = assistantPrompt,
                    state = assistantState,
                    isKeyboardVisible = isKeyboardVisible,
                    onPromptChange = onAssistantPromptChange,
                    onSubmitPrompt = onSubmitAssistantPrompt,
                    onOpenProviderSettings = onOpenProviderSettings
                )
            }
        }
    }
}

/** The two stages of Ask Verb, in the order the architecture requires them to be read. */
enum class AskVerbStage { ACTIONS, INTERPRETATION }
