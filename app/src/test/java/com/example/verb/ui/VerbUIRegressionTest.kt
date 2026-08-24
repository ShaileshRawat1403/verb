package com.example.verb.ui

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import com.example.verb.model.ActionResult
import com.example.verb.model.VerbIntent
import com.example.verb.ai.AiAssistantState
import com.example.verb.ai.AiProviderConfig
import com.example.verb.ai.AiProviderId
import com.example.verb.ai.AiProviderSettings
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class VerbUIRegressionTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    // The three tests that used to sit here drove VerbNaturalLanguageSheet, a second
    // natural-language surface reachable only from the terminal header. It has been retired: it
    // asked for a sentence in a place neither Ask nor the Assistant knew about, so a user had three
    // sentence boxes and no way to tell which one would understand them. Its one entry point now
    // opens the Verb sheet, and typed intent resolution lives in the merged Ask Verb surface.
    //
    // The guards those tests asserted are behaviour of IntentEngine and ActionRegistry, not of that
    // composable, and they are still covered where they belong -- `VerbLogicTest`'s
    // `supported intent accepted`, `unsupported intent rejected`, and
    // `process stop requires confirmation and retains original intent parameters`. Nothing was
    // weakened to delete a screen.

    @Test
    fun actionResultCardPressingTerminalCallsZeroArgumentCallbackExactlyOnce() {
        var callCount = 0
        composeTestRule.setContent {
            ActionResultCard(
                result = ActionResult(
                    intentId = "test.action",
                    title = "Test Title",
                    summary = "Testing card",
                    isSuccess = true
                ),
                onOpenTerminal = {
                    callCount++
                },
                onInspectText = {}
            )
        }

        composeTestRule.onNodeWithText("Terminal").performClick()
        assertEquals(1, callCount)
    }

    @Test
    fun historyReplaysTheOriginalTypedIntentWithoutNaturalLanguageReparsing() {
        var replayedIntent: VerbIntent? = null
        val replayIntent = VerbIntent(
            id = "file.list",
            name = "List Files",
            parameters = mapOf("path" to "/storage/emulated/0/Download")
        )
        val history = listOf(
            ActionResult(
                intentId = "storage.summary",
                title = "Storage Summary",
                summary = "Current result"
            ),
            ActionResult(
                intentId = replayIntent.id,
                title = "Files in Directory",
                summary = "Previous result",
                originalIntent = replayIntent
            )
        )

        composeTestRule.setContent {
            AskScreen(
                queryInput = "",
                isExecuting = false,
                currentResult = null,
                historyList = history,
                confirmationPending = null,
                onQueryChange = {},
                onSubmitQuery = {},
                onSubmitIntent = { replayedIntent = it },
                onConfirmAction = {},
                onDismissConfirmation = {},
                onOpenTerminal = {},
                onInspectText = {}
            )
        }

        composeTestRule.onNodeWithText("Files in Directory").performClick()

        assertEquals(replayIntent, replayedIntent)
    }

    @Test
    fun resultProvenanceUIRendersDerivedObservedAndExplanationDistinctly() {
        composeTestRule.setContent {
            ActionResultCard(
                result = ActionResult(
                    intentId = "process.info",
                    title = "Process Info",
                    summary = "Details for process",
                    isSuccess = true,
                    derivedData = mapOf("PID" to "1234", "CPU" to "5%"),
                    observedOutput = "ProcessState(pid=1234, state=RUNNING, cpuUsage=5.0%)",
                    explanation = "This process is using 5% CPU."
                ),
                onOpenTerminal = {},
                onInspectText = {}
            )
        }

        // Initially Derived is visible directly
        composeTestRule.onNodeWithText("Derived").assertIsDisplayed()
        composeTestRule.onNodeWithText("PID").assertIsDisplayed()
        composeTestRule.onNodeWithText("1234").assertIsDisplayed()
        composeTestRule.onNodeWithText("CPU").assertIsDisplayed()
        composeTestRule.onNodeWithText("5%").assertIsDisplayed()

        // Toggle Evidence visibility to show Observed and Explanation
        composeTestRule.onNodeWithText("Show Evidence").performClick()

        composeTestRule.onNodeWithText("Observed").assertIsDisplayed()
        composeTestRule.onNodeWithText("ProcessState(pid=1234, state=RUNNING, cpuUsage=5.0%)").assertIsDisplayed()

        composeTestRule.onNodeWithText("Explanation").assertIsDisplayed()
        composeTestRule.onNodeWithText("This process is using 5% CPU.").assertIsDisplayed()

        // Assert shell-prompt framing (e.g. "$ ") is absent from rendered UI
        composeTestRule.onAllNodesWithText("$ ", substring = true).assertCountEquals(0)
        composeTestRule.onAllNodesWithText("# ", substring = true).assertCountEquals(0)
    }

    @Test
    fun assistantDoesNotSubmitUntilProviderAndKeyAreConfigured() {
        var submitCount = 0
        composeTestRule.setContent {
            AssistantScreen(
                providerSettings = AiProviderSettings(),
                prompt = "Explain ls",
                state = AiAssistantState.Idle,
                onPromptChange = {},
                onSubmitPrompt = { submitCount++ },
                onOpenProviderSettings = {}
            )
        }

        composeTestRule.onNodeWithTag("assistant_submit_button").assertIsNotEnabled()
        assertEquals(0, submitCount)
    }

    @Test
    fun assistantEnablesSubmissionOnlyWhenProviderIsReady() {
        composeTestRule.setContent {
            AssistantScreen(
                providerSettings = AiProviderSettings(
                    config = AiProviderConfig(AiProviderId.OPENAI, "test-model", "https://api.openai.com/v1"),
                    hasApiKey = true
                ),
                prompt = "Explain ls",
                state = AiAssistantState.Idle,
                onPromptChange = {},
                onSubmitPrompt = {},
                onOpenProviderSettings = {}
            )
        }

        composeTestRule.onNodeWithTag("assistant_submit_button").assertIsEnabled()
    }
}
