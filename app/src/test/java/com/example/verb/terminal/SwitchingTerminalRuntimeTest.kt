package com.example.verb.terminal

import com.example.verb.session.VerbTerminalSessionHolder
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/**
 * The facade that lets "a project has sessions" cost the rest of the app nothing.
 *
 * Ninety-odd call sites ask a terminal what it is doing. Rather than teaching each of them which
 * session to ask about, they keep asking the same object and it answers about whichever session is
 * in front. So the property under test is not "it delegates" -- it is that a *switch changes the
 * answer*, because that is what the screens depend on to re-render.
 */
class SwitchingTerminalRuntimeTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @After
    fun tearDown() {
        VerbTerminalSessionHolder.resetForTests()
    }

    private fun runtime(name: String): TerminalRuntime =
        TerminalRuntime(
            workingDir = temporaryFolder.newFolder(name),
            useFakeForTesting = true
        )

    @Test
    fun `reads follow whichever session is in front`() = runTest {
        val facade = SwitchingTerminalRuntime(
            scope = TestScope(StandardTestDispatcher(testScheduler)),
            active = VerbTerminalSessionHolder.activeRuntime
        )
        val first = VerbTerminalSessionHolder.getOrCreateActive { runtime("first") }
        val firstId = VerbTerminalSessionHolder.activeId.value!!
        val second = VerbTerminalSessionHolder.runtimeOf(
            VerbTerminalSessionHolder.open { runtime("second") }!!
        )!!
        advanceUntilIdle()

        first.sendCommand("i-am-first")
        second.sendCommand("i-am-second")
        advanceUntilIdle()

        // Second is in front, because opening a terminal puts you in it.
        assertTrue(facade.terminalOutput.value.contains("i-am-second"))
        assertFalse(
            "the terminal you are not looking at must not bleed into the one you are",
            facade.terminalOutput.value.contains("i-am-first")
        )

        VerbTerminalSessionHolder.activate(firstId)
        advanceUntilIdle()

        assertTrue(facade.terminalOutput.value.contains("i-am-first"))
        assertFalse(facade.terminalOutput.value.contains("i-am-second"))
    }

    /** Each terminal keeps its own directory, which is the point of having a second one. */
    @Test
    fun `the launch directory is the one in front, not the first one opened`() = runTest {
        val facade = SwitchingTerminalRuntime(
            scope = TestScope(StandardTestDispatcher(testScheduler)),
            active = VerbTerminalSessionHolder.activeRuntime
        )
        val first = VerbTerminalSessionHolder.getOrCreateActive { runtime("alpha") }
        val firstId = VerbTerminalSessionHolder.activeId.value!!
        val secondId = VerbTerminalSessionHolder.open { runtime("beta") }!!
        val second = VerbTerminalSessionHolder.runtimeOf(secondId)!!
        advanceUntilIdle()

        assertEquals(second.launchWorkingDirectory, facade.launchWorkingDirectory)

        VerbTerminalSessionHolder.activate(firstId)
        assertEquals(first.launchWorkingDirectory, facade.launchWorkingDirectory)
    }

    /**
     * Typing reaches the terminal you are looking at. There is no ambiguity to resolve here --
     * a person cannot type into a terminal that is not in front -- so writes go to the front and
     * nowhere else.
     */
    @Test
    fun `writes reach only the session in front`() = runTest {
        val facade = SwitchingTerminalRuntime(
            scope = TestScope(StandardTestDispatcher(testScheduler)),
            active = VerbTerminalSessionHolder.activeRuntime
        )
        val first = VerbTerminalSessionHolder.getOrCreateActive { runtime("one") }
        val secondId = VerbTerminalSessionHolder.open { runtime("two") }!!
        val second = VerbTerminalSessionHolder.runtimeOf(secondId)!!
        advanceUntilIdle()

        facade.sendCommand("typed-once")
        advanceUntilIdle()

        assertTrue(second.terminalOutput.value.contains("typed-once"))
        assertFalse(first.terminalOutput.value.contains("typed-once"))
    }

    /**
     * With no session at all, reads report the quiet defaults and writes are dropped. A keystroke
     * aimed at a terminal that does not exist has nowhere to be delivered later, so queueing it
     * would only mean surprising someone with it afterwards.
     */
    @Test
    fun `with no session, reads are quiet and writes are dropped rather than queued`() = runTest {
        val facade = SwitchingTerminalRuntime(
            scope = TestScope(StandardTestDispatcher(testScheduler)),
            active = VerbTerminalSessionHolder.activeRuntime
        )
        advanceUntilIdle()

        assertEquals("", facade.terminalOutput.value)
        assertFalse(facade.isSessionActive.value)
        assertEquals(emptyList<CommandExecutionRecord>(), facade.commandHistory.value)

        facade.sendCommand("into-the-void")
        advanceUntilIdle()

        val opened = VerbTerminalSessionHolder.getOrCreateActive { runtime("later") }
        advanceUntilIdle()
        assertFalse(
            "a dropped keystroke must not reappear in the next terminal",
            opened.terminalOutput.value.contains("into-the-void")
        )
    }
}
