package com.example.verb.terminal

import com.example.verb.semantic.SecretGuard
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.util.UUID

/**
 * Session-local, in-memory-only tracker for [CommandExecutionRecord]s built from
 * [ShellIntegrationEvent]s. No persistence, no transcript/output storage, and never forwarded to
 * any AI provider -- this is local execution metadata only (see docs/... Shell Awareness P0
 * scope). Bounded to [MAX_HISTORY] records; oldest entries are dropped first.
 *
 * Markers are advisory and can arrive out of order or be entirely forged by any process writing
 * to the PTY (see [ShellIntegrationEvent]'s own doc). This tracker is written so that no sequence
 * of events -- missing START, missing END, repeated START, END with nothing running -- can throw
 * or silently merge two unrelated commands into one record.
 */
class CommandExecutionTracker {

    private val _history = MutableStateFlow<List<CommandExecutionRecord>>(emptyList())
    val history: StateFlow<List<CommandExecutionRecord>> = _history.asStateFlow()

    private val _shellIntegrationActive = MutableStateFlow(false)
    val shellIntegrationActive: StateFlow<Boolean> = _shellIntegrationActive.asStateFlow()

    private var runningRecord: CommandExecutionRecord? = null
    private var pendingCommandText: String? = null
    private var lastKnownCwd: String? = null

    @Synchronized
    fun onEvent(event: ShellIntegrationEvent) {
        when (event) {
            is ShellIntegrationEvent.CurrentDirectory -> lastKnownCwd = event.path
            is ShellIntegrationEvent.CommandMetadata -> pendingCommandText = redact(event.commandText)
            ShellIntegrationEvent.CommandStart -> startRecord()
            is ShellIntegrationEvent.CommandEnd -> endRecord(event.exitCode)
            ShellIntegrationEvent.Handshake -> _shellIntegrationActive.value = true
            ShellIntegrationEvent.PromptStart, ShellIntegrationEvent.PromptEnd -> Unit
        }
    }

    /**
     * Called when the session is destroyed or restarted (see
     * [TermuxTerminalRuntimeAdapter.destroy]): any in-flight record is closed as [CommandLifecycleState.ABANDONED]
     * rather than left RUNNING forever. Past history is intentionally preserved across a restart
     * (still process-lifetime-only, never persisted) since the records describe commands that did
     * run, not the session's own lifecycle.
     */
    @Synchronized
    fun onSessionEnded() {
        abandonRunningRecord()
        pendingCommandText = null
        _shellIntegrationActive.value = false
    }

    private fun startRecord() {
        // A record already running with no matching END (forged/out-of-order C, or a shell that
        // never got to emit D) is abandoned, never silently overwritten or merged with the new one.
        abandonRunningRecord()
        runningRecord = CommandExecutionRecord(
            id = UUID.randomUUID().toString(),
            commandText = pendingCommandText.orEmpty(),
            workingDirectory = lastKnownCwd,
            startedAtEpochMs = System.currentTimeMillis()
        )
        pendingCommandText = null
    }

    private fun endRecord(exitCode: Int) {
        val record = runningRecord ?: return // D with nothing running: ignore, never fabricate a record.
        runningRecord = null
        append(
            record.copy(
                endedAtEpochMs = System.currentTimeMillis(),
                exitCode = exitCode,
                state = if (exitCode == 0) CommandLifecycleState.COMPLETED else CommandLifecycleState.FAILED
            )
        )
    }

    private fun abandonRunningRecord() {
        val record = runningRecord ?: return
        runningRecord = null
        append(record.copy(endedAtEpochMs = System.currentTimeMillis(), state = CommandLifecycleState.ABANDONED))
    }

    private fun redact(commandText: String): String =
        SecretGuard.redactKnownSensitiveText(commandText.take(ShellIntegrationParser.MAX_COMMAND_TEXT_LENGTH))

    private fun append(record: CommandExecutionRecord) {
        val updated = _history.value + record
        _history.value = if (updated.size > MAX_HISTORY) updated.subList(updated.size - MAX_HISTORY, updated.size) else updated
    }

    companion object {
        const val MAX_HISTORY = 50
    }
}
