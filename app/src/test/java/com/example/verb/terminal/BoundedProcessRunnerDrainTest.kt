package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The drain runs on a bare `Thread`, where an escaping exception kills the app rather than the
 * probe. Timing out is the normal way to produce one: `destroyForcibly()` closes the stream while
 * the read is parked in it. Verb died a few seconds after launch this way once Codex's readiness
 * probe started running through `qemu-aarch64`, which is slow enough to exceed the bound reliably.
 */
class BoundedProcessRunnerDrainTest {

    @Test
    fun `a probe that exceeds its bound reports TIMEOUT instead of crashing`() {
        val result = BoundedProcessRunner.run(
            argv = listOf("sleep", "10"),
            environment = emptyArray(),
            workingDirectory = null,
            timeoutMs = 300
        )

        assertEquals(BoundedProcessRunner.Outcome.TIMEOUT, result.outcome)
    }

    /** A process killed mid-write must not take the app down with it. */
    @Test
    fun `a chatty process killed mid-stream still reports TIMEOUT`() {
        val result = BoundedProcessRunner.run(
            argv = listOf("sh", "-c", "while :; do echo spam; done"),
            environment = emptyArray(),
            workingDirectory = null,
            timeoutMs = 300
        )

        assertEquals(BoundedProcessRunner.Outcome.TIMEOUT, result.outcome)
    }
}
