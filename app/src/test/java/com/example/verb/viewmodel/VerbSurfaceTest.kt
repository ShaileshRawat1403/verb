package com.example.verb.viewmodel

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The information architecture, asserted rather than described.
 *
 * These are plain JVM tests on purpose. The old five-tab model could only be checked by driving a
 * Compose tree; the replacement is a sealed surface plus one pure transition, so the back chain and
 * the searchability of every capability are testable without an Activity, a ViewModel, a device or a
 * Robolectric shadow. That is the same reason `desktop/src/tui/render.rs` keeps `bar_slots` pure.
 */
class VerbSurfaceTest {

    // ---- back behaviour -----------------------------------------------------------------------

    @Test
    fun `back from the terminal belongs to the system so the app can exit`() {
        assertNull(VerbSurface.None.afterBack(taskOpenedFromSheet = false))
        assertNull(VerbSurface.None.afterBack(taskOpenedFromSheet = true))
    }

    @Test
    fun `back from the sheet returns to the terminal`() {
        assertEquals(VerbSurface.None, VerbSurface.Sheet.afterBack(taskOpenedFromSheet = false))
    }

    @Test
    fun `back from a task opened through the sheet retraces to the sheet`() {
        val task = VerbSurface.Task(VerbTask.SESSIONS)
        assertEquals(VerbSurface.Sheet, task.afterBack(taskOpenedFromSheet = true))
    }

    /**
     * A task can also be opened directly -- from the workspace's first action, or from a link inside
     * another task. Retracing to a sheet in that case would open a surface the user never asked for,
     * which is Verb navigating on its own initiative.
     */
    @Test
    fun `back from a task opened directly returns to the terminal, not to a sheet`() {
        val task = VerbSurface.Task(VerbTask.PROVIDER)
        assertEquals(VerbSurface.None, task.afterBack(taskOpenedFromSheet = false))
    }

    @Test
    fun `the whole chain terminates at the terminal in at most two dismissals`() {
        var surface: VerbSurface? = VerbSurface.Task(VerbTask.ASK_VERB)
        surface = surface!!.afterBack(taskOpenedFromSheet = true)
        assertEquals(VerbSurface.Sheet, surface)
        surface = surface!!.afterBack(taskOpenedFromSheet = false)
        assertEquals(VerbSurface.None, surface)
        assertNull(surface!!.afterBack(taskOpenedFromSheet = false))
    }

    // ---- search -------------------------------------------------------------------------------

    @Test
    fun `an empty query lists everything, so the sheet is browsable before it is searched`() {
        assertEquals(VerbTask.entries.size, VerbTask.entries.count { it.matches("") })
        assertEquals(VerbTask.entries.size, VerbTask.entries.count { it.matches("   ") })
    }

    @Test
    fun `search is case-insensitive`() {
        assertTrue(VerbTask.SESSIONS.matches("RESUME"))
        assertTrue(VerbTask.SESSIONS.matches("resume"))
        assertTrue(VerbTask.SESSIONS.matches("ReSuMe"))
    }

    @Test
    fun `multiple terms narrow rather than widen`() {
        // "agent" alone reaches several tasks; adding "import" must leave only the import task.
        val agentMatches = VerbTask.entries.filter { it.matches("agent") }
        val narrowed = VerbTask.entries.filter { it.matches("agent import") }
        assertTrue("expected more than one match for a single broad term", agentMatches.size > 1)
        assertEquals(listOf(VerbTask.AGENT_RUNTIME), narrowed)
    }

    @Test
    fun `a query nothing answers matches nothing rather than falling back to everything`() {
        assertTrue(VerbTask.entries.none { it.matches("do a barrel roll") })
    }

    /**
     * The reachability contract for this phase: nothing that used to be a tab, and nothing that used
     * to be buried inside one, may be lost to the information-architecture change. Each word below
     * is one a user would plausibly type, and each must land on the task that owns that capability.
     */
    @Test
    fun `every capability the tabs used to hold is still reachable by a word a user would type`() {
        val expected = mapOf(
            // The Ask and Assistant tabs.
            "ask" to VerbTask.ASK_VERB,
            "assistant" to VerbTask.ASK_VERB,
            // The Agents tab, and the recovery actions that lived on its cards.
            "claude" to VerbTask.AGENTS,
            "codex" to VerbTask.AGENTS,
            "opencode" to VerbTask.AGENTS,
            "resume" to VerbTask.SESSIONS,
            // Evidence and recorded runs, previously only in the terminal's overflow menu.
            "evidence" to VerbTask.EVIDENCE,
            "diagnostics" to VerbTask.EVIDENCE,
            "exit code" to VerbTask.RUNS,
            // Everything the System tab held.
            "provider key" to VerbTask.PROVIDER,
            "install node" to VerbTask.RUNTIMES,
            "manifest" to VerbTask.AGENT_RUNTIME,
            "world" to VerbTask.WORKING_WORLD,
            "vcont" to VerbTask.CONTINUITY,
            "distribution" to VerbTask.SYSTEM
        )

        expected.forEach { (query, task) ->
            assertTrue(
                "\"$query\" should reach ${task.name}",
                VerbTask.entries.filter { it.matches(query) }.contains(task)
            )
        }
    }

    @Test
    fun `no task is unreachable by its own title`() {
        VerbTask.entries.forEach { task ->
            assertTrue(
                "${task.name} cannot be found by its own title",
                VerbTask.entries.filter { it.matches(task.title) }.contains(task)
            )
        }
    }
}
