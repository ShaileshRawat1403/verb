package com.example.verb.terminal

enum class CommandLifecycleState {
    RUNNING,
    COMPLETED,
    FAILED,
    /** Session ended or restarted while this record was still running; no exit code was ever seen. */
    ABANDONED
}

/**
 * A single local, in-memory command lifecycle record built from advisory shell-integration
 * markers. Deliberately holds no transcript or output text -- see [com.example.verb.terminal.CommandExecutionTracker]
 * for the bounded, non-persistent history this is stored in.
 */
data class CommandExecutionRecord(
    val id: String,
    val commandText: String,
    val workingDirectory: String?,
    val startedAtEpochMs: Long,
    val endedAtEpochMs: Long? = null,
    val exitCode: Int? = null,
    val state: CommandLifecycleState = CommandLifecycleState.RUNNING
) {
    val durationMs: Long?
        get() = endedAtEpochMs?.let { it - startedAtEpochMs }
}
