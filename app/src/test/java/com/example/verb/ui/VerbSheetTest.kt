package com.example.verb.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import com.example.verb.viewmodel.VerbTask
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The one surface that replaced four tabs.
 *
 * What is being protected here is reachability. An information-architecture change that quietly drops
 * a capability is a regression wearing a redesign's clothes, so every group the old tabs held is
 * asserted findable by a word a person would actually type.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class VerbSheetTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private fun setSheet(onOpenTask: (VerbTask) -> Unit = {}, onDismiss: () -> Unit = {}) {
        composeTestRule.setContent {
            VerbSheet(onDismiss = onDismiss, onOpenTask = onOpenTask)
        }
    }

    @Test
    fun `the sheet lists its tasks before anything is typed`() {
        setSheet()
        composeTestRule.onNodeWithTag("verb_sheet_search").assertIsDisplayed()
        composeTestRule.onNodeWithText(VerbTask.ASK_VERB.title).assertIsDisplayed()
        composeTestRule.onNodeWithText(VerbTask.AGENTS.title).assertIsDisplayed()
    }

    @Test
    fun `typing narrows the list`() {
        setSheet()
        composeTestRule.onNodeWithTag("verb_sheet_search").performTextReplacement("resume")

        composeTestRule.onNodeWithText(VerbTask.SESSIONS.title).assertIsDisplayed()
        composeTestRule.onNodeWithText(VerbTask.SYSTEM.title).assertDoesNotExist()
    }

    @Test
    fun `search does not care about case`() {
        setSheet()
        composeTestRule.onNodeWithTag("verb_sheet_search").performTextReplacement("CLAUDE")
        composeTestRule.onNodeWithText(VerbTask.AGENTS.title).assertIsDisplayed()
    }

    /** Naming the absence, rather than leaving a blank panel that reads as a bug. */
    @Test
    fun `a query nothing answers says so instead of going blank`() {
        setSheet()
        composeTestRule.onNodeWithTag("verb_sheet_search").performTextReplacement("do a barrel roll")

        composeTestRule.onNodeWithTag("verb_sheet_no_matches").assertIsDisplayed()
        composeTestRule.onNodeWithText(VerbTask.ASK_VERB.title).assertDoesNotExist()
    }

    @Test
    fun `choosing a row opens that task`() {
        var opened: VerbTask? = null
        setSheet(onOpenTask = { opened = it })

        composeTestRule.onNodeWithTag("verb_task_sessions").performClick()

        assertEquals(VerbTask.SESSIONS, opened)
    }

    /**
     * One row per capability group the tabs used to own. Written as a table so a future task that
     * disappears from the sheet fails here by name rather than being noticed on a device.
     *
     * Each is reached by typing, which is both how a person actually finds a task and the only way
     * to assert on a row a [androidx.compose.foundation.lazy.LazyColumn] has not composed yet.
     */
    @Test
    fun `every capability group the tabs held is reachable by name`() {
        setSheet()
        listOf(
            // Ask + Assistant
            VerbTask.ASK_VERB,
            // Agents, and its session recovery actions
            VerbTask.AGENTS,
            VerbTask.SESSIONS,
            // What the terminal's overflow menu alone used to reach
            VerbTask.EVIDENCE,
            VerbTask.RUNS,
            // Everything System held
            VerbTask.PROVIDER,
            VerbTask.RUNTIMES,
            VerbTask.AGENT_RUNTIME,
            VerbTask.WORKING_WORLD,
            VerbTask.CONTINUITY,
            VerbTask.SYSTEM
        ).forEach { task ->
            composeTestRule.onNodeWithTag("verb_sheet_search").performTextReplacement(task.title)
            composeTestRule
                .onNodeWithTag("verb_task_${task.name.lowercase()}")
                .assertExists("${task.name} cannot be reached by typing its own name")
        }
    }
}
