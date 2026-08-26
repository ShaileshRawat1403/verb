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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.example.verb.ai.AiProviderSettings
import com.example.verb.model.ActionResult
import com.example.verb.model.VerbIntent
import com.example.verb.terminal.TerminalAiExchange
import com.example.verb.terminal.TerminalEvidence

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
 * * **Ask** is second, optional, and visibly separate. It is a model talking, it can execute
 *   nothing, and it is labelled as such.
 *
 * Nothing is merged into one answer box, because a deterministic result and a model's opinion are
 * different kinds of claim and the interface must not blur them.
 *
 * The second stage used to be an "Interpretation" screen that sent nothing but the words the user
 * typed -- the chatbot-you-must-explain-yourself-to that `docs/PRODUCT_VISION.md` rejects by name.
 * It is now [AssistPanel], the same evidence-bound assistant the terminal opens, so the fourth ask
 * box does not quietly reappear inside the screen built to remove the other three.
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
    // Stage two: the optional, clearly-labelled, evidence-bound assistant.
    providerSettings: AiProviderSettings = AiProviderSettings(),
    onOpenProviderSettings: () -> Unit = {},
    aiExplanation: String? = null,
    isAiExplaining: Boolean = false,
    aiEvidence: TerminalEvidence? = null,
    aiThread: List<TerminalAiExchange> = emptyList(),
    onAskVerbAi: (String) -> Unit = {},
    onExplainEvidence: () -> Unit = {},
    onClearAiThread: () -> Unit = {},
    isKeyboardVisible: Boolean = false,
    startOnAssistant: Boolean = false,
    onAssistantStageConsumed: () -> Unit = {},
    modifier: Modifier = Modifier
) {
    var stage by rememberSaveable { mutableStateOf(AskVerbStage.ACTIONS) }

    // Opened from the terminal, where the question already occurred to the person: skip the stage
    // they did not ask for. Consumed immediately so coming back later starts on actions again --
    // `docs/PRD.md` orders observed fact before interpretation, and that default has to hold.
    LaunchedEffect(startOnAssistant) {
        if (startOnAssistant) {
            stage = AskVerbStage.ASK
            onAssistantStageConsumed()
        }
    }

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
                selected = stage == AskVerbStage.ASK,
                onClick = { stage = AskVerbStage.ASK },
                label = { Text("Ask") },
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

            AskVerbStage.ASK -> Column(modifier = Modifier.fillMaxSize()) {
                // Said before the input, not after: a person deciding whether to type something
                // needs to know where it goes while they are still deciding. The claim is narrower
                // than it used to be because the envelope is real -- structural facts Verb observed
                // itself do travel, and the panel shows exactly which ones.
                Text(
                    text = "A model you configured answers here. It cannot run anything. Verb " +
                        "attaches only the structural facts it observed itself, listed under " +
                        "\"Based on\" — never terminal output, command text, file contents, " +
                        "transcripts or credentials.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier
                        .padding(horizontal = 16.dp)
                        .testTag("ask_interpretation_boundary")
                )
                AssistPanel(
                    aiExplanation = aiExplanation,
                    isAiExplaining = isAiExplaining,
                    evidence = aiEvidence,
                    thread = aiThread,
                    onAsk = onAskVerbAi,
                    onExplain = onExplainEvidence,
                    onClearThread = onClearAiThread,
                    showHeader = false,
                    showBoundaryNote = false,
                    providerSettings = providerSettings,
                    onOpenProviderSettings = onOpenProviderSettings
                )
            }
        }
    }
}

/** The two stages of Ask Verb, in the order the architecture requires them to be read. */
enum class AskVerbStage { ACTIONS, ASK }
