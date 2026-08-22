package com.example.verb.session

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant

class AgentSessionDisplayTest {

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
        assertNull(agentSessionDisplay(null))
    }

    @Test
    fun `LIVE shows Running, with neither action`() {
        val display = agentSessionDisplay(sessionWith(VerbSessionState.LIVE))!!

        assertEquals("Running", display.statusLabel)
        assertFalse(display.showResume)
        assertFalse(display.showStartNew)
    }

    @Test
    fun `INTERRUPTED shows a checking detail, offers a fresh start, and never Resume`() {
        // Resume must not appear: nothing has established that recovery works. "Start new" may,
        // because it claims nothing about the interrupted session -- and a card with no action at
        // all would strand the user, unable to resume and unable to start over.
        val display = agentSessionDisplay(sessionWith(VerbSessionState.INTERRUPTED))!!

        assertEquals("Session interrupted", display.statusLabel)
        assertEquals("Checking recovery status…", display.detailLabel)
        assertFalse(display.showResume)
        assertTrue(display.showStartNew)
    }

    @Test
    fun `RECOVERABLE is the only state that shows Resume`() {
        val display = agentSessionDisplay(sessionWith(VerbSessionState.RECOVERABLE))!!

        assertEquals("Session recoverable", display.statusLabel)
        assertTrue(display.showResume)
        assertFalse(display.showStartNew)
    }

    @Test
    fun `ENDED shows Start new, not Resume`() {
        val display = agentSessionDisplay(sessionWith(VerbSessionState.ENDED))!!

        assertEquals("Session ended", display.statusLabel)
        assertFalse(display.showResume)
        assertTrue(display.showStartNew)
    }

    @Test
    fun `Resume appears in exactly one of the four states`() {
        val statesShowingResume = VerbSessionState.entries.filter {
            agentSessionDisplay(sessionWith(it))?.showResume == true
        }

        assertEquals(listOf(VerbSessionState.RECOVERABLE), statesShowingResume)
    }
}
