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
 * Owns the one [VerbSession] the UI can see for a single agent, and the watch that keeps it
 * truthful. Agent-neutral by construction: everything here is lifecycle semantics from
 * `docs/VERB_SESSION_CONTRACT.md`, and every agent-specific fact -- how the agent is launched, what
 * counts as recovery evidence, how it resumes -- lives behind the [AgentAdapter] this is built
 * with. That split is the whole point: there must be exactly one session state machine, however
 * many agents Verb supports (see `ClaudeSessionCoordinator` / `CodexSessionCoordinator` for how an
 * agent binds itself to it).
 *
 * Deliberately not [VerbTerminalSessionHolder]: that owns the one live `TerminalRuntime` for the
 * process. This owns product-level identity for one agent's session within it. They stay separate
 * on purpose -- see `docs/VERB_SESSION_CONTRACT.md`'s "One conceptual distinction to preserve".
 *
 * Scope, deliberately narrow: one session at a time per agent. The product-level record is
 * persisted, but its process binding is not -- startup must re-establish that from the host.
 */
class AgentSessionCoordinator(
    private val agentType: String,
    private val adapterFactory: (projectDirectory: File?) -> AgentAdapter,
    private val terminalRuntimeAdapter: TerminalRuntimeAdapter,
    private val coroutineScope: CoroutineScope,
    private val sessionStore: VerbSessionStore = InMemoryVerbSessionStore(),
    private val processBindingConfirmed: Boolean = false
) {
    private val _session = MutableStateFlow<VerbSession?>(null)
    val session: StateFlow<VerbSession?> = _session.asStateFlow()

    /** The project the *tracked session* launched under -- never the UI's currently selected one. */
    private var sessionProjectDirectory: File? = null

    private var watchJob: Job? = null

    init {
        restorePersistedSession()
    }

    /**
     * Cancels the current watch, if any. Production relies on the owning `CoroutineScope`'s own
     * cancellation (see the class doc); this exists for tests, where a launch that is never driven
     * to a settled outcome would otherwise poll forever past the end of the test.
     */
    fun cancelWatch() {
        watchJob?.cancel()
    }

    /**
     * Call immediately after sending the command that launches the agent, with the command-history
     * snapshot captured immediately *before* sending it (so the watch below can tell which new
     * record is the agent's).
     *
     * Always produces a fresh [VerbSession.id]: this is the only place one is minted, whether this
     * is the very first launch or "Start new" after [VerbSessionState.ENDED].
     */
    fun onLaunched(project: VerbProject?, idsBeforeLaunch: Set<String>) {
        sessionProjectDirectory = project?.directory
        val agent = AgentRef(agentType = agentType, resumeIdentity = null)
        val now = Instant.now()
        _session.value = VerbSession(
            id = UUID.randomUUID().toString(),
            projectId = project?.id,
            runtime = agentType,
            createdAt = now,
            lastSeenAt = now,
            state = VerbSessionState.LIVE,
            lastKnownCwd = project?.directory?.absolutePath,
            lastObservedAt = project?.directory?.let { now },
            agent = agent,
            process = LiveAgentBinding
        )
        sessionStore.save(_session.value!!)
        VerbTerminalSessionHolder.claimForeground(agentType)
        watchForExit(idsBeforeLaunch)
    }

    /**
     * Resumes the tracked session via the agent's [AgentAdapter] / [VerbSessionResumer]. A no-op
     * unless the tracked session is [VerbSessionState.RECOVERABLE] -- resume is not a shortcut
     * around [VerbSessionState.INTERRUPTED] resolving first, and there is nothing to resume from
     * [VerbSessionState.LIVE] or [VerbSessionState.ENDED].
     */
    suspend fun resume() {
        val current = _session.value ?: return
        if (current.state != VerbSessionState.RECOVERABLE) return

        val idsBefore = terminalRuntimeAdapter.commandHistory.value.mapTo(mutableSetOf()) { it.id }
        val resumed = VerbSessionResumer.resume(current, adapter())
        _session.value = resumed
        sessionStore.save(resumed)
        if (resumed.state == VerbSessionState.LIVE) {
            VerbTerminalSessionHolder.claimForeground(agentType)
            watchForExit(idsBefore)
        }
    }

    /**
     * Re-checks recovery for a session whose process is gone. [VerbSessionState.INTERRUPTED] means
     * "no answer yet", and the answer can arrive after the bounded retries in [resolveAfterExit]
     * have run out -- an agent can finish writing its own state late, or (as with Codex) only start
     * recording once the user has signed in. Without this, a stale INTERRUPTED would sit there
     * claiming Verb is still checking when nothing is.
     *
     * Cheap and idempotent, so callers can drive it from something as ordinary as opening the
     * Agents screen. A [VerbSessionState.LIVE] session is left alone: its truth is the process
     * binding, not a disk scan.
     */
    fun refresh() {
        val current = _session.value ?: return
        if (current.process != null) return
        coroutineScope.launch { resolveAfterExit() }
    }

    /**
     * Watches for the agent -- not the whole terminal session -- exiting. `commandHistory` is used
     * rather than `isSessionActive` deliberately: the underlying shell stays alive the whole time
     * the agent runs as its foreground child, so shell liveness cannot distinguish "the agent is
     * still running" from "the agent exited and the shell is back at its prompt." See
     * [AgentResumeLauncher] for the same reasoning applied to resume detection.
     */
    private fun watchForExit(idsBefore: Set<String>) {
        watchJob?.cancel()
        watchJob = coroutineScope.launch {
            // Unbounded, deliberately: a genuinely live agent session can run for however long
            // the user is working, so there is no timeout to wrap this in the way resume() has one.
            while (true) {
                val settled = terminalRuntimeAdapter.commandHistory.value.firstOrNull {
                    it.id !in idsBefore && it.state != CommandLifecycleState.RUNNING
                }
                if (settled != null) break
                delay(EXIT_POLL_INTERVAL_MS)
            }

            VerbTerminalSessionHolder.releaseForeground(agentType)
            _session.value = _session.value?.copy(process = null, lastSeenAt = Instant.now())
            _session.value?.let(sessionStore::save)
            resolveAfterExit()
        }
    }

    /**
     * [VerbSessionState.INTERRUPTED] is a waiting state, not a resting one (per
     * `docs/VERB_SESSION_CONTRACT.md`): an agent's transcript write can lag its exit by a moment,
     * so an immediate `canResume` check landing on `UNKNOWN` is retried a bounded number of times
     * before this gives up and leaves the session honestly `INTERRUPTED` rather than guessing
     * `ENDED`. Each attempt still updates [session] -- that bounded window is exactly the
     * "Checking recovery status…" moment the UI shows.
     */
    private suspend fun resolveAfterExit() {
        val agent = _session.value?.agent ?: return
        repeat(RESOLVE_ATTEMPTS) { attempt ->
            val current = _session.value ?: return
            val adapter = adapter()
            val identity = agent.resumeIdentity ?: adapter.resumeIdentity(agent)
            val resolvedAgent = agent.copy(resumeIdentity = identity)
            val verdict = adapter.canResume(resolvedAgent)
            val resolvedState = VerbSessionStateResolver.resolve(processPresent = false, resolvedAgent, verdict)
            _session.value = current.copy(
                lastSeenAt = Instant.now(),
                state = resolvedState,
                agent = resolvedAgent
            )
            sessionStore.save(_session.value!!)
            if (resolvedState != VerbSessionState.INTERRUPTED) return
            if (attempt < RESOLVE_ATTEMPTS - 1) delay(RESOLVE_RETRY_DELAY_MS)
        }
    }

    private fun adapter(): AgentAdapter = adapterFactory(sessionProjectDirectory)

    /**
     * Restores identity after process death, then asks the host whether the old binding is still
     * attached. A new Android process has no existing [VerbTerminalSessionHolder] binding, even
     * though it immediately creates a new shell for the UI; that new shell cannot prove the old
     * agent process survived.
     */
    private fun restorePersistedSession() {
        val persisted = sessionStore.load() ?: return
        if (persisted.agent?.agentType?.let { it != agentType } == true) return
        sessionProjectDirectory = persisted.lastKnownCwd?.let(::File)

        // A surviving terminal is not evidence that *this* agent survived with it. Two agents with
        // persisted records would otherwise both restore as LIVE from the same PTY, which is how the
        // Agents tab came to show Claude and Codex both "Running" while neither process existed.
        // The marker beside the runtime says which agent actually holds it.
        val bindingStillAttached = processBindingConfirmed &&
            terminalRuntimeAdapter.isSessionActive.value &&
            terminalRuntimeAdapter.sessionState.value == com.example.verb.terminal.TerminalSessionState.RUNNING &&
            VerbTerminalSessionHolder.foregroundAgent() == agentType
        val restored = if (bindingStillAttached) {
            persisted.copy(
                state = VerbSessionState.LIVE,
                process = LiveAgentBinding,
                lastSeenAt = Instant.now()
            )
        } else {
            val agent = persisted.agent?.let { persistedAgent ->
                val identity = persistedAgent.resumeIdentity
                    ?: adapter().resumeIdentity(persistedAgent)
                persistedAgent.copy(resumeIdentity = identity)
            }
            val verdict = agent?.let { adapter().canResume(it) }
            persisted.copy(
                state = VerbSessionStateResolver.resolve(
                    processPresent = false,
                    agent = agent,
                    resumeVerdict = verdict
                ),
                process = null,
                agent = agent,
                lastSeenAt = Instant.now()
            )
        }
        _session.value = restored
        sessionStore.save(restored)

        if (bindingStillAttached && restored.state == VerbSessionState.LIVE) {
            val idsBefore = terminalRuntimeAdapter.commandHistory.value.mapTo(mutableSetOf()) { it.id }
            watchForExit(idsBefore)
        }
    }

    private object LiveAgentBinding : ProcessBinding

    private companion object {
        const val EXIT_POLL_INTERVAL_MS = 500L
        const val RESOLVE_ATTEMPTS = 10
        const val RESOLVE_RETRY_DELAY_MS = 500L
    }
}
