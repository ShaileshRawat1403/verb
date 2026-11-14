package com.example.verb.session

import com.example.verb.project.VerbProject
import com.example.verb.terminal.CommandLifecycleState
import com.example.verb.terminal.TerminalRuntimeAdapter
import com.example.verb.terminal.TerminalSessionState
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
 * One lifecycle implementation per agent type, with independently durable product sessions.
 * Every live session keeps its own terminal binding and watcher. The single [session] flow is a
 * compatibility view for the agent card; [sessions] contains every session, including older ones.
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
    constructor(
        agentType: String,
        adapterFactory: (projectDirectory: File?, runtime: TerminalRuntimeAdapter?) -> AgentAdapter,
        terminalRuntimeAdapter: TerminalRuntimeAdapter,
        coroutineScope: CoroutineScope,
        sessionStore: VerbSessionStore = InMemoryVerbSessionStore(),
        processBindingConfirmed: Boolean = false,
        eventLog: VerbEventLog = VerbEventLog.Disabled
    ) : this(
        agentType, adapterFactory, { terminalRuntimeAdapter }, coroutineScope, sessionStore,
        processBindingConfirmed, eventLog
    )

    private data class Binding(
        val terminalId: String,
        val runtime: TerminalRuntimeAdapter,
        val watch: Job
    )

    private val records = linkedMapOf<String, VerbSession>()
    private val bindings = mutableMapOf<String, Binding>()
    private val _sessions = MutableStateFlow<List<VerbSession>>(emptyList())
    val sessions: StateFlow<List<VerbSession>> = _sessions.asStateFlow()
    private val _session = MutableStateFlow<VerbSession?>(null)
    val session: StateFlow<VerbSession?> = _session.asStateFlow()

    init {
        restorePersistedSessions()
    }

    fun cancelWatch() {
        bindings.values.forEach { it.watch.cancel() }
        bindings.clear()
    }

    private fun publish(record: VerbSession) {
        sessionStore.save(record)
        records[record.id] = record
        val ordered = records.values.toList()
        _sessions.value = ordered
        _session.value = ordered.lastOrNull()
    }

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
        val idsBefore = targetRuntime.commandHistory.value.mapTo(mutableSetOf()) { it.id }
        val now = Instant.now()
        val record = VerbSession(
            id = UUID.randomUUID().toString(),
            projectId = project?.id,
            runtime = agentType,
            createdAt = now,
            lastSeenAt = now,
            state = VerbSessionState.LIVE,
            lastKnownCwd = project?.directory?.absolutePath,
            lastObservedAt = project?.directory?.let { now },
            agent = AgentRef(agentType, null),
            process = LiveAgentBinding
        )
        if (!VerbTerminalSessionHolder.claimForeground(sessionId, agentType, idsBefore, record.id)) {
            return false
        }
        try {
            publish(record)
        } catch (_: RuntimeException) {
            VerbTerminalSessionHolder.releaseForeground(sessionId, agentType)
            return false
        }
        eventLog.append(record, "SESSION_STARTED")
        eventLog.append(record, "AGENT_STARTED")
        eventLog.append(record, "PROCESS_STARTED")
        try {
            targetRuntime.sendCommand(command)
        } catch (_: Throwable) {
            VerbTerminalSessionHolder.releaseForeground(sessionId, agentType)
            val ended = record.copy(process = null, state = VerbSessionState.ENDED, lastSeenAt = Instant.now())
            publish(ended)
            eventLog.append(ended, "PROCESS_ENDED", exitCode = -1)
            eventLog.append(ended, "AGENT_ENDED")
            return false
        }
        watchForExit(record.id, idsBefore, sessionId, targetRuntime)
        return true
    }

    /** Legacy hook for callers that already dispatched a command. */
    fun onLaunched(
        project: VerbProject?,
        idsBeforeLaunch: Set<String>,
        sessionId: String? = null,
        runtime: TerminalRuntimeAdapter? = null
    ) {
        val terminalId = sessionId ?: VerbTerminalSessionHolder.activeId.value ?: "default"
        val targetRuntime = runtime ?: terminalRuntimeProvider(terminalId)
        val now = Instant.now()
        val record = VerbSession(
            id = UUID.randomUUID().toString(),
            projectId = project?.id,
            runtime = agentType,
            createdAt = now,
            lastSeenAt = now,
            state = VerbSessionState.LIVE,
            lastKnownCwd = project?.directory?.absolutePath,
            lastObservedAt = project?.directory?.let { now },
            agent = AgentRef(agentType, null),
            process = LiveAgentBinding
        )
        check(VerbTerminalSessionHolder.claimForeground(terminalId, agentType, idsBeforeLaunch, record.id)) {
            "Terminal is already occupied"
        }
        try {
            publish(record)
        } catch (error: Throwable) {
            VerbTerminalSessionHolder.releaseForeground(terminalId, agentType)
            throw error
        }
        eventLog.append(record, "SESSION_STARTED")
        eventLog.append(record, "AGENT_STARTED")
        eventLog.append(record, "PROCESS_STARTED")
        if (targetRuntime != null) watchForExit(record.id, idsBeforeLaunch, terminalId, targetRuntime)
    }

    /** Resume a chosen product session. The optional ID defaults to the latest agent card record. */
    suspend fun resume(
        sessionId: String? = null,
        runtime: TerminalRuntimeAdapter? = null,
        productSessionId: String? = null
    ) {
        val current = productSessionId?.let(records::get) ?: _session.value ?: return
        if (current.state != VerbSessionState.RECOVERABLE) return
        val terminalId = sessionId ?: VerbTerminalSessionHolder.activeId.value ?: "default"
        val targetRuntime = runtime ?: terminalRuntimeProvider(terminalId) ?: return
        val idsBefore = targetRuntime.commandHistory.value.mapTo(mutableSetOf()) { it.id }
        if (!VerbTerminalSessionHolder.claimForeground(terminalId, agentType, idsBefore, current.id)) return
        val resumed = try {
            VerbSessionResumer.resume(current, adapter(current, targetRuntime))
        } catch (error: Throwable) {
            VerbTerminalSessionHolder.releaseForeground(terminalId, agentType)
            throw error
        }
        if (resumed.state != VerbSessionState.LIVE) {
            VerbTerminalSessionHolder.releaseForeground(terminalId, agentType)
            return
        }
        publish(resumed)
        eventLog.append(resumed, "SESSION_STATE_CHANGED", state = resumed.state)
        eventLog.append(resumed, "PROCESS_STARTED")
        eventLog.append(resumed, "AGENT_STARTED")
        watchForExit(current.id, idsBefore, terminalId, targetRuntime)
    }

    fun refresh() {
        records.values.filter { it.process == null }.forEach { record ->
            coroutineScope.launch { resolveAfterExit(record.id) }
        }
    }

    private fun watchForExit(
        productSessionId: String,
        idsBefore: Set<String>,
        terminalId: String,
        runtime: TerminalRuntimeAdapter
    ) {
        bindings.remove(productSessionId)?.watch?.cancel()
        val job = coroutineScope.launch {
            var settled: com.example.verb.terminal.CommandExecutionRecord? = null
            var observedActive = runtime.isSessionActive.value
            while (true) {
                settled = runtime.commandHistory.value.firstOrNull {
                    it.id !in idsBefore && it.state != CommandLifecycleState.RUNNING
                }
                if (settled != null) break
                if (runtime.isSessionActive.value) observedActive = true
                if (runtime.sessionState.value == TerminalSessionState.EXITED ||
                    runtime.sessionState.value == TerminalSessionState.FAILED ||
                    (observedActive && !runtime.isSessionActive.value)
                ) break
                delay(EXIT_POLL_INTERVAL_MS)
            }
            VerbTerminalSessionHolder.releaseForeground(terminalId, agentType)
            bindings.remove(productSessionId)
            val current = records[productSessionId] ?: return@launch
            val exited = current.copy(process = null, lastSeenAt = Instant.now())
            publish(exited)
            eventLog.append(exited, "PROCESS_ENDED", exitCode = settled?.exitCode ?: -1)
            eventLog.append(exited, "AGENT_ENDED")
            resolveAfterExit(productSessionId, runtime)
        }
        bindings[productSessionId] = Binding(terminalId, runtime, job)
    }

    private suspend fun resolveAfterExit(productSessionId: String, runtime: TerminalRuntimeAdapter? = null) {
        repeat(RESOLVE_ATTEMPTS) { attempt ->
            val current = records[productSessionId] ?: return
            if (current.process != null) return
            val originalAgent = current.agent ?: return
            val adapter = adapter(current, runtime)
            val identity = originalAgent.resumeIdentity ?: adapter.resumeIdentity(originalAgent)
            val agent = originalAgent.copy(resumeIdentity = identity)
            val observed = adapter.canResume(agent)
            val verdict = if (identity == null && observed == ResumeVerdict.YES) {
                ResumeVerdict.UNKNOWN
            } else observed
            val state = VerbSessionStateResolver.resolve(false, agent, verdict)
            val resolved = current.copy(lastSeenAt = Instant.now(), state = state, agent = agent)
            publish(resolved)
            eventLog.append(resolved, "RECOVERY_CHECKED", state = state)
            if (state != current.state) eventLog.append(resolved, "SESSION_STATE_CHANGED", state = state)
            if (state == VerbSessionState.ENDED && current.state != VerbSessionState.ENDED) {
                eventLog.append(resolved, "SESSION_ENDED", state = state)
            }
            if (state != VerbSessionState.INTERRUPTED) return
            if (attempt < RESOLVE_ATTEMPTS - 1) delay(RESOLVE_RETRY_DELAY_MS)
        }
    }

    private fun adapter(record: VerbSession, runtime: TerminalRuntimeAdapter?): AgentAdapter =
        adapterFactory(record.lastKnownCwd?.let(::File), runtime)

    private fun restorePersistedSessions() {
        val persisted = sessionStore.loadAll().filter { it.agent?.agentType == agentType }
        persisted.forEach { previous ->
            val entry = VerbTerminalSessionHolder.foregroundBindingForProductSession(previous.id)
                ?: if (persisted.size == 1) VerbTerminalSessionHolder.foregroundBindingForAgent(agentType) else null
            val terminalId = entry?.first
            val binding = entry?.second
            val runtime = terminalId?.let(terminalRuntimeProvider)
            val attached = processBindingConfirmed && runtime != null &&
                runtime.isSessionActive.value && runtime.sessionState.value == TerminalSessionState.RUNNING &&
                binding != null
            val restored = if (attached) {
                previous.copy(state = VerbSessionState.LIVE, process = LiveAgentBinding, lastSeenAt = Instant.now())
            } else {
                val adapter = adapter(previous, null)
                val agent = previous.agent?.let {
                    it.copy(resumeIdentity = it.resumeIdentity ?: adapter.resumeIdentity(it))
                }
                val observed = agent?.let(adapter::canResume) ?: ResumeVerdict.UNKNOWN
                val verdict = if (agent?.resumeIdentity == null && observed == ResumeVerdict.YES) {
                    ResumeVerdict.UNKNOWN
                } else observed
                previous.copy(
                    state = VerbSessionStateResolver.resolve(false, agent, verdict),
                    process = null, agent = agent, lastSeenAt = Instant.now()
                )
            }
            publish(restored)
            if (attached) {
                watchForExit(previous.id, binding!!.commandIdsBeforeLaunch, terminalId!!, runtime!!)
            } else {
                eventLog.append(restored, "RECOVERY_CHECKED", state = restored.state)
                if (restored.state != previous.state) {
                    eventLog.append(restored, "SESSION_STATE_CHANGED", state = restored.state)
                }
            }
        }
    }

    private object LiveAgentBinding : ProcessBinding

    private companion object {
        const val EXIT_POLL_INTERVAL_MS = 500L
        const val RESOLVE_ATTEMPTS = 10
        const val RESOLVE_RETRY_DELAY_MS = 500L
    }
}
