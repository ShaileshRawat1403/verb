package com.example.verb.ui

import com.example.verb.session.VerbSessionState
import com.example.verb.terminal.AgentWorkFact
import com.example.verb.terminal.CommandExecutionRecord
import com.example.verb.terminal.CommandLifecycleState
import com.example.verb.terminal.GitSnapshot
import com.example.verb.terminal.TerminalAiHelper
import com.example.verb.terminal.TerminalEvidence
import com.example.verb.terminal.TerminalSessionState
import java.time.Instant
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AssistEvidenceTest {

    private val now: Instant = Instant.parse("2026-08-26T12:00:00Z")

    private fun command(text: String, exit: Int?, state: CommandLifecycleState, durationMs: Long) =
        CommandExecutionRecord(
            id = "opaque",
            commandText = text,
            workingDirectory = "/private/client-name",
            startedAtEpochMs = 0,
            endedAtEpochMs = durationMs,
            exitCode = exit,
            state = state
        )

    private fun evidence() = TerminalEvidence(
        sessionState = TerminalSessionState.RUNNING,
        workingDirectoryKnown = true,
        shellIntegrationActive = true,
        commandTail = listOf(
            command("alpha-secret-build", 0, CommandLifecycleState.COMPLETED, 450),
            command("beta-secret-deploy", 1, CommandLifecycleState.FAILED, 2_000)
        ),
        agentWork = listOf(
            AgentWorkFact("Claude Code", VerbSessionState.LIVE, now.minusSeconds(240), "claude")
        )
    )

    /**
     * `docs/UX_FOUNDATION.md`: plain language on screen, the contract's exact vocabulary
     * underneath. The panel is what a person reads, so the contract's spelling must not surface
     * here even though the same snapshot sends it to the provider.
     */
    @Test
    fun `the panel reads in plain language and never in contract vocabulary`() {
        val lines = AssistEvidence.displayLines(evidence(), now)
        val text = lines.joinToString("\n")

        assertTrue(text.contains("terminal session running"))
        assertTrue(text.contains("failed"))
        assertTrue(text.contains("finished"))
        assertTrue(text.contains("Claude Code"))
        assertFalse(text.contains("RUNNING"))
        assertFalse(text.contains("COMPLETED"))
        assertFalse(text.contains("FAILED"))
        assertFalse(text.contains("LIVE"))
    }

    /** Colour never carries meaning alone: every state line leads with a glyph from the narrow set. */
    @Test
    fun `every state line carries a glyph as well as a word`() {
        val lines = AssistEvidence.displayLines(evidence(), now)

        assertTrue(lines.any { it.startsWith("●") })
        assertTrue(lines.any { it.trimStart().startsWith("✕") })
        assertTrue(lines.any { it.trimStart().startsWith("○") })
        assertTrue(lines.all { it.none { ch -> ch.code > 0x2FFF } })
    }

    /** The same privacy boundary as the prompt: what the user reads back cannot contain the secret. */
    @Test
    fun `command text and paths never reach the panel`() {
        val text = AssistEvidence.displayLines(evidence(), now).joinToString("\n")

        assertFalse(text.contains("alpha-secret-build"))
        assertFalse(text.contains("beta-secret-deploy"))
        assertFalse(text.contains("client-name"))
    }

    /**
     * The panel and the envelope are built from one snapshot, so they can disagree in wording but
     * never in facts. Both must report the same number of commands, in the same order.
     */
    @Test
    fun `panel and provider envelope describe the same commands in the same order`() {
        val evidence = evidence()
        val panel = AssistEvidence.displayLines(evidence, now)
        val envelope = TerminalAiHelper.evidenceLines(evidence)

        assertTrue(panel.any { it.contains("newest first") })
        assertTrue(envelope.any { it.contains("newest first") })
        // "Last 1 commands" was what the device actually rendered.
        assertFalse(panel.any { it.contains("1 commands") })
        // Newest is the failing one in both renderings.
        assertTrue(panel.indexOfFirst { it.contains("failed") } < panel.indexOfFirst { it.contains("finished") })
        assertTrue(envelope.indexOfFirst { it.contains("FAILED") } < envelope.indexOfFirst { it.contains("COMPLETED") })
    }

    @Test
    fun `a single command boundary reads as one command, not as "1 commands"`() {
        val one = TerminalEvidence(
            sessionState = TerminalSessionState.RUNNING,
            workingDirectoryKnown = true,
            shellIntegrationActive = true,
            commandTail = listOf(command("only", 2, CommandLifecycleState.FAILED, 35))
        )

        assertTrue(AssistEvidence.displayLines(one, now).any { it.contains("The last command —") })
        assertTrue(TerminalAiHelper.evidenceLines(one).any { it.contains("Last command boundary") })
    }

    @Test
    fun `an unobserved fact reads as unknown rather than as a negative claim`() {
        val lines = AssistEvidence.displayLines(
            TerminalEvidence(TerminalSessionState.RUNNING, false, false),
            now
        )

        assertTrue(lines.any { it.contains("working directory not observed") })
        assertTrue(lines.any { it.contains("does not report command boundaries") })
        assertTrue(lines.any { it.contains("no command boundaries recorded yet") })
    }

    /**
     * The working tree is the fact the assistant was missing: without it, "what did the agent
     * change?" had no evidence at all behind it.
     */
    @Test
    fun `a dirty tree reads as a count in both renderings, and names nothing`() {
        val dirty = TerminalEvidence(
            sessionState = TerminalSessionState.RUNNING,
            workingDirectoryKnown = true,
            shellIntegrationActive = true,
            git = GitSnapshot(
                observed = true,
                insideRepository = true,
                onNamedBranch = true,
                changedFiles = 3,
                stagedFiles = 1,
                headShort = "a1b2c3d"
            )
        )

        val panel = AssistEvidence.displayLines(dirty, now).joinToString("\n")
        val envelope = TerminalAiHelper.evidenceLines(dirty, now).joinToString("\n")

        assertTrue(panel.contains("3 changed files"))
        assertTrue(panel.contains("1 staged"))
        assertTrue(panel.contains("a1b2c3d"))
        assertTrue(envelope.contains("3 changed files"))
        assertTrue(envelope.contains("name withheld"))
    }

    /** Unknown is not No: a tree Verb could not read must never render as a clean one. */
    @Test
    fun `an unobserved tree never reads as clean`() {
        val unknown = TerminalEvidence(
            TerminalSessionState.RUNNING, true, true,
            git = GitSnapshot.unobserved(now)
        )

        val panel = AssistEvidence.displayLines(unknown, now).joinToString("\n")
        val envelope = TerminalAiHelper.evidenceLines(unknown, now).joinToString("\n")

        assertTrue(panel.contains("not observed"))
        assertFalse(panel.contains("clean"))
        assertTrue(envelope.contains("not observed"))
        assertFalse(envelope.contains("clean"))
    }

    @Test
    fun `a clean tree says so, and says which commit it is clean at`() {
        val clean = TerminalEvidence(
            TerminalSessionState.RUNNING, true, true,
            git = GitSnapshot(observed = true, insideRepository = true, onNamedBranch = true, headShort = "a1b2c3d")
        )

        assertTrue(AssistEvidence.displayLines(clean, now).any { it.contains("clean at a1b2c3d") })
    }

    /** No snapshot at all is the same claim as one that failed, and reads the same way. */
    @Test
    fun `no snapshot reads as not observed`() {
        val none = TerminalEvidence(TerminalSessionState.RUNNING, true, true)

        assertTrue(AssistEvidence.displayLines(none, now).any { it.contains("not observed") })
    }

    @Test
    fun `durations read the way a person says them`() {
        assertEquals("450ms", AssistEvidence.duration(450))
        assertEquals("2.0s", AssistEvidence.duration(2_000))
        assertEquals("1m 5s", AssistEvidence.duration(65_000))
    }

    @Test
    fun `a timestamp reads as an age, and clock skew is not evidence`() {
        assertEquals("just now", AssistEvidence.relativeTime(now.minusSeconds(5), now))
        assertEquals("4m ago", AssistEvidence.relativeTime(now.minusSeconds(240), now))
        assertEquals("3h ago", AssistEvidence.relativeTime(now.minusSeconds(10_800), now))
        assertEquals("2d ago", AssistEvidence.relativeTime(now.minusSeconds(172_800), now))
        assertEquals("just now", AssistEvidence.relativeTime(now.plusSeconds(30), now))
    }
}
