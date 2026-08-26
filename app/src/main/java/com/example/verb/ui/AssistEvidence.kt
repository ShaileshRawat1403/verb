package com.example.verb.ui

import com.example.verb.terminal.TerminalAiHelper
import com.example.verb.terminal.TerminalEvidence
import java.time.Instant

/**
 * The evidence envelope, read back to the person who asked.
 *
 * `TerminalAiHelper.evidenceLines` produces the lines that cross the provider boundary, in the
 * contract's exact vocabulary (`RUNNING`, `LIVE`, `FAILED`) because that is what a model should
 * reason over. This produces the same facts in the words `docs/UX_FOUNDATION.md` requires on
 * screen -- plain language on screen, exact vocabulary underneath -- from the same snapshot, so the
 * two can never describe different evidence.
 *
 * Every line carries a glyph and a word, never colour alone, and states come from
 * [VerbStatusVocabulary] rather than being spelled out here a second time.
 */
object AssistEvidence {

    fun displayLines(evidence: TerminalEvidence, now: Instant = Instant.now()): List<String> = buildList {
        add("${VerbStatusVocabulary.processGlyph(evidence.sessionState)} terminal session " +
            VerbStatusVocabulary.processWord(evidence.sessionState))

        add(
            if (evidence.workingDirectoryKnown) {
                "· working directory observed — the path itself was not sent"
            } else {
                "◌ working directory not observed"
            }
        )

        add(
            if (evidence.shellIntegrationActive) {
                "· your shell reports command boundaries"
            } else {
                "◌ your shell does not report command boundaries"
            }
        )

        if (evidence.commandTail.isEmpty()) {
            add("◌ no command boundaries recorded yet")
        } else {
            add(
                if (evidence.commandTail.size == 1) {
                    "The last command — no command text was sent:"
                } else {
                    "The last ${evidence.commandTail.size} commands, newest first — no command text was sent:"
                }
            )
            evidence.commandTail.asReversed().forEach { record ->
                val exit = record.exitCode?.let { " · exit $it" } ?: ""
                val took = record.durationMs?.let { " · ${duration(it)}" } ?: ""
                add(
                    "  ${VerbStatusVocabulary.commandGlyph(record.state)} " +
                        "${VerbStatusVocabulary.commandWord(record.state)}$exit$took"
                )
            }
        }

        evidence.agentWork.forEach { fact ->
            val agent = fact.agentType?.let { " ($it)" } ?: ""
            add(
                "${VerbStatusVocabulary.sessionGlyph(fact.sessionState)} ${fact.profileName}$agent · " +
                    "${VerbStatusVocabulary.sessionWord(fact.sessionState)} · " +
                    "last seen ${relativeTime(fact.lastSeenAt, now)}"
            )
        }
    }

    /** Sub-second work reads in milliseconds; anything longer reads the way a person would say it. */
    internal fun duration(millis: Long): String = when {
        millis < 1_000 -> "${millis}ms"
        millis < 60_000 -> String.format("%.1fs", millis / 1000.0)
        else -> "${millis / 60_000}m ${(millis % 60_000) / 1000}s"
    }

    /**
     * "4m ago" rather than an ISO instant. A timestamp is what the record stores; how long ago it
     * was is the question a person is actually asking.
     *
     * One implementation, in the layer that owns the envelope, so the panel and the provider cannot
     * describe the same moment differently.
     */
    internal fun relativeTime(then: Instant, now: Instant): String =
        TerminalAiHelper.relativeAge(then, now)
}
