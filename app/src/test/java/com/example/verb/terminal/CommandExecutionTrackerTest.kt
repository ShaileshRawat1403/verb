package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class CommandExecutionTrackerTest {

    @Test
    fun `normal flow produces one completed record with cwd, exit code and duration`() {
        val tracker = CommandExecutionTracker()

        tracker.onEvent(ShellIntegrationEvent.CurrentDirectory("/home/user/project"))
        tracker.onEvent(ShellIntegrationEvent.CommandMetadata("git status"))
        tracker.onEvent(ShellIntegrationEvent.CommandStart)
        tracker.onEvent(ShellIntegrationEvent.CommandEnd(0))

        val history = tracker.history.value
        assertEquals(1, history.size)
        val record = history.single()
        assertEquals("git status", record.commandText)
        assertEquals("/home/user/project", record.workingDirectory)
        assertEquals(0, record.exitCode)
        assertEquals(CommandLifecycleState.COMPLETED, record.state)
        assertTrue(record.durationMs != null && record.durationMs!! >= 0)
    }

    @Test
    fun `a nonzero exit code is reported as FAILED`() {
        val tracker = CommandExecutionTracker()
        tracker.onEvent(ShellIntegrationEvent.CommandStart)
        tracker.onEvent(ShellIntegrationEvent.CommandEnd(1))

        assertEquals(CommandLifecycleState.FAILED, tracker.history.value.single().state)
    }

    @Test
    fun `END without a preceding START is ignored, not fabricated into a record`() {
        val tracker = CommandExecutionTracker()

        tracker.onEvent(ShellIntegrationEvent.CommandEnd(0))

        assertTrue(tracker.history.value.isEmpty())
    }

    @Test
    fun `a repeated START abandons the still-running record instead of merging or crashing`() {
        val tracker = CommandExecutionTracker()

        tracker.onEvent(ShellIntegrationEvent.CommandMetadata("sleep 100"))
        tracker.onEvent(ShellIntegrationEvent.CommandStart)
        // No END arrives -- a second command starts anyway (forged or out-of-order marker).
        tracker.onEvent(ShellIntegrationEvent.CommandMetadata("echo hi"))
        tracker.onEvent(ShellIntegrationEvent.CommandStart)
        tracker.onEvent(ShellIntegrationEvent.CommandEnd(0))

        val history = tracker.history.value
        assertEquals(2, history.size)
        assertEquals("sleep 100", history[0].commandText)
        assertEquals(CommandLifecycleState.ABANDONED, history[0].state)
        assertEquals("echo hi", history[1].commandText)
        assertEquals(CommandLifecycleState.COMPLETED, history[1].state)
        assertNotEquals(history[0].id, history[1].id)
    }

    @Test
    fun `session end abandons a running record but preserves prior history`() {
        val tracker = CommandExecutionTracker()
        tracker.onEvent(ShellIntegrationEvent.CommandStart)
        tracker.onEvent(ShellIntegrationEvent.CommandEnd(0))

        tracker.onEvent(ShellIntegrationEvent.CommandMetadata("still-running"))
        tracker.onEvent(ShellIntegrationEvent.CommandStart)
        tracker.onSessionEnded()

        val history = tracker.history.value
        assertEquals(2, history.size)
        assertEquals(CommandLifecycleState.COMPLETED, history[0].state)
        assertEquals(CommandLifecycleState.ABANDONED, history[1].state)
        assertEquals("still-running", history[1].commandText)
    }

    @Test
    fun `session end with nothing running does not add a spurious record`() {
        val tracker = CommandExecutionTracker()
        tracker.onEvent(ShellIntegrationEvent.CommandStart)
        tracker.onEvent(ShellIntegrationEvent.CommandEnd(0))

        tracker.onSessionEnded()

        assertEquals(1, tracker.history.value.size)
    }

    @Test
    fun `history is bounded to 50 records, dropping the oldest first`() {
        val tracker = CommandExecutionTracker()
        repeat(60) { i ->
            tracker.onEvent(ShellIntegrationEvent.CommandMetadata("cmd-$i"))
            tracker.onEvent(ShellIntegrationEvent.CommandStart)
            tracker.onEvent(ShellIntegrationEvent.CommandEnd(0))
        }

        val history = tracker.history.value
        assertEquals(CommandExecutionTracker.MAX_HISTORY, history.size)
        assertEquals("cmd-10", history.first().commandText) // oldest 10 (0..9) dropped
        assertEquals("cmd-59", history.last().commandText)
    }

    @Test
    fun `handshake flips shellIntegrationActive, session end resets it`() {
        val tracker = CommandExecutionTracker()
        assertEquals(false, tracker.shellIntegrationActive.value)

        tracker.onEvent(ShellIntegrationEvent.Handshake)
        assertEquals(true, tracker.shellIntegrationActive.value)

        tracker.onSessionEnded()
        assertEquals(false, tracker.shellIntegrationActive.value)
    }

    @Test
    fun `current working directory starts unknown`() {
        assertNull(CommandExecutionTracker().currentWorkingDirectory.value)
    }

    @Test
    fun `a valid OSC 7 event updates the current working directory`() {
        val tracker = CommandExecutionTracker()

        tracker.onEvent(ShellIntegrationEvent.CurrentDirectory("/data/data/com.aistudio.verb.app/files/projects/demo"))

        assertEquals(
            "/data/data/com.aistudio.verb.app/files/projects/demo",
            tracker.currentWorkingDirectory.value
        )
    }

    @Test
    fun `a later OSC 7 event replaces the previous working directory`() {
        val tracker = CommandExecutionTracker()

        tracker.onEvent(ShellIntegrationEvent.CurrentDirectory("/first"))
        tracker.onEvent(ShellIntegrationEvent.CurrentDirectory("/second"))

        assertEquals("/second", tracker.currentWorkingDirectory.value)
    }

    @Test
    fun `no event other than OSC 7 ever sets the working directory`() {
        val tracker = CommandExecutionTracker()

        tracker.onEvent(ShellIntegrationEvent.Handshake)
        tracker.onEvent(ShellIntegrationEvent.PromptStart)
        tracker.onEvent(ShellIntegrationEvent.PromptEnd)
        tracker.onEvent(ShellIntegrationEvent.CommandMetadata("cd /somewhere"))
        tracker.onEvent(ShellIntegrationEvent.CommandStart)
        tracker.onEvent(ShellIntegrationEvent.CommandEnd(0))

        assertNull(tracker.currentWorkingDirectory.value)
    }

    /**
     * Session end covers restart and reconfigure too: both go through
     * [TermuxTerminalRuntimeAdapter.destroy], which calls [CommandExecutionTracker.onSessionEnded].
     * The next session must not inherit the previous session's directory -- a project switch or an
     * Agent Runtime activation restarts the PTY somewhere else entirely.
     */
    @Test
    fun `session end clears the current working directory`() {
        val tracker = CommandExecutionTracker()
        tracker.onEvent(ShellIntegrationEvent.CurrentDirectory("/old/session/dir"))

        tracker.onSessionEnded()

        assertNull(tracker.currentWorkingDirectory.value)
    }

    @Test
    fun `after a session end the directory stays unknown until the next OSC 7`() {
        val tracker = CommandExecutionTracker()
        tracker.onEvent(ShellIntegrationEvent.CurrentDirectory("/old/session/dir"))
        tracker.onSessionEnded()

        // A full command lifecycle in the new session, with no OSC 7 yet.
        tracker.onEvent(ShellIntegrationEvent.CommandMetadata("echo hi"))
        tracker.onEvent(ShellIntegrationEvent.CommandStart)
        assertNull(tracker.currentWorkingDirectory.value)
        tracker.onEvent(ShellIntegrationEvent.CommandEnd(0))

        assertNull(tracker.currentWorkingDirectory.value)
        // The record must not carry the previous session's directory either.
        assertNull(tracker.history.value.last().workingDirectory)

        tracker.onEvent(ShellIntegrationEvent.CurrentDirectory("/new/session/dir"))
        assertEquals("/new/session/dir", tracker.currentWorkingDirectory.value)
    }

    @Test
    fun `prompt start and end events do not themselves create records`() {
        val tracker = CommandExecutionTracker()
        tracker.onEvent(ShellIntegrationEvent.PromptStart)
        tracker.onEvent(ShellIntegrationEvent.PromptEnd)

        assertTrue(tracker.history.value.isEmpty())
    }

    @Test
    fun `command text is redacted before it ever reaches a stored record`() {
        val tracker = CommandExecutionTracker()
        tracker.onEvent(ShellIntegrationEvent.CommandMetadata("curl -H \"Authorization: Bearer sk-abcdef123456\" https://api.example.com"))
        tracker.onEvent(ShellIntegrationEvent.CommandStart)
        tracker.onEvent(ShellIntegrationEvent.CommandEnd(0))

        val commandText = tracker.history.value.single().commandText
        assertTrue(commandText.contains("REDACTED"))
        assertTrue(!commandText.contains("sk-abcdef123456"))
    }

    @Test
    fun `a running record is not published to history until it reaches a terminal state`() {
        val tracker = CommandExecutionTracker()
        tracker.onEvent(ShellIntegrationEvent.CommandStart)

        // RUNNING records are only appended to `history` once they resolve to COMPLETED/FAILED/
        // ABANDONED -- there is no "in-flight" row in P0 (no UI to show one to yet).
        assertTrue(tracker.history.value.isEmpty())
    }
}
