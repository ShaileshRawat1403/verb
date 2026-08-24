package com.example.verb.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import com.example.verb.ai.AiAssistantState
import com.example.verb.ai.AiProviderSettings
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The merged Ask Verb surface.
 *
 * There used to be two tabs here that looked the same and answered differently: one deterministic,
 * one a model. A user could not tell which would understand their question, which is precisely the
 * ambiguity the product exists to remove. These tests hold the merge's two guarantees -- the
 * deterministic stage is what you land on, and the model stage is visibly a different kind of claim.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class AskVerbScreenTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private fun setScreen() {
        composeTestRule.setContent {
            AskVerbScreen(
                queryInput = "",
                isExecuting = false,
                currentResult = null,
                historyList = emptyList(),
                confirmationPending = null,
                onQueryChange = {},
                onSubmitQuery = {},
                onSubmitIntent = {},
                onConfirmAction = {},
                onDismissConfirmation = {},
                onOpenTerminal = {},
                onInspectText = {},
                providerSettings = AiProviderSettings(),
                assistantPrompt = "",
                assistantState = AiAssistantState.Idle,
                onAssistantPromptChange = {},
                onSubmitAssistantPrompt = {},
                onOpenProviderSettings = {}
            )
        }
    }

    /**
     * `docs/PRD.md`'s architectural rule is an ordering: observed fact, then interpretation. The
     * default stage is the deterministic one because a model must never be the first thing that
     * answers a question about the user's own machine.
     */
    @Test
    fun `the deterministic stage is what the user lands on`() {
        setScreen()
        composeTestRule.onNodeWithTag("ask_input_field").assertIsDisplayed()
        composeTestRule.onNodeWithTag("ask_interpretation_boundary").assertDoesNotExist()
    }

    @Test
    fun `both stages are reachable and only one is shown at a time`() {
        setScreen()

        composeTestRule.onNodeWithTag("ask_stage_interpretation").performClick()
        composeTestRule.onNodeWithTag("ask_interpretation_boundary").assertIsDisplayed()
        composeTestRule.onNodeWithTag("ask_input_field").assertDoesNotExist()

        composeTestRule.onNodeWithTag("ask_stage_actions").performClick()
        composeTestRule.onNodeWithTag("ask_input_field").assertIsDisplayed()
        composeTestRule.onNodeWithTag("ask_interpretation_boundary").assertDoesNotExist()
    }

    /**
     * The boundary is stated before the input, not after it: a person deciding whether to type
     * something needs to know where it goes while they are still deciding.
     */
    @Test
    fun `the interpretation stage names what leaves the device`() {
        setScreen()
        composeTestRule.onNodeWithTag("ask_stage_interpretation").performClick()
        composeTestRule.onNodeWithTag("ask_interpretation_boundary").assertIsDisplayed()
    }

    @Test
    fun `the whole surface is one destination rather than two`() {
        setScreen()
        composeTestRule.onNodeWithTag("ask_verb_screen").assertIsDisplayed()
        composeTestRule.onNodeWithTag("ask_stage_actions").assertIsDisplayed()
        composeTestRule.onNodeWithTag("ask_stage_interpretation").assertIsDisplayed()
    }
}
