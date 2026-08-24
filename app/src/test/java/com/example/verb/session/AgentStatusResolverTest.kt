package com.example.verb.session

import com.example.verb.session.AgentStatusResolver.Action
import com.example.verb.session.AgentStatusResolver.Evidence
import com.example.verb.terminal.RuntimeProfileId
import com.example.verb.terminal.RuntimeProfileReport
import com.example.verb.terminal.RuntimeProfiles
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant

/**
 * The resolver exists because four sources of evidence used to be turned into words in four places,
 * and every wrong label was a new bug in a new place. These tests are the record of what each piece
 * of evidence is allowed to mean.
 */
class AgentStatusResolverTest {

    private fun report(
        id: RuntimeProfileId = RuntimeProfileId.CLAUDE_CODE,
        missingCommands: List<String> = emptyList(),
        incompatible: List<String> = emptyList()
    ) = RuntimeProfileReport(
        profile = RuntimeProfiles.forId(id),
        missingPackages = emptyList(),
        missingCommands = missingCommands,
        incompatibleCommands = incompatible
    )

    private fun evidence(
        report: RuntimeProfileReport = report(),
        session: VerbSession? = null,
        signedIn: Boolean? = null,
        installing: Boolean = false,
        otherInstallRunning: Boolean = false
    ) = Evidence(report, session, signedIn, installing, otherInstallRunning)

    private fun session(state: VerbSessionState) = VerbSession(
        id = "session-1",
        projectId = "alpha",
        runtime = "claude",
        createdAt = Instant.EPOCH,
        lastSeenAt = Instant.EPOCH,
        state = state,
        agent = AgentRef("claude")
    )

    @Test
    fun `an observed session outranks a probe`() {
        // The probe says the binary is installed and runnable. The session says it is running right
        // now. The second was observed; the first was inferred from a file being executable.
        val status = AgentStatusResolver.resolve(
            evidence(session = session(VerbSessionState.LIVE))
        )

        assertEquals("Running", status.label)
        assertEquals(Action.NONE, status.action)
        assertTrue(status.because.contains("process binding"))
    }

    @Test
    fun `each session state maps to exactly one action`() {
        assertEquals(
            Action.RESUME,
            AgentStatusResolver.resolve(evidence(session = session(VerbSessionState.RECOVERABLE))).action
        )
        // Never Resume: nothing has established that recovery works. "Start new" claims nothing
        // about the interrupted session.
        assertEquals(
            Action.START_NEW,
            AgentStatusResolver.resolve(evidence(session = session(VerbSessionState.INTERRUPTED))).action
        )
        assertEquals(
            Action.START_NEW,
            AgentStatusResolver.resolve(evidence(session = session(VerbSessionState.ENDED))).action
        )
    }

    @Test
    fun `an agent that cannot run here offers nothing, whatever the probe says`() {
        // dsh answers `--version` perfectly well and still cannot run: its native module has no
        // Android build. A passing probe is not proof an agent works.
        val status = AgentStatusResolver.resolve(
            evidence(report = report(RuntimeProfileId.DEEPSEEK_HARNESS))
        )

        assertEquals("Unavailable", status.label)
        assertEquals(Action.NONE, status.action)
        assertNotNull("the reason must be shown, not just the refusal", status.detail)
    }

    @Test
    fun `sign-in is a detail and never a status`() {
        // The evidence is that a credential file exists, which says an agent once wrote credentials
        // -- not that they are still valid.
        val signedIn = AgentStatusResolver.resolve(evidence(signedIn = true))
        assertEquals("Ready", signedIn.label)
        assertEquals("Saved login found — the agent verifies it when opened", signedIn.detail)

        val unknown = AgentStatusResolver.resolve(evidence(signedIn = null))
        assertEquals("Ready", unknown.label)
        assertEquals("an unobserved credential location says nothing", null, unknown.detail)
    }

    @Test
    fun `an install in progress elsewhere blocks the button but does not change the status`() {
        val status = AgentStatusResolver.resolve(
            evidence(report = report(missingCommands = listOf("claude")), otherInstallRunning = true)
        )

        assertEquals("Not installed", status.label)
        assertEquals(Action.NONE, status.action)
    }

    @Test
    fun `a saved recoverable session does not invent an installed executable`() {
        val status = AgentStatusResolver.resolve(
            evidence(
                report = report(missingCommands = listOf("claude")),
                session = session(VerbSessionState.RECOVERABLE)
            )
        )

        assertEquals("Not installed", status.label)
        assertEquals(Action.INSTALL, status.action)
        assertTrue(status.detail.orEmpty().contains("available after reinstall"))
    }

    @Test
    fun `every status says what it is based on`() {
        // The point of one resolver is that a label can be traced back to the evidence that produced
        // it. A status with no reason would be exactly the thing this replaced.
        val cases = listOf(
            evidence(),
            evidence(session = session(VerbSessionState.LIVE)),
            evidence(session = session(VerbSessionState.RECOVERABLE)),
            evidence(installing = true),
            evidence(report = report(missingCommands = listOf("claude")))
        )

        for (case in cases) {
            val status = AgentStatusResolver.resolve(case)
            assertTrue("'${status.label}' should say why", status.because.length > 10)
        }
    }
}
