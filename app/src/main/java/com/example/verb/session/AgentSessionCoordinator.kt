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
    private val adapterFactory: (projectDirectory: File?, runtime: TerminalRuntimeAdapter?) -> AgentAdapter,
    private val terminalRuntimeProvider: (sessionId: String) -> TerminalRuntimeAdapter?,
    private val coroutineScope: CoroutineScope,
    private val sessionStore: VerbSessionStore = InMemoryVerbSessionStore(),
    private val processBindingConfirmed: Boolean = false,
    private val eventLog: VerbEventLog = VerbEventLog.Disabled
) {
    /**
     * Secondary constructor for tests or single-runtime scenarios where terminal runtime is directly known.
     */
    constructor(
        agentType: String,
        adapterFactory: (projectDirectory: File?, runtime: TerminalRuntimeAdapter?) -> AgentAdapter,
        terminalRuntimeAdapter: TerminalRuntimeAdapter,
        coroutineScope: CoroutineScope,
        sessionStore: VerbSessionStore = InMemoryVerbSessionStore(),
        processBindingConfirmed: Boolean = false,
        eventLog: VerbEventLog = VerbEventLog.Disabled
    ) : this(
        agentType = agentType,
        adapterFactory = adapterFactory,
        terminalRuntimeProvider = { terminalRuntimeAdapter },
        coroutineScope = coroutineScope,
        sessionStore = sessionStore,
        processBindingConfirmed = processBindingConfirmed,
        eventLog = eventLog
    )

    private val _session = MutableStateFlow<VerbSession?>(null)
    val session: StateFlow<VerbSession?> = _session.asStateFlow()

    /** The project the *tracked session* launched under -- never the UI's currently selected one. */
    private var sessionProjectDirectory: File? = null

    private var boundSessionId: String? = null
    private var boundRuntime: TerminalRuntimeAdapter? = null
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
     * Atomically claims the foreground on the specified session, writes the live session state,
     * dispatches the launch command to the session's concrete runtime, and starts watching for exit.
     *
     * If dispatch fails, rolls back the foreground claim and marks the session ENDED.
     */
    fun launch(
        project: VerbProject?,
        sessionId: String,
        command: String,
        runtime: TerminalRuntimeAdapter? = null
    ): Boolean {
        val targetRuntime = runtime ?: terminalRuntimeProvider(sessionId) ?: return false
        val providerRuntime = terminalRuntimeProvider(sessionId)
        if (runtime != null && providerRuntime != null && providerRuntime != runtime) {
            throw IllegalArgumentException("Supplied runtime does not match runtime for session $sessionId")
        }

        sessionProjectDirectory = project?.directory
        val idsBeforeLaunch = targetRuntime.commandHistory.value.mapTo(mutableSetOf()) { it.id }

        val claimed = VerbTerminalSessionHolder.claimForeground(sessionId, agentType, idsBeforeLaunch)
        if (!claimed) {
            return false
        }

        boundSessionId = sessionId
        boundRuntime = targetRuntime

        val agent = AgentRef(agentType = agentType, resumeIdentity = null)
        val now = Instant.now()
        val newSession = VerbSession(
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
        _session.value = newSession
        sessionStore.save(newSession)
        eventLog.append(newSession, "SESSION_STARTED")
        eventLog.append(newSession, "AGENT_STARTED")
        eventLog.append(newSession, "PROCESS_STARTED")

        try {
            targetRuntime.sendCommand(command)
        } catch (t: Throwable) {
            VerbTerminalSessionHolder.releaseForeground(sessionId, agentType)
            boundSessionId = null
            boundRuntime = null
            _session.value = newSession.copy(process = null, state = VerbSessionState.ENDED, lastSeenAt = Instant.now())
            _session.value?.let(sessionStore::save)
            eventLog.append(newSession, "PROCESS_ENDED", exitCode = -1)
            eventLog.append(newSession, "AGENT_ENDED")
            return false
        }

        watchForExit(idsBeforeLaunch, sessionId, targetRuntime)
        return true
    }

    /**
     * Call immediately after sending the command that launches the agent, with the command-history
     * snapshot captured immediately *before* sending it.
     */
    fun onLaunched(
        project: VerbProject?,
        idsBeforeLaunch: Set<String>,
        sessionId: String? = null,
        runtime: TerminalRuntimeAdapter? = null
    ) {
        val targetSessionId = sessionId ?: VerbTerminalSessionHolder.activeId.value ?: "default"
        val targetRuntime = runtime ?: (sessionId?.let(terminalRuntimeProvider)) ?: terminalRuntimeProvider(targetSessionId)
        sessionProjectDirectory = project?.directory
        val agent = AgentRef(agentType = agentType, resumeIdentity = null)
        val now = Instant.now()
        val newSession = VerbSession(
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
        _session.value = newSession
        sessionStore.save(newSession)
        eventLog.append(newSession, "SESSION_STARTED")
        eventLog.append(newSession, "AGENT_STARTED")
        eventLog.append(newSession, "PROCESS_STARTED")
        VerbTerminalSessionHolder.claimForeground(targetSessionId, agentType, idsBeforeLaunch)
        if (targetRuntime != null) {
            boundSessionId = targetSessionId
            boundRuntime = targetRuntime
            watchForExit(idsBeforeLaunch, targetSessionId, targetRuntime)
        }
    }

    /**
     * Resumes the tracked session via the agent's [AgentAdapter] / [VerbSessionResumer].
     */
    suspend fun resume(
        sessionId: String? = null,
        runtime: TerminalRuntimeAdapter? = null
    ) {
        val current = _session.value ?: return
        if (current.state != VerbSessionState.RECOVERABLE) return

        val targetSessionId = sessionId ?: boundSessionId ?: VerbTerminalSessionHolder.activeId.value ?: "default"
        val targetRuntime = runtime ?: (sessionId?.let(terminalRuntimeProvider)) ?: boundRuntime ?: terminalRuntimeProvider(targetSessionId)
            ?: return

        val idsBefore = targetRuntime.commandHistory.value.mapTo(mutableSetOf()) { it.id }
        val adapter = adapter(targetRuntime)
        val resumed = VerbSessionResumer.resume(current, adapter)
        _session.value = resumed
        sessionStore.save(resumed)
        if (resumed.state != current.state) {
            eventLog.append(resumed, "SESSION_STATE_CHANGED", state = resumed.state)
        }
        if (resumed.state == VerbSessionState.LIVE) {
            eventLog.append(resumed, "PROCESS_STARTED")
            eventLog.append(resumed, "AGENT_STARTED")
            boundSessionId = targetSessionId
            boundRuntime = targetRuntime
            VerbTerminalSessionHolder.claimForeground(targetSessionId, agentType, idsBefore)
            watchForExit(idsBefore, targetSessionId, targetRuntime)
        }
    }

    /**
     * Re-checks recovery for a session whose process is gone.
     */
    fun refresh() {
        val current = _session.value ?: return
        if (current.process != null) return
        coroutineScope.launch { resolveAfterExit(boundRuntime) }
    }

    /**
     * Watches for the agent exiting, bound strictly to [runtime] of [sessionId].
     */
    private fun watchForExit(
        idsBefore: Set<String>,
        sessionId: String,
        runtime: TerminalRuntimeAdapter
    ) {
        watchJob?.cancel()
        watchJob = coroutineScope.launch {
            var settled: com.example.verb.terminal.CommandExecutionRecord? = null
            while (settled == null) {
                settled = runtime.commandHistory.value.firstOrNull {
                    it.id !in idsBefore && it.state != CommandLifecycleState.RUNNING
                }
                if (settled == null) delay(EXIT_POLL_INTERVAL_MS)
            }

            VerbTerminalSessionHolder.releaseForeground(sessionId, agentType)
            boundSessionId = null
            boundRuntime = null
            _session.value = _session.value?.copy(process = null, lastSeenAt = Instant.now())
            _session.value?.let(sessionStore::save)
            _session.value?.let { session ->
                eventLog.append(session, "PROCESS_ENDED", exitCode = settled.exitCode)
                eventLog.append(session, "AGENT_ENDED")
            }
            resolveAfterExit(runtime)
        }
    }

    private suspend fun resolveAfterExit(runtime: TerminalRuntimeAdapter? = null) {
        val agent = _session.value?.agent ?: return
        repeat(RESOLVE_ATTEMPTS) { attempt ->
            val current = _session.value ?: return
            val adapter = adapter(runtime)
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
            eventLog.append(_session.value!!, "RECOVERY_CHECKED", state = resolvedState)
            if (resolvedState != current.state) {
                eventLog.append(_session.value!!, "SESSION_STATE_CHANGED", state = resolvedState)
            }
            if (resolvedState == VerbSessionState.ENDED && current.state != VerbSessionState.ENDED) {
                eventLog.append(_session.value!!, "SESSION_ENDED", state = resolvedState)
            }
            if (resolvedState != VerbSessionState.INTERRUPTED) return
            if (attempt < RESOLVE_ATTEMPTS - 1) delay(RESOLVE_RETRY_DELAY_MS)
        }
    }

    private fun adapter(runtime: TerminalRuntimeAdapter? = null): AgentAdapter =
        adapterFactory(sessionProjectDirectory, runtime)

    /**
     * Restores identity after process death, then asks the host whether the old binding is still
     * attached.
     */
    private fun restorePersistedSession() {
        val persisted = sessionStore.load() ?: return
        if (persisted.agent?.agentType?.let { it != agentType } == true) return
        sessionProjectDirectory = persisted.lastKnownCwd?.let(::File)

        // Exact restoration identity + ambiguity refusal:
        val bindingEntry = VerbTerminalSessionHolder.foregroundBindingForAgent(agentType)
        val sessionId = bindingEntry?.first
        val foregroundBinding = bindingEntry?.second
        val runtime = sessionId?.let { terminalRuntimeProvider(it) }

        val bindingStillAttached = processBindingConfirmed &&
            runtime != null &&
            runtime.isSessionActive.value &&
            runtime.sessionState.value == com.example.verb.terminal.TerminalSessionState.RUNNING &&
            foregroundBinding != null

        val restored = if (bindingStillAttached) {
            boundSessionId = sessionId
            boundRuntime = runtime
            persisted.copy(
                state = VerbSessionState.LIVE,
                process = LiveAgentBinding,
                lastSeenAt = Instant.now()
            )
        } else {
            val adapter = adapter(null)
            val agent = persisted.agent?.let { persistedAgent ->
                val identity = persistedAgent.resumeIdentity
                    ?: adapter.resumeIdentity(persistedAgent)
                persistedAgent.copy(resumeIdentity = identity)
            }
            val verdict = agent?.let { adapter.canResume(it) }
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
        if (!bindingStillAttached) {
            eventLog.append(restored, "RECOVERY_CHECKED", state = restored.state)
            if (restored.state != persisted.state) {
                eventLog.append(restored, "SESSION_STATE_CHANGED", state = restored.state)
            }
        }

        if (bindingStillAttached && restored.state == VerbSessionState.LIVE) {
            watchForExit(foregroundBinding!!.commandIdsBeforeLaunch, sessionId!!, runtime!!)
        }
    }

    private object LiveAgentBinding : ProcessBinding

    private companion object {
        const val EXIT_POLL_INTERVAL_MS = 500L
        const val RESOLVE_ATTEMPTS = 10
        const val RESOLVE_RETRY_DELAY_MS = 500L
    }
}
