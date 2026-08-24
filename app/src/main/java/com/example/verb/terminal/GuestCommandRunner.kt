package com.example.verb.terminal

import java.io.File

/**
 * Runs a single, bounded, catalog-owned command inside the same proot guest the user's interactive
 * terminal uses -- same binds, same base HOME/PREFIX/PATH/LD_LIBRARY_PATH/certs/guest identity,
 * same linker setup, same working-directory resolution -- via
 * [TerminalEnvironmentResolver.resolveGuestCommand]. This is what makes a "Ready" runtime-profile
 * status truthful: it means the command actually resolved and ran inside the guest, not merely
 * that a file happened to exist on the host filesystem view of it.
 *
 * This is deliberately *not* the "exact same environment" as an interactive terminal session: it
 * never runs the interactive login shell, and never sources `.bashrc` / `.bash_profile` / any other
 * user-controlled shell startup file. Runtime profile probes resolve only from Verb's own
 * controlled base PATH ([TerminalEnvironmentResolver]'s `env` wrapper). This boundary is
 * intentional, not an oversight: sourcing arbitrary user shell files here would mean app-management
 * code executing user-authored shell configuration, which is exactly the unbounded surface a
 * catalog-owned, argv-only probe is supposed to avoid. Do not change this to source user startup
 * files -- if a probe needs a PATH entry, put it in Verb's base PATH, not here.
 *
 * Security invariants (do not relax without re-reading the Runtime Truth sprint notes):
 * - Only [RuntimeRequirement]s with a non-null [RuntimeRequirement.versionProbeArgs] can be probed;
 *   anything else is refused. Those arg lists are literal constants in RuntimeProfiles.kt, never
 *   user- or AI-supplied.
 * - Commands are always run as an argv array (ProcessBuilder(List<String>)); nothing is ever passed
 *   through a shell (`sh -c`), so there is no interpolation surface.
 * - Every probe is bounded to at most [MAX_TIMEOUT_MS] wall-clock milliseconds.
 * - Captured output is bounded to [MAX_OUTPUT_CHARS]; the child's stream is still drained to
 *   completion (just discarded past the bound) so a chatty process can never deadlock on a full
 *   pipe.
 * - This class has no dependency on, and must never be called from, any AI/assistant code path
 *   (see [TerminalAiHelper], which accepts structural command metadata only). It is wired solely
 *   from [RuntimeCapabilityDetector], itself driven only by explicit user action in the Runtime
 *   Capabilities UI.
 */
class GuestCommandRunner(private val resolver: TerminalEnvironmentResolver) {

    constructor(filesDir: File) : this(TerminalEnvironmentResolver(filesDir))

    enum class Outcome {
        /** Resolved in the guest PATH, ran, and exited 0. */
        READY,
        /** The guest's `env` reported the command was not found on the guest PATH (exit 127). */
        NOT_FOUND,
        /** The guest's `env` found the command but could not execute it (exit 126) -- present on
         *  PATH without the executable bit set, or not a valid binary for this guest. */
        NOT_EXECUTABLE,
        /** Ran and exited nonzero (other than the two `env` conventions above). */
        NONZERO_EXIT,
        /** Exceeded the bound before finishing; the process was forcibly destroyed. */
        TIMEOUT,
        /** The proot/login/termux-exec userland is not installed, so there is nothing to probe. */
        GUEST_UNAVAILABLE,
        /** The requirement has no catalog-registered probe; refused before anything was run. */
        REFUSED,
        /** The probe process itself could not be started (e.g. proot binary broken/corrupt). */
        LAUNCH_FAILED
    }

    data class GuestProbeResult(
        val outcome: Outcome,
        val exitCode: Int?,
        val output: String
    ) {
        val succeeded: Boolean get() = outcome == Outcome.READY
    }

    /**
     * Probes [requirement] using its catalog-declared [RuntimeRequirement.versionProbeArgs].
     * Refuses (without spawning anything) if the requirement has no registered probe. Runs the
     * bare command name -- never a resolved host path -- through the guest's `env`, so PATH
     * resolution happens inside the guest exactly as it would for a user typing the command.
     */
    fun probe(requirement: RuntimeRequirement, timeoutMs: Long = DEFAULT_TIMEOUT_MS): GuestProbeResult {
        val args = requirement.versionProbeArgs
            ?: return GuestProbeResult(Outcome.REFUSED, null, "no registered probe for '${requirement.command}'")
        val environment = resolver.resolveGuestCommand(listOf(requirement.command) + args)
            ?: return GuestProbeResult(Outcome.GUEST_UNAVAILABLE, null, "guest userland is not installed")
        return execute(environment, timeoutMs.coerceIn(1, MAX_TIMEOUT_MS))
    }

    /** Visible to tests so execution mechanics (timeout/bounded output) can be verified against a
     * hand-built [TerminalEnvironment] without needing a real installed guest userland. */
    internal fun execute(environment: TerminalEnvironment, timeoutMs: Long): GuestProbeResult {
        val result = BoundedProcessRunner.run(
            argv = environment.arguments.toList(),
            environment = environment.variables,
            workingDirectory = environment.workingDirectory,
            timeoutMs = timeoutMs
        )
        return when (result.outcome) {
            BoundedProcessRunner.Outcome.TIMEOUT ->
                GuestProbeResult(Outcome.TIMEOUT, null, result.output)
            BoundedProcessRunner.Outcome.LAUNCH_FAILED ->
                GuestProbeResult(Outcome.LAUNCH_FAILED, null, result.output)
            BoundedProcessRunner.Outcome.COMPLETED -> {
                // The guest `env` wrapper's own exit conventions are what distinguish "not on PATH"
                // from "on PATH but not runnable" -- interpretation stays here, with the catalog.
                val exit = result.exitCode
                val outcome = when (exit) {
                    0 -> Outcome.READY
                    ENV_COMMAND_NOT_FOUND -> Outcome.NOT_FOUND
                    ENV_COMMAND_NOT_EXECUTABLE -> Outcome.NOT_EXECUTABLE
                    else -> Outcome.NONZERO_EXIT
                }
                GuestProbeResult(outcome, exit, result.output)
            }
        }
    }

    companion object {
        /**
         * Upper bound for any probe. Raised from 3s once Codex began running through
         * `qemu-aarch64`: emulated startup on a large static binary regularly exceeded the old
         * bound, so a working agent probed as absent. Individual requirements still ask for less
         * (the default), so only the agents that need the headroom pay for it -- see
         * [RuntimeRequirement.probeTimeoutMs].
         */
        const val MAX_TIMEOUT_MS = 15_000L

        /** What a probe gets when its requirement does not ask for more. */
        const val DEFAULT_TIMEOUT_MS = 3_000L
        const val MAX_OUTPUT_CHARS = BoundedProcessRunner.MAX_OUTPUT_CHARS

        /** POSIX `env`'s exit code convention for "command not found" via PATH search. */
        private const val ENV_COMMAND_NOT_FOUND = 127

        /** POSIX `env`'s exit code convention for "command found but not executable". */
        private const val ENV_COMMAND_NOT_EXECUTABLE = 126
    }
}
