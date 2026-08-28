package com.example.verb.ui

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.assertIsDisplayed
import com.example.verb.terminal.SwitchingTerminalRuntime
import com.example.verb.terminal.TerminalRuntime
import com.example.verb.terminal.VerbTerminal
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * A terminal whose environment has changed underneath it, wrapped around a real runtime so only the
 * one answer under test is invented.
 */
private class WithPendingChange(
    delegate: VerbTerminal,
    pending: Boolean
) : VerbTerminal by delegate {
    override val pendingEnvironmentChange: StateFlow<Boolean> = MutableStateFlow(pending)
}

/**
 * Second half of the same regression as `TerminalScreenRenderTargetTest`.
 *
 * `pendingEnvironmentChange` is how Verb tells someone that selecting a project, finishing the
 * bootstrap or switching an Agent Runtime will not take effect until a new shell -- stated instead
 * of acted on, so a live session is never killed out from under them. The workspace used to recover
 * it with an `as? TerminalRuntime` cast, which the multi-session facade fails, so the banner and its
 * Restart button silently stopped appearing for every user. The property under test is that the
 * screen asks the terminal rather than its type, and that the answer follows the session in front.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TerminalFacadeReachTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun runtime(name: String): VerbTerminal =
        TerminalRuntime(workingDir = temporaryFolder.newFolder(name), useFakeForTesting = true)

    @Test
    fun `a queued environment change is announced through the facade`() {
        val active = MutableStateFlow<VerbTerminal?>(WithPendingChange(runtime("pending"), true))
        val facade = SwitchingTerminalRuntime(CoroutineScope(Dispatchers.Unconfined), active)

        composeTestRule.setContent {
            TerminalScreen(
                terminalOutput = "",
                terminalRuntime = facade,
                onSendCommand = {},
                onSendKey = {},
                onSendText = {},
                onClearTerminal = {},
                onInspectText = {},
                onSubmitIntent = {}
            )
        }

        composeTestRule.onNodeWithTag("pending_environment_change").assertIsDisplayed()
        composeTestRule.onNodeWithTag("apply_environment_restart").assertIsDisplayed()
    }

    /**
     * A pending change belongs to the session it was made against, so switching to a terminal with
     * nothing queued must take the offer away rather than carry it across.
     */
    @Test
    fun `the announcement follows the terminal in front`() {
        val pending = WithPendingChange(runtime("queued"), true)
        val settled = WithPendingChange(runtime("settled"), false)
        val active = MutableStateFlow<VerbTerminal?>(pending)
        val facade = SwitchingTerminalRuntime(CoroutineScope(Dispatchers.Unconfined), active)

        assertTrue(facade.pendingEnvironmentChange.value)

        active.value = settled
        assertFalse(
            "switching to a terminal with nothing queued must not inherit the other one's offer",
            facade.pendingEnvironmentChange.value
        )

        active.value = pending
        assertTrue(facade.pendingEnvironmentChange.value)
    }
}
