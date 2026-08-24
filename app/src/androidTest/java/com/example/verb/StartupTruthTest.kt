package com.example.verb

import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.example.MainActivity
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Guards two cold-start regressions that unit tests cannot see because they never construct the
 * real Activity/ViewModel/Compose pipeline together:
 *
 * 1. Heavy work in VerbViewModel's constructor (a property initializer, or a call inside init{})
 *    blocks the first frame. Unit tests exercise the ViewModel in isolation and never notice; on
 *    device this measured as a 12s+ cold start with 1400+ skipped frames.
 *
 * 2. A background refresh that never actually runs -- e.g. an edit that lands in the wrong
 *    function because two functions share identical anchor text -- leaves state flows at their
 *    initial empty value forever. Unit tests that construct the ViewModel and immediately assert
 *    on its state can still pass, because they don't wait for the async path the way a real user
 *    does. That exact bug shipped once in this codebase and was only caught by watching the
 *    agents surface on a real device stay empty.
 */
@RunWith(AndroidJUnit4::class)
class StartupTruthTest {

    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    /** The terminal is the workspace root; if the constructor blocks the main thread, it never draws. */
    @Test
    fun terminalWorkspaceRendersWithoutBlockingTheMainThread() {
        composeRule.waitUntil(timeoutMillis = COLD_START_BUDGET_MS) {
            composeRule.onAllNodesWithTag("verb_session_status").fetchSemanticsNodes().isNotEmpty()
        }
    }

    /**
     * RuntimeProfiles.all is static data -- Claude Code's card exists in it regardless of whether
     * the device has any guest userland installed. If runtimeProfileReports is still the initial
     * empty list once this budget expires, the background refresh that is supposed to populate it
     * either never ran or is not wired to this screen.
     *
     * Reached the way a user reaches it now that there is no tab bar: the workspace's Verb chip, then
     * the named task. That also makes this a live check that the sheet's rows really navigate.
     */
    @Test
    fun agentsSurfacePopulatesFromTheBackgroundRefresh() {
        composeRule.onNodeWithTag("verb_sheet_trigger").performClick()
        composeRule.onNodeWithTag("verb_task_agents").performClick()
        composeRule.waitUntil(timeoutMillis = PROFILE_REFRESH_BUDGET_MS) {
            composeRule.onAllNodesWithTag("agent_claude_code").fetchSemanticsNodes().isNotEmpty()
        }
    }

    private companion object {
        const val COLD_START_BUDGET_MS = 6_000L
        const val PROFILE_REFRESH_BUDGET_MS = 20_000L
    }
}
