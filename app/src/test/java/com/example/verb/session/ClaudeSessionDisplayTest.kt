package com.example.verb.session

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant

class ClaudeSessionDisplayTest {

    private fun sessionWith(state: VerbSessionState): VerbSession = VerbSession(
        id = "session-1",
        projectId = "project-1",
        runtime = "claude",
        createdAt = Instant.EPOCH,
        lastSeenAt = Instant.EPOCH,
        state = state,
        agent = AgentRef("claude", "session-abc")
    )

    @Test
    fun `no session at all means no display -- the card falls back to install-ready`() {
        assertNull(claudeSessionDisplay(null))
    }

    @Test
    fun `LIVE shows Running, with neither action`() {
        val display = claudeSessionDisplay(sessionWith(VerbSessionState.LIVE))!!

        assertEquals("Running", display.statusLabel)
        assertFalse(display.showResume)
        assertFalse(display.showStartNew)
    }

    @Test
    fun `INTERRUPTED shows a checking detail and neither action -- Resume must not appear yet`() {
        val display = claudeSessionDisplay(sessionWith(VerbSessionState.INTERRUPTED))!!

        assertEquals("Session interrupted", display.statusLabel)
        assertEquals("Checking recovery status…", display.detailLabel)
        assertFalse(display.showResume)
        assertFalse(display.showStartNew)
    }

    @Test
    fun `RECOVERABLE is the only state that shows Resume`() {
        val display = claudeSessionDisplay(sessionWith(VerbSessionState.RECOVERABLE))!!

        assertEquals("Session recoverable", display.statusLabel)
        assertTrue(display.showResume)
        assertFalse(display.showStartNew)
    }

    @Test
    fun `ENDED shows Start new, not Resume`() {
        val display = claudeSessionDisplay(sessionWith(VerbSessionState.ENDED))!!

        assertEquals("Session ended", display.statusLabel)
        assertFalse(display.showResume)
        assertTrue(display.showStartNew)
    }

    @Test
    fun `Resume appears in exactly one of the four states`() {
        val statesShowingResume = VerbSessionState.entries.filter {
            claudeSessionDisplay(sessionWith(it))?.showResume == true
        }

        assertEquals(listOf(VerbSessionState.RECOVERABLE), statesShowingResume)
    }
}
