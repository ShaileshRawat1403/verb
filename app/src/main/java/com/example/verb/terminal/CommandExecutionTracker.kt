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

    /**
     * The shell's last reported working directory, as a guest path, or null when it is genuinely
     * unknown -- before the first valid OSC 7 of a session, and again after the session ends.
     *
     * Only a valid [ShellIntegrationEvent.CurrentDirectory] ever sets this. It is never inferred
     * from prompt text, transcript contents, the session's launch arguments, or
     * `TerminalSession.getCwd()` (which reports the PTY leader's cwd -- proot's, not the guest
     * shell's -- and would therefore be wrong rather than merely unavailable).
     */
    private val _currentWorkingDirectory = MutableStateFlow<String?>(null)
    val currentWorkingDirectory: StateFlow<String?> = _currentWorkingDirectory.asStateFlow()

    private var runningRecord: CommandExecutionRecord? = null
    private var pendingCommandText: String? = null

    @Synchronized
    fun onEvent(event: ShellIntegrationEvent) {
        when (event) {
            is ShellIntegrationEvent.CurrentDirectory -> _currentWorkingDirectory.value = event.path
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
     *
     * [currentWorkingDirectory] is cleared, not preserved: it describes where the *shell* is right
     * now, and once the session is gone there is no shell. Carrying it over would let a restarted
     * or reconfigured session report the previous session's directory as if it were live -- and
     * would stamp it onto the first command of the new session, which may well start somewhere
     * else entirely (a project switch and an Agent Runtime activation both restart the PTY).
     */
    @Synchronized
    fun onSessionEnded() {
        abandonRunningRecord()
        pendingCommandText = null
        _currentWorkingDirectory.value = null
        _shellIntegrationActive.value = false
    }

    private fun startRecord() {
        // A record already running with no matching END (forged/out-of-order C, or a shell that
        // never got to emit D) is abandoned, never silently overwritten or merged with the new one.
        abandonRunningRecord()
        runningRecord = CommandExecutionRecord(
            id = UUID.randomUUID().toString(),
            commandText = pendingCommandText.orEmpty(),
            workingDirectory = _currentWorkingDirectory.value,
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
