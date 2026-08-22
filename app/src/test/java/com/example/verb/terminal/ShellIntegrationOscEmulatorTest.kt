package com.example.verb.terminal

import com.termux.terminal.TerminalEmulator
import com.termux.terminal.TerminalSession
import com.termux.terminal.TerminalSessionClient
import java.nio.charset.StandardCharsets
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * Exercises the vendored `com.termux.terminal` patch directly: constructs a real
 * [TerminalEmulator] (no PTY, no JNI subprocess -- a pure in-JVM object) and feeds it raw bytes,
 * the same way real PTY output would arrive. This is the one place that actually proves the
 * minimal vendored diff (OSC 7/633 case labels in `doOscSetTextParameters`, the new
 * `onShellIntegrationOsc` callback threaded through `TerminalOutput`/`TerminalSessionClient`)
 * behaves as intended, and that every other OSC code's existing behavior is untouched.
 */
class ShellIntegrationOscEmulatorTest {

    private class RecordingClient : TerminalSessionClient {
        val oscEvents = mutableListOf<Pair<Int, String>>()

        override fun onTextChanged(changedSession: TerminalSession) {}
        override fun onTitleChanged(changedSession: TerminalSession) {}
        override fun onSessionFinished(finishedSession: TerminalSession) {}
        override fun onCopyTextToClipboard(session: TerminalSession, text: String) {}
        override fun onPasteTextFromClipboard(session: TerminalSession?) {}
        override fun onBell(session: TerminalSession) {}
        override fun onColorsChanged(session: TerminalSession) {}
        override fun onTerminalCursorStateChange(state: Boolean) {}
        override fun setTerminalShellPid(session: TerminalSession, pid: Int) {}
        override fun getTerminalCursorStyle(): Int? = null
        override fun logError(tag: String, message: String) {}
        override fun logWarn(tag: String, message: String) {}
        override fun logInfo(tag: String, message: String) {}
        override fun logDebug(tag: String, message: String) {}
        override fun logVerbose(tag: String, message: String) {}
        override fun logStackTraceWithMessage(tag: String, message: String, e: Exception) {}
        override fun logStackTrace(tag: String, e: Exception) {}

        override fun onShellIntegrationOsc(session: TerminalSession, oscCode: Int, rawArgs: String) {
            oscEvents.add(oscCode to rawArgs)
        }
    }

    private lateinit var client: RecordingClient
    private lateinit var session: TerminalSession
    private lateinit var emulator: TerminalEmulator

    private val esc = 0x1B.toChar()
    private val bel = 0x07.toChar()

    @Before
    fun setUp() {
        client = RecordingClient()
        // No JNI subprocess is spawned here -- the constructor only stores fields until
        // initializeEmulator() is called, which this test never does.
        session = TerminalSession("/bin/sh", "/", arrayOf(), arrayOf(), 100, client)
        emulator = TerminalEmulator(session, 80, 24, 10, 10, 100, client)
    }

    /** Feeds a real, properly terminated OSC sequence: ESC ] <payload> BEL. */
    private fun feedOsc(payload: String) {
        feedRaw(esc.toString() + "]" + payload + bel)
    }

    private fun feedRaw(text: String) {
        val bytes = text.toByteArray(StandardCharsets.UTF_8)
        emulator.append(bytes, bytes.size)
    }

    @Test
    fun `OSC 7 bytes fire the callback with the raw payload`() {
        feedOsc("7;file:///home/user")
        assertEquals(listOf(7 to "file:///home/user"), client.oscEvents)
    }

    @Test
    fun `OSC 633 lifecycle bytes fire the callback with the raw payload`() {
        feedOsc("633;C")
        assertEquals(listOf(633 to "C"), client.oscEvents)
    }

    @Test
    fun `OSC 633 D with an exit code fires the callback intact`() {
        feedOsc("633;D;130")
        assertEquals(listOf(633 to "D;130"), client.oscEvents)
    }

    @Test
    fun `a sequence of markers around real output fires each callback in order`() {
        feedOsc("633;A")
        feedOsc("633;B")
        feedOsc("7;file:///root")
        feedOsc("633;C")
        feedOsc("633;D;0")

        assertEquals(
            listOf(633 to "A", 633 to "B", 7 to "file:///root", 633 to "C", 633 to "D;0"),
            client.oscEvents
        )
    }

    @Test
    fun `marker bytes never appear in the visible screen buffer`() {
        feedRaw("before ")
        feedOsc("633;C")
        feedRaw("after")

        val screenText = emulator.screen.transcriptText
        assertTrue(screenText.contains("before"))
        assertTrue(screenText.contains("after"))
        assertFalse(screenText.contains("633"))
        assertFalse(screenText.contains(esc))
    }

    @Test
    fun `an unrecognized OSC code does not fire the shell-integration callback and does not crash`() {
        feedOsc("999;whatever")
        assertTrue(client.oscEvents.isEmpty())
    }

    @Test
    fun `unknown OSC behavior for an already-handled code (title) is unaffected by this patch`() {
        // Asserted against `emulator` (the object that actually processed the bytes), not
        // `session` -- `session.mEmulator` is intentionally left null in this test since
        // initializeEmulator() (which spawns a JNI subprocess) is never called.
        feedOsc("0;my title")
        assertEquals("my title", emulator.title)
        assertTrue(client.oscEvents.isEmpty())
    }

    @Test
    fun `malformed OSC 633 payload does not crash the emulator and produces no visible artifact`() {
        feedOsc("633;") // empty subcommand
        feedOsc("633;D;not-a-number")
        feedRaw("still typing")

        val screenText = emulator.screen.transcriptText
        assertTrue(screenText.contains("still typing"))
        // The vendored callback still fires (it forwards the raw string unconditionally); it's
        // ShellIntegrationParser, exercised separately, that rejects the malformed payloads.
        assertEquals(2, client.oscEvents.size)
    }
}
