package com.example.verb.terminal

import java.io.InputStream
import java.util.concurrent.TimeUnit

/**
 * The execution mechanics shared by every bounded, out-of-band probe Verb runs: argv-only launch,
 * a hard wall-clock bound, a bounded captured prefix, and a drained child stream.
 *
 * Extracted so [GuestCommandRunner] and [AgentRuntimeCompatibilityProbe] cannot drift apart on the
 * parts that make a probe safe. It deliberately owns *only* the mechanics -- it decides nothing
 * about what may be run. Each caller keeps its own policy: [GuestCommandRunner] still refuses any
 * requirement without a catalog-registered probe, and [AgentRuntimeCompatibilityProbe] still runs a
 * single hard-coded command. Nothing here accepts a shell string; every caller passes a real argv
 * array, so there is no interpolation surface at this layer either.
 */
internal object BoundedProcessRunner {

    const val MAX_OUTPUT_CHARS = 2_000

    /** Why a bounded run ended. Distinguishes "never started" from "ran and failed" from "too slow". */
    enum class Outcome {
        /** The process ran to completion within the bound; see [Result.exitCode]. */
        COMPLETED,
        /** Exceeded the bound before finishing; the process was forcibly destroyed. */
        TIMEOUT,
        /** The process itself could not be started (missing/corrupt executable, EACCES, ...). */
        LAUNCH_FAILED
    }

    data class Result(
        val outcome: Outcome,
        val exitCode: Int?,
        val output: String
    )

    /**
     * Runs [argv] with exactly [environment] (the inherited environment is cleared first, so a probe
     * can never accidentally pick up the app process's own variables) and at most [timeoutMs]
     * milliseconds of wall clock.
     *
     * @param argv full argument vector including argv[0]; never a shell string.
     * @param environment `KEY=VALUE` entries; entries without `=` are ignored.
     * @param workingDirectory host directory to start in, when it exists.
     */
    fun run(
        argv: List<String>,
        environment: Array<String>,
        workingDirectory: java.io.File?,
        timeoutMs: Long
    ): Result {
        return try {
            val builder = ProcessBuilder(argv).redirectErrorStream(true)
            if (workingDirectory != null && workingDirectory.isDirectory) {
                builder.directory(workingDirectory)
            }
            builder.environment().clear()
            for (entry in environment) {
                val separator = entry.indexOf('=')
                if (separator > 0) {
                    builder.environment()[entry.substring(0, separator)] = entry.substring(separator + 1)
                }
            }

            val process = builder.start()
            val output = StringBuilder()
            val drainer = Thread { drainBounded(process.inputStream, output) }.apply {
                isDaemon = true
                start()
            }

            val finished = process.waitFor(timeoutMs, TimeUnit.MILLISECONDS)
            if (!finished) {
                process.destroyForcibly()
                drainer.join(500)
                return Result(Outcome.TIMEOUT, null, output.toString())
            }
            drainer.join(2_000)
            Result(Outcome.COMPLETED, process.exitValue(), output.toString())
        } catch (e: Exception) {
            Result(Outcome.LAUNCH_FAILED, null, (e.message ?: e.javaClass.simpleName).take(MAX_OUTPUT_CHARS))
        }
    }

    /**
     * Reads [stream] to completion so the child never blocks on a full pipe, keeping a bounded
     * prefix.
     *
     * Runs on a bare `Thread`, where an escaping exception is fatal to the whole app rather than to
     * the probe. Timing out is the normal case that produces one: `destroyForcibly()` closes this
     * stream while the read is parked in it, and the read fails with
     * `InterruptedIOException: read interrupted by close() on another thread`. That crashed Verb a
     * few seconds after launch once Codex's readiness probe began running through `qemu-aarch64`,
     * which is slow enough to exceed the bound reliably.
     *
     * Whatever has been captured so far is still the probe's output, and the timeout is already
     * reported by the caller, so there is nothing to propagate: the drain simply stops.
     */
    private fun drainBounded(stream: InputStream, sink: StringBuilder) = runCatching {
        val charBuffer = CharArray(1024)
        var captured = 0
        stream.bufferedReader().use { reader ->
            while (true) {
                val read = reader.read(charBuffer)
                if (read == -1) break
                if (captured >= MAX_OUTPUT_CHARS) continue
                val take = minOf(read, MAX_OUTPUT_CHARS - captured)
                sink.append(charBuffer, 0, take)
                captured += take
                if (captured >= MAX_OUTPUT_CHARS) sink.append("...[truncated]")
            }
        }
    }.getOrElse { }
}
