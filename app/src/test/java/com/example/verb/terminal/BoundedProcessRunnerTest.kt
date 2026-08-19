package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The mechanics every bounded probe depends on. Exercised against real host commands so the
 * timeout, the exit-code mapping and the bounded capture are verified rather than assumed --
 * [AgentRuntimeCompatibilityProbe] turns these outcomes straight into product state.
 */
class BoundedProcessRunnerTest {

    private fun run(vararg argv: String, timeoutMs: Long = 5_000L) =
        BoundedProcessRunner.run(argv.toList(), emptyArray(), null, timeoutMs)

    @Test
    fun `a successful command completes with exit code zero`() {
        val result = run("/bin/sh", "-c", "exit 0")

        assertEquals(BoundedProcessRunner.Outcome.COMPLETED, result.outcome)
        assertEquals(0, result.exitCode)
    }

    /** 255 is precisely what the Agent Runtime returned on the validation device. */
    @Test
    fun `a nonzero exit is reported as completed with that code, not as a failure`() {
        val result = run("/bin/sh", "-c", "exit 255")

        assertEquals(BoundedProcessRunner.Outcome.COMPLETED, result.outcome)
        assertEquals(255, result.exitCode)
    }

    @Test
    fun `a command exceeding its bound is destroyed and reported as a timeout`() {
        val result = run("/bin/sh", "-c", "sleep 10", timeoutMs = 300L)

        assertEquals(BoundedProcessRunner.Outcome.TIMEOUT, result.outcome)
        assertNull(result.exitCode)
    }

    @Test
    fun `a process that cannot start is reported as a launch failure`() {
        val result = run("/definitely/not/a/real/binary")

        assertEquals(BoundedProcessRunner.Outcome.LAUNCH_FAILED, result.outcome)
        assertNull(result.exitCode)
    }

    @Test
    fun `captured output is bounded and the child is still drained to completion`() {
        val result = run("/bin/sh", "-c", "yes verbverbverb | head -c 200000")

        assertEquals(BoundedProcessRunner.Outcome.COMPLETED, result.outcome)
        assertEquals(0, result.exitCode)
        assertTrue(result.output.length < BoundedProcessRunner.MAX_OUTPUT_CHARS + 64)
        assertTrue(result.output.contains("truncated"))
    }

    /** The inherited environment is cleared, so a probe can never pick up the app's own variables. */
    @Test
    fun `only the supplied environment reaches the child`() {
        val result = BoundedProcessRunner.run(
            argv = listOf("/bin/sh", "-c", "echo \"[\$VERB_PROBE_MARKER]\""),
            environment = arrayOf("VERB_PROBE_MARKER=present", "malformed-entry-without-equals"),
            workingDirectory = null,
            timeoutMs = 5_000L
        )

        assertEquals(0, result.exitCode)
        assertTrue(result.output.contains("[present]"))
    }
}
