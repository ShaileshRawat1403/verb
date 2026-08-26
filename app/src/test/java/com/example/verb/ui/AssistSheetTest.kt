package com.example.verb.ui

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.example.verb.ai.AiProviderConfig
import com.example.verb.ai.AiProviderId
import com.example.verb.ai.AiProviderSettings
import com.example.verb.session.VerbSessionState
import com.example.verb.terminal.AgentWorkFact
import com.example.verb.terminal.TerminalAiExchange
import com.example.verb.terminal.TerminalEvidence
import com.example.verb.terminal.TerminalSessionState
import java.time.Instant
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The assistant surface itself. `TerminalAiHelperTest` proves what crosses the provider boundary;
 * these prove what the person actually sees, which is where the defects this file was written for
 * lived — a duplicated answer, an evidence panel in the wrong vocabulary, and a sheet that opened
 * itself over another screen.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class AssistSheetTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private val evidence = TerminalEvidence(
        sessionState = TerminalSessionState.RUNNING,
        workingDirectoryKnown = true,
        shellIntegrationActive = true,
        agentWork = listOf(
            AgentWorkFact("Claude Code", VerbSessionState.LIVE, Instant.now(), "claude")
        )
    )

    /** A configured provider, so the tests exercise the surface rather than the not-set-up state. */
    private val readyProvider = AiProviderSettings(
        config = AiProviderConfig(AiProviderId.OPENAI, "test-model", "https://api.openai.com/v1"),
        hasApiKey = true
    )

    private fun panel(
        aiExplanation: String? = null,
        thread: List<TerminalAiExchange> = emptyList(),
        evidence: TerminalEvidence? = null,
        isAiExplaining: Boolean = false,
        onClearThread: () -> Unit = {},
        onAsk: (String) -> Unit = {}
    ) {
        composeTestRule.setContent {
            AssistPanel(
                aiExplanation = aiExplanation,
                isAiExplaining = isAiExplaining,
                evidence = evidence,
                thread = thread,
                onAsk = onAsk,
                onExplain = {},
                onClearThread = onClearThread,
                providerSettings = readyProvider
            )
        }
    }

    /**
     * The answer to the newest question lives in the thread *and* was the last explanation. Drawing
     * both printed it twice, one above the other, which reads as the assistant repeating itself.
     */
    @Test
    fun theNewestAnswerIsDrawnOnceEvenThoughItIsBothTheThreadTailAndTheExplanation() {
        panel(
            aiExplanation = "The build never reported an exit code.",
            thread = listOf(
                TerminalAiExchange("Why did it stop?", "The build never reported an exit code.")
            ),
            evidence = evidence
        )

        composeTestRule.onAllNodesWithText("The build never reported an exit code.")
            .assertCountEquals(1)
    }

    /** An unprompted explanation has no question, so it is not in the thread and must still show. */
    @Test
    fun anUnpromptedExplanationIsShownEvenWithNoThread() {
        panel(aiExplanation = "Nothing has run in this session yet.", evidence = evidence)

        composeTestRule.onNodeWithText("Nothing has run in this session yet.").assertExists()
    }

    /**
     * The answer names its evidence and the evidence sits beside it, in the words
     * `docs/UX_FOUNDATION.md` requires on screen rather than the contract's own spelling.
     */
    @Test
    fun theEvidencePanelReadsBackInPlainLanguage() {
        panel(aiExplanation = "An answer.", evidence = evidence)

        composeTestRule.onNodeWithText("Based on").assertExists()
        composeTestRule.onAllNodesWithText("RUNNING", substring = true).assertCountEquals(0)
        composeTestRule.onAllNodesWithText("LIVE", substring = true).assertCountEquals(0)
        composeTestRule.onAllNodesWithText("terminal session running", substring = true)
            .assertCountEquals(1)
    }

    /** Closing is not clearing. Clearing is a labelled control the user has to choose. */
    @Test
    fun clearingTheThreadIsAnExplicitControlAndOnlyAppearsWhenThereIsAThread() {
        var cleared = 0
        panel(
            aiExplanation = "An answer.",
            thread = listOf(TerminalAiExchange("A question", "An answer.")),
            onClearThread = { cleared++ }
        )

        composeTestRule.onNodeWithTag("btn_clear_ai_thread").performClick()
        assertEquals(1, cleared)
    }

    @Test
    fun thereIsNothingToClearBeforeAConversationStarts() {
        panel(aiExplanation = "An unprompted explanation.")

        composeTestRule.onAllNodesWithText("Clear").assertCountEquals(0)
    }

    /** A request in flight must not accept a second one; the send control says so by being off. */
    @Test
    fun theSendControlIsDisabledWhileAnAnswerIsInFlight() {
        var asked = 0
        panel(isAiExplaining = true, evidence = evidence, onAsk = { asked++ })

        composeTestRule.onNodeWithTag("btn_ask_terminal_ai").performClick()
        assertEquals(0, asked)
    }

    /**
     * The terminal's sheet must not open itself. It used to appear whenever the shared assistant
     * state went non-null, so asking from the Ask Verb screen slid the terminal's sheet up over it
     * with the same conversation inside — a surface Verb opened on its own.
     */
    @Test
    fun theTerminalSheetDoesNotOpenItselfWhenTheSharedAssistantStateFills() {
        composeTestRule.setContent {
            TerminalScreen(
                terminalOutput = "",
                terminalRuntime = null,
                onSendCommand = {},
                onSendKey = {},
                onSendText = {},
                onClearTerminal = {},
                onInspectText = {},
                onSubmitIntent = {},
                aiExplanation = "An answer produced by the other host.",
                isAiExplaining = false,
                terminalAiEvidence = evidence,
                terminalAiThread = listOf(
                    TerminalAiExchange("Asked elsewhere", "An answer produced by the other host.")
                ),
                aiProviderSettings = readyProvider
            )
        }

        composeTestRule.onNodeWithTag("terminal_ai_explanation_sheet").assertDoesNotExist()
        composeTestRule.onAllNodesWithText("An answer produced by the other host.")
            .assertCountEquals(0)
    }
}
