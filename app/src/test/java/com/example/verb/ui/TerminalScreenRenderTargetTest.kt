package com.example.verb.ui

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.assertIsDisplayed
import com.example.verb.session.VerbTerminalSessionHolder
import com.example.verb.terminal.SwitchingTerminalRuntime
import com.example.verb.terminal.TerminalRuntime
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The workspace must reach the terminal it is actually looking at.
 *
 * A project has sessions, so what `TerminalScreen` is handed is a facade for whichever one is in
 * front. It used to recover the real canvas with `is TerminalRuntime -> termuxDelegate`, which the
 * facade matches no better than any other unrelated type -- so the screen silently rendered the
 * transcript fallback meant for headless tests, and pinch zoom, native scrolling, native selection
 * and the real cursor all disappeared with the view they live in. The property under test is that
 * the render target is *asked for*, follows the switch, and never costs an inactive session its PTY.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TerminalScreenRenderTargetTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @After
    fun tearDown() {
        VerbTerminalSessionHolder.resetForTests()
    }

    /** A production runtime -- not the JVM fake, whose whole point is having no canvas to mount. */
    private fun runtime(name: String) = TerminalRuntime(workingDir = temporaryFolder.newFolder(name))

    private fun facade() = SwitchingTerminalRuntime(
        scope = CoroutineScope(Dispatchers.Unconfined),
        active = VerbTerminalSessionHolder.activeRuntime
    )

    @Test
    fun `the facade names the active session's own canvas, and follows a switch`() {
        val facade = facade()
        val first = VerbTerminalSessionHolder.getOrCreateActive { runtime("first") }
        val firstId = VerbTerminalSessionHolder.activeId.value!!
        val secondId = VerbTerminalSessionHolder.open { runtime("second") }!!
        val second = VerbTerminalSessionHolder.runtimeOf(secondId)!!

        assertNotNull("a production runtime must have a real canvas to mount", first.termuxDelegate)
        assertNotSame(
            "two terminals are two PTYs and therefore two canvases",
            first.termuxDelegate,
            second.termuxDelegate
        )

        assertSame(
            "opening the second terminal must move the canvas to it",
            second.termuxDelegate,
            facade.renderTarget.value
        )

        VerbTerminalSessionHolder.activate(firstId)
        assertSame(
            "switching back must mount the first terminal's canvas, not repaint the second's",
            first.termuxDelegate,
            facade.renderTarget.value
        )

        assertNotNull(
            "the terminal switched away from keeps its canvas -- and its PTY -- alive",
            second.termuxDelegate
        )
        assertTrue(secondId in VerbTerminalSessionHolder.sessionIds.value)
    }

    /**
     * The production screen, composed exactly as `MainActivity` composes it: given the facade, it
     * must mount the real `com.termux.view.TerminalView` rather than the Compose transcript.
     */
    @Test
    fun `TerminalScreen mounts the real terminal view when handed the facade`() {
        val facade = facade()
        VerbTerminalSessionHolder.getOrCreateActive { runtime("only") }

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

        composeTestRule.onNodeWithTag("termux_terminal_view").assertIsDisplayed()
        composeTestRule.onNodeWithTag("terminal_output_text").assertDoesNotExist()
    }
}
