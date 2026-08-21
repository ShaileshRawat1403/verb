package com.example.verb.session

import com.example.verb.project.VerbProject
import com.example.verb.terminal.CommandLifecycleState
import com.example.verb.terminal.TerminalRuntimeAdapter
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.io.File
import java.time.Instant
import java.util.UUID

/**
 * Owns the one Claude [VerbSession] the UI can see, and the watch that keeps it truthful. This is
 * the first thing in the codebase that constructs a real [VerbSession] -- everything before this
 * (`VerbSessionStateResolver`, `ClaudeAgentAdapter`, `VerbSessionResumer`) proved the contract in
 * isolation; this is where it starts driving what a user actually sees.
 *
 * Deliberately not [VerbTerminalSessionHolder]: that owns the one live `TerminalRuntime` for the
 * process. This owns product-level identity for one agent's session within it. They stay separate
 * on purpose -- see `docs/VERB_SESSION_CONTRACT.md`'s "One conceptual distinction to preserve".
 *
 * Scope, deliberately narrow: one agent (Claude), one session at a time, no persistence. Starting a
 * new session (from launch or from ENDED) replaces whatever was tracked before.
 */
class ClaudeSessionCoordinator(
    private val filesDir: File,
    private val terminalRuntimeAdapter: TerminalRuntimeAdapter,
    private val coroutineScope: CoroutineScope
) {
    private val _session = MutableStateFlow<VerbSession?>(null)
    val session: StateFlow<VerbSession?> = _session.asStateFlow()

    /** The project the *tracked session* launched under -- never the UI's currently selected one. */
    private var sessionProjectDirectory: File? = null

    private var watchJob: Job? = null

    /**
     * Cancels the current watch, if any. Production relies on the owning `CoroutineScope`'s own
     * cancellation (see the class doc); this exists for tests, where a launch that is never driven
     * to a settled outcome would otherwise poll forever past the end of the test.
     */
    fun cancelWatch() {
        watchJob?.cancel()
    }

    /**
     * Call immediately after sending the command that launches Claude, with the command-history
     * snapshot captured immediately *before* sending it (so the watch below can tell which new
     * record is Claude's).
     *
     * Always produces a fresh [VerbSession.id]: this is the only place one is minted, whether this
     * is the very first launch or "Start new" after [VerbSessionState.ENDED].
     */
    fun onLaunched(project: VerbProject?, idsBeforeLaunch: Set<String>) {
        sessionProjectDirectory = project?.directory
        val agent = AgentRef(agentType = "claude", resumeIdentity = null)
        val now = Instant.now()
        _session.value = VerbSession(
            id = UUID.randomUUID().toString(),
            projectId = project?.id,
            runtime = "claude",
            createdAt = now,
            lastSeenAt = now,
            state = VerbSessionState.LIVE,
            agent = agent,
            process = LiveClaudeBinding
        )
        watchForExit(idsBeforeLaunch)
    }

    /**
     * Resumes the tracked session via [ClaudeAgentAdapter] / [VerbSessionResumer]. A no-op unless
     * the tracked session is [VerbSessionState.RECOVERABLE] -- resume is not a shortcut around
     * [VerbSessionState.INTERRUPTED] resolving first, and there is nothing to resume from
     * [VerbSessionState.LIVE] or [VerbSessionState.ENDED].
     */
    suspend fun resume() {
        val current = _session.value ?: return
        if (current.state != VerbSessionState.RECOVERABLE) return

        val idsBefore = terminalRuntimeAdapter.commandHistory.value.mapTo(mutableSetOf()) { it.id }
        val resumed = VerbSessionResumer.resume(current, adapter())
        _session.value = resumed
        if (resumed.state == VerbSessionState.LIVE) {
            watchForExit(idsBefore)
        }
    }

    /**
     * Watches for Claude -- not the whole terminal session -- exiting. `commandHistory` is used
     * rather than `isSessionActive` deliberately: the underlying shell stays alive the whole time
     * Claude runs as its foreground child, so shell liveness cannot distinguish "Claude is still
     * running" from "Claude exited and the shell is back at its prompt." See `ClaudeAgentAdapter`
     * for the same reasoning applied to resume detection.
     */
    private fun watchForExit(idsBefore: Set<String>) {
        watchJob?.cancel()
        watchJob = coroutineScope.launch {
            // Unbounded, deliberately: a genuinely live Claude session can run for however long
            // the user is working, so there is no timeout to wrap this in the way resume() has one.
            while (true) {
                val settled = terminalRuntimeAdapter.commandHistory.value.firstOrNull {
                    it.id !in idsBefore && it.state != CommandLifecycleState.RUNNING
                }
                if (settled != null) break
                delay(EXIT_POLL_INTERVAL_MS)
            }

            _session.value = _session.value?.copy(process = null, lastSeenAt = Instant.now())
            resolveAfterExit()
        }
    }

    /**
     * [VerbSessionState.INTERRUPTED] is a waiting state, not a resting one (per
     * `docs/VERB_SESSION_CONTRACT.md`): Claude's transcript write can lag its exit by a moment, so
     * an immediate `canResume` check landing on `UNKNOWN` is retried a bounded number of times
     * before this gives up and leaves the session honestly `INTERRUPTED` rather than guessing
     * `ENDED`. Each attempt still updates [session] -- that bounded window is exactly the
     * "Checking recovery status…" moment the UI shows.
     */
    private suspend fun resolveAfterExit() {
        val agent = _session.value?.agent ?: return
        repeat(RESOLVE_ATTEMPTS) { attempt ->
            val current = _session.value ?: return
            val verdict = adapter().canResume(agent)
            val resolvedState = VerbSessionStateResolver.resolve(processPresent = false, agent, verdict)
            _session.value = current.copy(lastSeenAt = Instant.now(), state = resolvedState)
            if (resolvedState != VerbSessionState.INTERRUPTED) return
            if (attempt < RESOLVE_ATTEMPTS - 1) delay(RESOLVE_RETRY_DELAY_MS)
        }
    }

    private fun adapter(): ClaudeAgentAdapter =
        ClaudeAgentAdapter(filesDir, sessionProjectDirectory, terminalRuntimeAdapter)

    private object LiveClaudeBinding : ProcessBinding

    private companion object {
        const val EXIT_POLL_INTERVAL_MS = 500L
        const val RESOLVE_ATTEMPTS = 10
        const val RESOLVE_RETRY_DELAY_MS = 500L
    }
}
