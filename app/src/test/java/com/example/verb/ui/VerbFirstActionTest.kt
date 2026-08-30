package com.example.verb.ui

import com.example.verb.session.VerbSession
import com.example.verb.session.VerbSessionState
import com.example.verb.terminal.RuntimeProfileId
import com.example.verb.terminal.RuntimeProfileReport
import com.example.verb.terminal.RuntimeProfiles
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant

/**
 * The workspace's first action, which exists because an empty screen whose loudest affordance is
 * "Help" answers a question nobody asked.
 *
 * Reports are built from the real [RuntimeProfiles] entries rather than hand-made fakes, so a change
 * to what Verb actually ships shows up here instead of being papered over by a test fixture that
 * agrees with itself.
 */
class VerbFirstActionTest {

    private fun report(
        id: RuntimeProfileId,
        ready: Boolean = true,
        incompatible: Boolean = false
    ): RuntimeProfileReport = RuntimeProfileReport(
        profile = RuntimeProfiles.all.first { it.id == id },
        missingPackages = if (ready || incompatible) emptyList() else listOf("nodejs"),
        missingCommands = emptyList(),
        // A version constraint violated by what is already installed: no install can fix it, which
        // is what RuntimeProfileReport.isUnsatisfiable exists to distinguish.
        incompatibleCommands = if (incompatible) listOf("python") else emptyList()
    )

    private fun session(state: VerbSessionState): VerbSession = VerbSession(
        id = "session-under-test",
        projectId = "project",
        runtime = "claude",
        createdAt = Instant.EPOCH,
        lastSeenAt = Instant.EPOCH,
        state = state
    )

    @Test
    fun `a live session means nothing is offered, because the user needs the terminal`() {
        val action = verbFirstAction(
            reports = listOf(report(RuntimeProfileId.CLAUDE_CODE)),
            sessions = mapOf(RuntimeProfileId.CLAUDE_CODE to session(VerbSessionState.LIVE))
        )
        assertEquals(VerbFirstAction.None, action)
    }

    @Test
    fun `a recoverable session is offered as a resume, which beats starting over`() {
        val action = verbFirstAction(
            reports = listOf(report(RuntimeProfileId.CLAUDE_CODE)),
            sessions = mapOf(RuntimeProfileId.CLAUDE_CODE to session(VerbSessionState.RECOVERABLE))
        )
        assertTrue("expected a Resume, got $action", action is VerbFirstAction.Resume)
        assertEquals(RuntimeProfileId.CLAUDE_CODE, (action as VerbFirstAction.Resume).profileId)
    }

    @Test
    fun `a recoverable record with no local binary offers reinstall before resume`() {
        val action = verbFirstAction(
            reports = listOf(report(RuntimeProfileId.CLAUDE_CODE, ready = false)),
            sessions = mapOf(RuntimeProfileId.CLAUDE_CODE to session(VerbSessionState.RECOVERABLE))
        )

        assertTrue("expected an Install, got $action", action is VerbFirstAction.Install)
    }

    /**
     * `INTERRUPTED` is the absence of an answer, not a yes. Offering Resume there would claim
     * recovery works when nothing has established that -- `Unknown != No`, and equally `Unknown !=
     * Yes`. Starting is the action that is honest in that state.
     */
    @Test
    fun `an interrupted session is never offered as a resume`() {
        val action = verbFirstAction(
            reports = listOf(report(RuntimeProfileId.CLAUDE_CODE)),
            sessions = mapOf(RuntimeProfileId.CLAUDE_CODE to session(VerbSessionState.INTERRUPTED))
        )
        assertTrue("expected a Start, got $action", action is VerbFirstAction.Start)
    }

    @Test
    fun `a ready agent with no session is offered as a start, carrying its real launch command`() {
        val action = verbFirstAction(
            reports = listOf(report(RuntimeProfileId.CLAUDE_CODE)),
            sessions = emptyMap()
        )
        assertTrue("expected a Start, got $action", action is VerbFirstAction.Start)
        val start = action as VerbFirstAction.Start
        assertEquals(
            RuntimeProfiles.all.first { it.id == RuntimeProfileId.CLAUDE_CODE }.launchLine,
            start.command
        )
    }

    @Test
    fun `an agent that is not installed yet is offered as an install`() {
        val action = verbFirstAction(
            reports = listOf(report(RuntimeProfileId.CLAUDE_CODE, ready = false)),
            sessions = emptyMap()
        )
        assertTrue("expected an Install, got $action", action is VerbFirstAction.Install)
    }

    @Test
    fun `an unsatisfiable agent is offered nothing, because that install can never succeed`() {
        val action = verbFirstAction(
            reports = listOf(report(RuntimeProfileId.CLAUDE_CODE, ready = false, incompatible = true)),
            sessions = emptyMap()
        )
        assertEquals(VerbFirstAction.None, action)
    }

    /**
     * Admission is evidence-based. A profile the runtime layer knows how to install is not an agent
     * Verb supports, and the workspace must not offer one as a first action just because a card for
     * it exists somewhere in the runtime catalogue.
     */
    @Test
    fun `a profile outside the admitted set is never offered`() {
        val unadmitted = RuntimeProfiles.all.filter { it.id !in ADMITTED_AGENT_PROFILES }
        assertTrue("expected the runtime catalogue to be wider than the admitted set", unadmitted.isNotEmpty())

        val action = verbFirstAction(
            reports = unadmitted.map { report(it.id) },
            sessions = emptyMap()
        )
        assertEquals(VerbFirstAction.None, action)
    }

    @Test
    fun `no reports at all offers nothing rather than guessing`() {
        assertEquals(VerbFirstAction.None, verbFirstAction(reports = emptyList(), sessions = emptyMap()))
    }

    @Test
    fun `the admitted set is exactly the five integrations Verb has verified`() {
        assertEquals(
            setOf(
                RuntimeProfileId.CLAUDE_CODE,
                RuntimeProfileId.CODEX,
                RuntimeProfileId.OPENCODE,
                RuntimeProfileId.ANTIGRAVITY,
                RuntimeProfileId.HERMES
            ),
            ADMITTED_AGENT_PROFILES
        )
    }
}
