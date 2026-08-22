package com.example.verb.terminal

import java.io.File

/**
 * Answers one question, by running something rather than by inspecting files: can this installed
 * Agent Runtime actually execute inside this app's sandbox?
 *
 * [AgentRuntimeInstaller] already verifies the archive digest, extracts it, and confirms every
 * command the manifest declares exists and is executable *as a file*. On the validation device all
 * of that passed while every real invocation died immediately -- so file checks are not evidence of
 * executability, and "Installed" must not imply "Ready".
 *
 * The probe runs `/bin/bash --version` through [AgentRuntimeEnvironment.resolveGuestCommand], which
 * is the same construction site the interactive session uses, so the binds, rootfs, guest
 * environment and loader isolation under test are exactly the ones the user is about to get.
 *
 * Boundaries (do not relax):
 * - The command is a hard-coded constant. Nothing here accepts user, project, or AI input.
 * - argv-only via [BoundedProcessRunner]; never `sh -c`, so there is no interpolation surface.
 * - Bounded to [TIMEOUT_MS] wall clock, with bounded captured output.
 * - Runs `/bin/bash --version`, not a login or interactive shell, so no guest startup file
 *   (`/etc/profile`, `~/.bash_profile`, ...) is ever sourced by app-management code.
 * - The workspace bind is an app-private throwaway directory, not the user's selected project, so a
 *   check never touches project contents.
 * - Never called from any AI path, and never logs argv, environment, paths, or captured output.
 */
class AgentRuntimeCompatibilityProbe(
    private val filesDir: File,
    private val paths: AgentRuntimePaths = AgentRuntimePaths(filesDir)
) {

    /**
     * Runs the check and maps the outcome onto the closed compatibility model.
     *
     * A nonzero exit is [AgentCompatibilityState.INCOMPATIBLE], not a failure of the check: the
     * check succeeded, and its answer is "this cannot run here". [AgentCompatibilityState.CHECK_FAILED]
     * is reserved for the case where nothing could be concluded because the probe never ran.
     */
    fun check(runtime: AgentRuntimeInstaller.InstalledRuntime): AgentCompatibilityState {
        val workspace = probeWorkspace() ?: return AgentCompatibilityState.CHECK_FAILED
        // Probes the backend the session will actually use, so "compatible" is never claimed on
        // the strength of a different launch path than the one the user gets.
        val environment = runCatching {
            QemuAgentRuntimeEnvironment(filesDir, workspace, runtime.manifest)
                .resolveGuestCommand(runtime.rootfs, PROBE_COMMAND)
        }.getOrNull() ?: return AgentCompatibilityState.CHECK_FAILED

        val result = BoundedProcessRunner.run(
            argv = environment.arguments.toList(),
            environment = environment.variables,
            workingDirectory = environment.workingDirectory,
            timeoutMs = TIMEOUT_MS
        )

        val state = when (result.outcome) {
            BoundedProcessRunner.Outcome.TIMEOUT -> AgentCompatibilityState.CHECK_TIMED_OUT
            BoundedProcessRunner.Outcome.LAUNCH_FAILED -> AgentCompatibilityState.CHECK_FAILED
            BoundedProcessRunner.Outcome.COMPLETED ->
                if (result.exitCode == 0) {
                    AgentCompatibilityState.COMPATIBLE
                } else {
                    AgentCompatibilityState.INCOMPATIBLE
                }
        }

        // Metadata only. The exit code and the resulting state are safe; argv, environment, guest
        // paths and captured output are not, and are deliberately never logged.
        TerminalSessionLogger.info(
            LogCategory.DIAGNOSTIC,
            "Agent Runtime compatibility check finished: state=$state exitCode=${result.exitCode ?: "none"}"
        )
        return state
    }

    /**
     * A throwaway, app-private directory to bind at `/workspace` for the duration of the check.
     * Reused rather than recreated per check, and never the user's selected project.
     */
    private fun probeWorkspace(): File? = runCatching {
        File(paths.root, "compat-probe").apply { mkdirs() }.takeIf { it.isDirectory }
    }.getOrNull()

    companion object {
        /**
         * Probes an actual agent launcher, not `/bin/bash`.
         *
         * Bash was the original probe and it was too weak to be meaningful: it survives on devices
         * where no agent can run. Measured on the validation device, inside the app process, the
         * runtime reaches a working Debian shell while the agent launchers die with `SIGSYS` --
         * Android's seccomp policy permits the small syscall set bash uses and refuses the ones the
         * Bun-compiled agents make. A probe that answers "yes" there would recreate exactly the
         * failure this class exists to prevent, one level deeper: a launch button for a runtime that
         * opens a shell but cannot run the thing the user opened it for.
         *
         * `--version` still starts the real binary and exits on its own, so nothing is authenticated
         * and no network call is required.
         */
        private val PROBE_COMMAND = listOf("/usr/local/bin/claude", "--version")

        const val TIMEOUT_MS = 5_000L
    }
}
