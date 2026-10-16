package com.example.verb.session

import com.example.verb.terminal.CommandLifecycleState
import com.example.verb.terminal.TerminalRuntimeAdapter
import com.example.verb.terminal.TerminalSessionState
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull

/**
 * Runs an agent's resume command in the user's real terminal and decides whether it produced a
 * live session. Shared by every [AgentAdapter], because the reasoning below is about how a PTY and
 * shell integration behave, not about any one agent.
 *
 * Deliberately reads [TerminalRuntimeAdapter.commandHistory], not [TerminalRuntimeAdapter.terminalOutput]:
 * `terminalOutput` mirrors the terminal emulator's own screen, which echoes typed input as soon as
 * it is written to the PTY -- so a text marker embedded in the very command being sent would appear
 * to "complete" instantly, before the shell had done anything at all. `commandHistory` is driven by
 * real OSC 633 command-boundary events the shell's own prompt hooks emit, which fire only when a
 * command genuinely finishes, so it is immune to that race.
 *
 * A new RUNNING command boundary is positive evidence that the shell executed the command.
 * A settled boundary means the agent exited. Silence, including absent shell integration, cannot
 * establish that the agent started and is therefore never reported as success.
 *
 * A device without shell integration active has no signal either way and this conservatively
 * reports failure -- see [AgentAdapter.resume]'s contract: a caller only ever advances to LIVE on a
 * non-null result, so "cannot confirm" and "confirmed failed" are safe to treat the same.
 */
object AgentResumeLauncher {

    suspend fun launch(
        terminalRuntimeAdapter: TerminalRuntimeAdapter,
        command: String,
        settleMs: Long,
        pollIntervalMs: Long
    ): Boolean {
        val ready = withTimeoutOrNull(settleMs) {
            while (true) {
                if (terminalRuntimeAdapter.shellIntegrationActive.value &&
                    terminalRuntimeAdapter.runningCommand.value == null &&
                    terminalRuntimeAdapter.sessionState.value == TerminalSessionState.RUNNING
                ) return@withTimeoutOrNull true
                delay(pollIntervalMs)
            }
            @Suppress("UNREACHABLE_CODE")
            false
        } ?: false
        if (!ready) return false
        val idsBefore = terminalRuntimeAdapter.commandHistory.value.mapTo(mutableSetOf()) { it.id }
        terminalRuntimeAdapter.runningCommand.value?.id?.let(idsBefore::add)

        terminalRuntimeAdapter.sendCommand(command)

        val startedId = withTimeoutOrNull(settleMs) {
            while (true) {
                val running = terminalRuntimeAdapter.runningCommand.value
                if (running != null && running.id !in idsBefore) return@withTimeoutOrNull running.id
                if (terminalRuntimeAdapter.commandHistory.value.any { it.id !in idsBefore }) return@withTimeoutOrNull null
                delay(pollIntervalMs)
            }
            @Suppress("UNREACHABLE_CODE")
            null
        }
        if (startedId == null) return false
        // Keep observing briefly: an invalid identity can start and exit almost immediately.
        val settled = withTimeoutOrNull(settleMs) {
            while (true) {
                if (terminalRuntimeAdapter.commandHistory.value.any {
                    it.id == startedId && it.state != CommandLifecycleState.RUNNING
                }) break
                delay(pollIntervalMs)
            }
        }
        return settled == null && terminalRuntimeAdapter.isSessionActive.value &&
            terminalRuntimeAdapter.runningCommand.value?.id == startedId
    }
}
