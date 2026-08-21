package com.example.verb.session

import com.example.verb.terminal.CommandLifecycleState
import com.example.verb.terminal.TerminalRuntimeAdapter
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
 * The check is inverted from the usual "wait for completion" pattern (compare
 * `VerbViewModel.installOneProfile`), because resuming launches an interactive session rather than
 * running a script to completion: **a new, settled command-history record appearing at all -- with
 * any exit code -- means the agent already exited**, so resume did not produce a live session. No
 * such record appearing within the window is the expected shape of success, since a genuinely
 * resumed, still-running agent will not return to the prompt until the user quits, arbitrarily
 * later.
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
        val idsBefore = terminalRuntimeAdapter.commandHistory.value.mapTo(mutableSetOf()) { it.id }

        terminalRuntimeAdapter.sendCommand(command)

        val exitedWithinWindow = withTimeoutOrNull(settleMs) {
            while (true) {
                val settled = terminalRuntimeAdapter.commandHistory.value.firstOrNull {
                    it.id !in idsBefore && it.state != CommandLifecycleState.RUNNING
                }
                if (settled != null) break
                delay(pollIntervalMs)
            }
        }

        return exitedWithinWindow == null
    }
}
