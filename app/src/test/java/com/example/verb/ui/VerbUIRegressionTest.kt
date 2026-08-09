package com.example.verb.ui

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import com.example.verb.model.ActionResult
import com.example.verb.model.VerbIntent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class VerbUIRegressionTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun defaultPromptSubmitsSupportedV0Intent() {
        var submittedIntent: VerbIntent? = null
        composeTestRule.setContent {
            VerbNaturalLanguageSheet(
                onDismiss = {},
                onSubmitIntent = { intent ->
                    submittedIntent = intent
                }
            )
        }

        composeTestRule.onNodeWithTag("verb_submit_intent_button").performClick()

        assertNotNull(submittedIntent)
        assertEquals("file.list", submittedIntent!!.id)
    }

    @Test
    fun unsupportedInputSubmitsTypedIntentUnsupportedIntent() {
        var submittedIntent: VerbIntent? = null
        composeTestRule.setContent {
            VerbNaturalLanguageSheet(
                onDismiss = {},
                onSubmitIntent = { intent ->
                    submittedIntent = intent
                }
            )
        }

        composeTestRule.onNodeWithTag("verb_prompt_input").performTextReplacement("do a barrel roll")
        composeTestRule.onNodeWithTag("verb_submit_intent_button").performClick()

        assertNotNull(submittedIntent)
        assertEquals("unsupported.intent", submittedIntent!!.id)
    }

    @Test
    fun stopProcessSubmitsTypedIntentProcessStopWithPid() {
        var submittedIntent: VerbIntent? = null
        composeTestRule.setContent {
            VerbNaturalLanguageSheet(
                onDismiss = {},
                onSubmitIntent = { intent ->
                    submittedIntent = intent
                }
            )
        }

        composeTestRule.onNodeWithTag("verb_prompt_input").performTextReplacement("stop process 1234")
        composeTestRule.onNodeWithTag("verb_submit_intent_button").performClick()

        assertNotNull(submittedIntent)
        assertEquals("process.stop", submittedIntent!!.id)
        assertEquals("1234", submittedIntent!!.parameters["pid"])
    }

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
}
