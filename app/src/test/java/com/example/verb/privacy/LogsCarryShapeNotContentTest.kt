package com.example.verb.privacy

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Logcat is not the durable store, and that is exactly why this is easy to get wrong.
 *
 * `docs/ARCHITECTURE.md` forbids retaining raw PTY output. A `Log.w` carrying a terminal transcript
 * is not "retention" in the store's sense, so it slips past the rule while making the same
 * disclosure: anyone holding the phone can read app logs over adb, and a transcript contains the
 * user's prompts, the model's replies, every command run, and whatever those commands printed --
 * including a token echoed by accident.
 *
 * This is a lint expressed as a test because it has already happened once. `c5b5642` hardened agent
 * launches and removed environment/argv exposure, and in the same commit turned
 *
 *     Log.w(TAG, "Session finished exit=... shell=...")
 *
 * into
 *
 *     Log.w(TAG, "Session finished exit=... shell=... output=[$transcript]")
 *
 * Reviewing the diff is what caught it. This makes the next one fail a build instead.
 */
class LogsCarryShapeNotContentTest {

    /** Expressions that read user or model content rather than structure. */
    private val contentExpressions = listOf(
        "transcriptText",
        "screen.transcript"
    )

    private fun kotlinSources(): List<File> =
        File("src/main/java").walkTopDown().filter { it.isFile && it.extension == "kt" }.toList()

    private val logCall = Regex("""\bLog\.[wdiev]\(""")

    /**
     * The leak this exists for never put the content and the log on one line:
     *
     *     val transcript = runCatching { ... transcriptText }.getOrNull()
     *     Log.w(TAG, "Session finished ... output=[$transcript]")
     *
     * So a line-by-line scan would have passed it, which is why this binds the two together --
     * it collects every name assigned from a content expression, then flags any log statement
     * that interpolates one of those names.
     */
    private fun offendersIn(file: File): List<String> {
        val lines = file.readLines()
        val contentNames = mutableSetOf<String>()
        for (line in lines) {
            if (contentExpressions.none { it in line }) continue
            Regex("""\b(?:val|var)\s+(\w+)\s*=""").find(line)?.let { contentNames += it.groupValues[1] }
        }

        val offenders = mutableListOf<String>()
        lines.forEachIndexed { index, line ->
            if (!logCall.containsMatchIn(line)) return@forEachIndexed
            val inlineContent = contentExpressions.any { it in line }
            val interpolated = contentNames.any { name ->
                Regex("""\$""" + name + """\b""").containsMatchIn(line) ||
                    Regex("""\$\{[^}]*\b""" + name + """\b""").containsMatchIn(line)
            }
            if (inlineContent || interpolated) {
                offenders += "${file.path}:${index + 1}: ${line.trim()}"
            }
        }
        return offenders
    }

    @Test
    fun `no log statement carries terminal content`() {
        val offenders = kotlinSources().flatMap(::offendersIn)
        assertTrue(
            "a log statement is carrying terminal content; log its shape instead (outputLength, " +
                "hadOutput, exit code):\n" + offenders.joinToString("\n"),
            offenders.isEmpty()
        )
    }

    /** The guard is worthless if it cannot see the sources it is meant to police. */
    @Test
    fun `the guard is actually reading the main source set`() {
        val sources = kotlinSources()
        assertTrue("no Kotlin sources found; the path this test walks has moved", sources.size > 50)
        assertTrue(
            "the adapter this rule exists for was not scanned",
            sources.any { it.name == "TermuxTerminalRuntimeAdapter.kt" }
        )
    }
}
