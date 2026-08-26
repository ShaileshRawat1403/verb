package com.example.verb.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Psychology
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.verb.ai.AiProviderSettings
import com.example.verb.terminal.TerminalAiExchange
import com.example.verb.terminal.TerminalEvidence

/**
 * The assistant. One surface, two doors.
 *
 * There used to be two ask boxes that answered the same question differently: this one, which
 * attaches exactly the evidence envelope Verb assembled, and an "Interpretation" screen that sent
 * nothing but the words the user typed. The second is the surface `docs/PRODUCT_VISION.md` rejects
 * by name -- *"a user should not have to open a chatbot and begin by explaining themselves"* -- and
 * having both meant a person could not know which one would understand their question. That is the
 * exact ambiguity `AskVerbScreen` was created to remove, reappearing one level down.
 *
 * So this panel is the only place a model answers, reachable from Ask Verb (where a person goes to
 * ask) and from the terminal (where the question usually occurs to them). The evidence it was given
 * renders directly beneath the answer that names it, always, in the plain language
 * `docs/UX_FOUNDATION.md` requires on screen.
 */
@Composable
fun AssistPanel(
    aiExplanation: String?,
    isAiExplaining: Boolean,
    evidence: TerminalEvidence?,
    thread: List<TerminalAiExchange>,
    onAsk: (String) -> Unit,
    onExplain: () -> Unit,
    onClearThread: () -> Unit,
    modifier: Modifier = Modifier,
    showHeader: Boolean = true,
    /** False when the host screen already states the boundary above the panel. */
    showBoundaryNote: Boolean = true,
    providerSettings: AiProviderSettings = AiProviderSettings(),
    onOpenProviderSettings: () -> Unit = {}
) {
    val keyboardController = LocalSoftwareKeyboardController.current
    val colors = MaterialTheme.colorScheme

    // Scrollable, deliberately: an analysis can easily exceed the height it is given, and a plain
    // Column simply clipped whatever did not fit -- text the user asked for and could never reach.
    // Two parts, and the split is the point: the transcript scrolls, the question box does not.
    // On a real device the whole panel was one scrolling column, so opening the keyboard pushed the
    // field a person had just tapped off the bottom of the sheet -- they were typing into something
    // they could not see.
    Column(modifier = modifier.imePadding()) {
        Column(
            modifier = Modifier
                .weight(1f, fill = false)
                .verticalScroll(rememberScrollState())
                .padding(start = 20.dp, end = 20.dp, top = 20.dp)
        ) {
        if (showHeader) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    imageVector = Icons.Default.Psychology,
                    contentDescription = null,
                    tint = colors.primary,
                    modifier = Modifier.size(22.dp)
                )
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = "Ask Verb",
                    fontSize = 17.sp,
                    fontWeight = FontWeight.Bold,
                    color = colors.onSurface
                )
                Spacer(modifier = Modifier.weight(1f))
                ClearControl(thread, onClearThread, colors.onSurfaceVariant)
            }
            Spacer(modifier = Modifier.height(14.dp))
        } else if (thread.isNotEmpty()) {
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                ClearControl(thread, onClearThread, colors.onSurfaceVariant)
            }
        }

        // The conversation, oldest first, so the newest answer sits closest to the input the user
        // is about to type into next. Each answer rode its own evidence block.
        thread.forEach { exchange ->
            Text(
                text = exchange.question,
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold,
                color = colors.onSurfaceVariant,
                modifier = Modifier.padding(bottom = 2.dp)
            )
            Text(
                text = AssistMarkdown.render(exchange.answer, colors.surfaceVariant),
                fontSize = 14.sp,
                lineHeight = 21.sp,
                color = colors.onSurface,
                modifier = Modifier.padding(bottom = 12.dp)
            )
        }

        // The unprompted explanation has no question, so it is not in the thread. Rendering it
        // unconditionally printed the newest answer twice, once here and once above.
        val standaloneAnswer = aiExplanation
            ?.takeIf { it.isNotBlank() && it != thread.lastOrNull()?.answer }
        if (standaloneAnswer != null) {
            Text(
                text = AssistMarkdown.render(standaloneAnswer, colors.surfaceVariant),
                fontSize = 14.sp,
                lineHeight = 21.sp,
                color = colors.onSurface,
                modifier = Modifier.padding(bottom = 12.dp)
            )
        }

        if (isAiExplaining) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.padding(bottom = 12.dp)
            ) {
                CircularProgressIndicator(
                    modifier = Modifier.size(18.dp),
                    strokeWidth = 2.dp,
                    color = colors.primary
                )
                Spacer(modifier = Modifier.width(10.dp))
                Text(
                    text = "Reading the evidence…",
                    fontSize = 13.sp,
                    color = colors.onSurfaceVariant
                )
            }
        }

        // What the model was given, directly under the answer that names it. The provider received
        // these same facts in the contract's own vocabulary, from this same snapshot.
        evidence?.let {
            Text(
                text = "Based on",
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold,
                color = colors.primary
            )
            AssistEvidence.displayLines(it).forEach { line ->
                Text(
                    text = line,
                    fontSize = 12.sp,
                    color = colors.onSurfaceVariant,
                    modifier = Modifier.padding(top = 2.dp)
                )
            }
            Spacer(modifier = Modifier.height(14.dp))
        }

        // No provider, no answers. Said here rather than discovered by tapping Send and reading a
        // raw exception in the answer slot -- the screen this panel replaced gated submission on
        // provider readiness, and promoting the panel must not quietly drop that.
        if (!providerSettings.isReady) {
            Text(
                text = "No model provider is configured, so nothing can answer yet. Verb's own " +
                    "evidence is still collected either way.",
                fontSize = 13.sp,
                color = colors.onSurfaceVariant,
                modifier = Modifier
                    .padding(bottom = 4.dp)
                    .testTag("assist_provider_missing")
            )
            TextButton(
                onClick = onOpenProviderSettings,
                modifier = Modifier
                    .padding(bottom = 8.dp)
                    .testTag("btn_assist_configure_provider")
            ) {
                Text("Configure provider", fontSize = 13.sp, color = colors.primary)
            }
        } else if (thread.isEmpty() && standaloneAnswer == null && !isAiExplaining) {
            // Nothing said yet: say what this surface can answer rather than showing a bare box. A
            // screen with nothing on it is a puzzle, not restraint.
            if (showBoundaryNote) Text(
                text = "Verb answers from what it observed itself — the session, the shell's " +
                    "command boundaries and your agent sessions. Your command text and terminal " +
                    "output are never sent.",
                fontSize = 13.sp,
                color = colors.onSurfaceVariant,
                modifier = Modifier
                    .padding(bottom = 12.dp)
                    .testTag("assist_boundary_note")
            )

            TextButton(
                onClick = onExplain,
                modifier = Modifier
                    .padding(bottom = 8.dp)
                    .testTag("btn_retry_ai_explain")
            ) {
                Text("Explain what just happened", fontSize = 13.sp, color = colors.primary)
            }
        }

        }

        // Pinned beneath the transcript, where the thread has just been read.
        var question by rememberSaveable { mutableStateOf("") }
        val canSend = providerSettings.isReady && question.isNotBlank() && !isAiExplaining
        fun submit() {
            onAsk(question)
            question = ""
            keyboardController?.hide()
        }
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 20.dp, end = 20.dp, top = 8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            BasicTextField(
                value = question,
                onValueChange = { question = it },
                modifier = Modifier
                    .weight(1f)
                    .height(42.dp)
                    .clip(RoundedCornerShape(10.dp))
                    .background(colors.surfaceVariant)
                    .border(1.dp, colors.outlineVariant, RoundedCornerShape(10.dp))
                    .padding(horizontal = 12.dp, vertical = 10.dp)
                    .testTag("terminal_ai_question_field"),
                textStyle = TextStyle(fontSize = 14.sp, color = colors.onSurface),
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                keyboardActions = KeyboardActions(
                    onSend = { if (canSend) submit() }
                ),
                decorationBox = { inner ->
                    if (question.isEmpty()) {
                        Text(
                            if (thread.isEmpty()) "Ask about this session…" else "Ask a follow-up…",
                            fontSize = 14.sp,
                            color = colors.onSurfaceVariant
                        )
                    }
                    inner()
                }
            )
            Spacer(modifier = Modifier.width(8.dp))
            IconButton(
                onClick = ::submit,
                enabled = canSend,
                modifier = Modifier.size(48.dp).testTag("btn_ask_terminal_ai")
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.Send,
                    contentDescription = "Ask",
                    tint = if (canSend) {
                        colors.primary
                    } else {
                        colors.onSurfaceVariant
                    }
                )
            }
        }
        Spacer(modifier = Modifier.height(8.dp))
    }
}

/**
 * Starting over is the user's explicit move. Closing a sheet is not: a swipe down happens by
 * accident, and a conversation lost to one does not come back.
 */
@Composable
private fun ClearControl(
    thread: List<TerminalAiExchange>,
    onClearThread: () -> Unit,
    tint: Color
) {
    if (thread.isEmpty()) return
    Text(
        text = "Clear",
        fontSize = 13.sp,
        color = tint,
        modifier = Modifier
            .clickable { onClearThread() }
            .padding(horizontal = 8.dp, vertical = 4.dp)
            .testTag("btn_clear_ai_thread")
    )
}
