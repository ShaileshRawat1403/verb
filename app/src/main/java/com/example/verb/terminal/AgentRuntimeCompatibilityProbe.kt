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
    fun check(runtime: AgentRuntimeInstaller.InstalledRuntime): AgentCompatibilityState =
        checkCommand(runtime, AGENT_PROBE_COMMAND)

    /**
     * Narrow admission check used only to enter the runtime long enough to install an agent whose
     * own catalog probe will then decide whether that agent is usable.
     *
     * It exists to break a circular dependency: Verb would otherwise refuse to install `agy` until
     * `agy --version` succeeds, while that command cannot exist until Verb has entered the runtime
     * and installed it. A working shell is not evidence that any particular agent works -- that
     * remains each profile's own probe, reported on its own card.
     */
    fun checkShellForProfileInstallation(
        runtime: AgentRuntimeInstaller.InstalledRuntime
    ): AgentCompatibilityState = checkCommand(runtime, SHELL_PROBE_COMMAND)

    private fun checkCommand(
        runtime: AgentRuntimeInstaller.InstalledRuntime,
        command: List<String>
    ): AgentCompatibilityState {
        val workspace = probeWorkspace() ?: return AgentCompatibilityState.CHECK_FAILED
        // Probes the backend the session will actually use, so "compatible" is never claimed on
        // the strength of a different launch path than the one the user gets.
        val environment = runCatching {
            QemuAgentRuntimeEnvironment(filesDir, workspace, runtime.manifest)
                .resolveGuestCommand(runtime.rootfs, command)
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
        /**
         * The runtime-wide question is "can anything execute in here", so the probe is the
         * runtime's own shell.
         *
         * This constant read `/usr/local/bin/claude --version` until 5 September 2026, which made a
         * Claude-specific result the whole runtime's verdict. On a Vivo I2202 the card said
         * "Installed, but incompatible on this device. This Linux runtime cannot execute inside
         * this Android app sandbox" while Antigravity was running inside that very runtime, one
         * screen away. Claude Code does not even use the Agent Runtime in the shipping product --
         * it installs into the local userland -- so the check was gating every agent on an agent
         * that never runs there.
         *
         * Whether a *particular* agent works is that agent's own catalog probe, reported on its own
         * card. This one answers only whether the sandbox can execute the rootfs at all.
         */
        internal val AGENT_PROBE_COMMAND = listOf("/bin/bash", "--version")

        /**
         * Kept as its own name because it is called at a different moment -- see
         * [checkShellForProfileInstallation] -- even though it now asks the same question as
         * [AGENT_PROBE_COMMAND]. They were always meant to be the same check.
         */
        internal val SHELL_PROBE_COMMAND = AGENT_PROBE_COMMAND

        const val TIMEOUT_MS = 5_000L
    }
}
