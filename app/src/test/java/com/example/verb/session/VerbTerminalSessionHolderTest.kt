package com.example.verb.session

import com.example.verb.terminal.TerminalRuntime
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.assertSame
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class VerbTerminalSessionHolderTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @After
    fun tearDown() {
        VerbTerminalSessionHolder.resetForTests()
    }

    private fun newRuntime(): TerminalRuntime {
        val filesDir = temporaryFolder.newFolder("files-${System.nanoTime()}")
        return TerminalRuntime(workingDir = filesDir, useFakeForTesting = true)
    }

    @Test
    fun `getOrCreateActive returns the same instance on a second call`() {
        val first = VerbTerminalSessionHolder.getOrCreateActive { newRuntime() }
        val second = VerbTerminalSessionHolder.getOrCreateActive { newRuntime() }

        assertSame("a second VerbViewModel must reattach, not spawn a duplicate session", first, second)
    }

    @Test
    fun `the factory does not run once a runtime already exists`() {
        VerbTerminalSessionHolder.getOrCreateActive { newRuntime() }
        var factoryRan = false

        VerbTerminalSessionHolder.getOrCreateActive {
            factoryRan = true
            newRuntime()
        }

        assertFalse("reattaching must not construct a second TerminalRuntime", factoryRan)
    }

    @Test
    fun `resetForTests clears the held instance`() {
        val first = VerbTerminalSessionHolder.getOrCreateActive { newRuntime() }

        VerbTerminalSessionHolder.resetForTests()
        val second = VerbTerminalSessionHolder.getOrCreateActive { newRuntime() }

        assertNotSame(first, second)
    }

    /**
     * The whole point of the change: an agent in one terminal, your own commands in another.
     * Before this, those were the same slot and you had to choose.
     */
    @Test
    fun `a second session opens alongside the first and becomes active`() {
        val first = VerbTerminalSessionHolder.getOrCreateActive { newRuntime() }
        val firstId = VerbTerminalSessionHolder.activeId.value

        val secondId = VerbTerminalSessionHolder.open { newRuntime() }

        assertEquals(2, VerbTerminalSessionHolder.sessionIds.value.size)
        assertEquals("opening a terminal puts you in it", secondId, VerbTerminalSessionHolder.activeId.value)
        assertNotSame(first, VerbTerminalSessionHolder.activeRuntime.value)
        assertSame(first, VerbTerminalSessionHolder.runtimeOf(firstId!!))
    }

    @Test
    fun `switching brings a session to the front without disturbing the others`() {
        VerbTerminalSessionHolder.getOrCreateActive { newRuntime() }
        val firstId = VerbTerminalSessionHolder.activeId.value!!
        VerbTerminalSessionHolder.open { newRuntime() }

        VerbTerminalSessionHolder.activate(firstId)

        assertEquals(firstId, VerbTerminalSessionHolder.activeId.value)
        assertEquals("switching must not close anything", 2, VerbTerminalSessionHolder.sessionIds.value.size)
    }

    /** A workspace with no terminal is a screen with nothing to do. */
    @Test
    fun `the last session cannot be closed`() {
        VerbTerminalSessionHolder.getOrCreateActive { newRuntime() }
        val onlyId = VerbTerminalSessionHolder.activeId.value!!

        assertFalse(VerbTerminalSessionHolder.close(onlyId))
        assertEquals(1, VerbTerminalSessionHolder.sessionIds.value.size)
    }

    @Test
    fun `closing the active session hands the front to what remains`() {
        VerbTerminalSessionHolder.getOrCreateActive { newRuntime() }
        val firstId = VerbTerminalSessionHolder.activeId.value!!
        val secondId = VerbTerminalSessionHolder.open { newRuntime() }!!

        assertTrue(VerbTerminalSessionHolder.close(secondId))

        assertEquals(firstId, VerbTerminalSessionHolder.activeId.value)
        assertNull(VerbTerminalSessionHolder.runtimeOf(secondId))
    }

    /**
     * The ceiling is stated, not discovered by running out of memory. The shells are cheap; the
     * agents inside them are not.
     */
    @Test
    fun `opening past the ceiling reports it instead of failing`() {
        VerbTerminalSessionHolder.getOrCreateActive { newRuntime() }
        repeat(VerbTerminalSessionHolder.MAX_SESSIONS - 1) {
            assertTrue(VerbTerminalSessionHolder.open { newRuntime() } != null)
        }

        assertNull("past the ceiling must report, not throw", VerbTerminalSessionHolder.open { newRuntime() })
        assertEquals(VerbTerminalSessionHolder.MAX_SESSIONS, VerbTerminalSessionHolder.sessionIds.value.size)
    }

    /**
     * An agent occupying terminal two must not make terminal one look occupied. With one terminal
     * those were the same statement; with several they are not.
     */
    @Test
    fun `a foreground agent belongs to its own session`() {
        VerbTerminalSessionHolder.getOrCreateActive { newRuntime() }
        val shellId = VerbTerminalSessionHolder.activeId.value!!
        VerbTerminalSessionHolder.open { newRuntime() }

        VerbTerminalSessionHolder.claimForeground("claude", emptySet())
        VerbTerminalSessionHolder.activate(shellId)

        assertEquals("the claim followed the session it was made in", "claude",
            VerbTerminalSessionHolder.foregroundAgent())
        VerbTerminalSessionHolder.releaseForeground("claude")
        assertNull(VerbTerminalSessionHolder.foregroundAgent())
    }
}
