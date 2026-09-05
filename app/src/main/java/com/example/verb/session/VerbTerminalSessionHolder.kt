package com.example.verb.session

import com.example.verb.terminal.TerminalRuntime
import com.example.verb.terminal.VerbTerminal
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Owns this process's terminal sessions, so their lifetime stops being implicitly the ViewModel's.
 *
 * Without this, a `VerbViewModel` created because the Activity was recreated for real (not merely a
 * config change -- rows 1/3/4 in `docs/DURABLE_SESSION.md` already survive those) would construct a
 * brand new `TerminalRuntime` and spawn a duplicate proot process, while the old one leaked in the
 * background with nothing left pointing to it. A new `VerbViewModel` reattaches to what is already
 * here instead.
 *
 * **A project has sessions.** There used to be exactly one, which meant running an agent and
 * running your own commands were the same slot and you had to choose. The shells themselves are
 * cheap -- one rootfs, a bash each -- so the honest limit is what you run *inside* them, not how
 * many you open. That limit is [MAX_SESSIONS], and it is stated rather than discovered by running
 * out of memory.
 *
 * Deliberately process-scoped, not persisted: none of this survives process death (force-stop,
 * background kill) any more than the ViewModel-owned instance it replaced did -- the whole process,
 * this object included, disappears together. `docs/DURABLE_SESSION.md` argues force-stop should stay
 * a hard boundary rather than something engineered around.
 */
object VerbTerminalSessionHolder {

    /**
     * Two shells is the case this was built for: an agent in one, your own commands in the other.
     * Beyond a handful, the constraint stops being Verb's and starts being the phone's -- each
     * agent inside a session is a full Node runtime, and Codex runs under qemu on some devices.
     */
    const val MAX_SESSIONS: Int = 4

    /**
     * Which agent is running in a terminal, when one is.
     *
     * The binding proves a PTY survived; it never proved *what* was inside it, and that gap is why
     * two agents could both report "Running" while neither was: each coordinator saw the same live
     * terminal and claimed it. The marker has exactly the lifetime of the thing it describes -- it
     * lives beside the runtime, survives an Activity being recreated with it, and dies with the
     * process, which is precisely when Verb also stops being able to prove anything.
     *
     * It belongs to a *session*, not to the process. With one terminal those were the same thing;
     * with several, an agent in session two must not make session one look occupied.
     */
    /**
     * The [ForegroundBinding.agentType] for Antigravity.
     *
     * Antigravity has no [AgentSessionCoordinator], because Verb has no evidence-based way to
     * recover one of its conversations. It still *occupies* a terminal, and those are different
     * claims: occupancy says "something owns this PTY, do not draw over it and do not offer to start
     * something else here", while recovery says "this conversation can be picked back up". Only the
     * second needs an adapter. Naming it after the launch command keeps the workspace line reading
     * the way the others do -- `claude running`, `codex running`, `agy running`.
     */
    const val ANTIGRAVITY_AGENT_TYPE: String = "agy"

    data class ForegroundBinding(
        val agentType: String,
        val commandIdsBeforeLaunch: Set<String>
    )

    /** One terminal: its runtime, and whatever agent currently occupies it. */
    private class Session(val runtime: TerminalRuntime) {
        @Volatile var foreground: ForegroundBinding? = null
    }

    private val sessions = LinkedHashMap<String, Session>()
    private val nextOrdinal = AtomicLong(1)

    private val _sessionIds = MutableStateFlow<List<String>>(emptyList())

    /** Every open terminal, in the order it was opened. */
    val sessionIds: StateFlow<List<String>> = _sessionIds.asStateFlow()

    private val _activeId = MutableStateFlow<String?>(null)

    /** The terminal the person is looking at, and the one keystrokes reach. */
    val activeId: StateFlow<String?> = _activeId.asStateFlow()

    private val _activeRuntime = MutableStateFlow<VerbTerminal?>(null)

    /** The active terminal's runtime, for `SwitchingTerminalRuntime` to follow. */
    val activeRuntime: StateFlow<VerbTerminal?> = _activeRuntime.asStateFlow()

    private val _runtimes = MutableStateFlow<List<VerbTerminal>>(emptyList())

    /**
     * Every open terminal's runtime, so a caller can ask about *all* of them rather than the one in
     * front. The foreground hold needs exactly this: an agent running in a terminal you are not
     * looking at is still an agent running.
     */
    val runtimes: StateFlow<List<VerbTerminal>> = _runtimes.asStateFlow()

    /** True when this Android process already owned a terminal before the caller asked. */
    fun hasAnySession(): Boolean = synchronized(this) { sessions.isNotEmpty() }

    /** The runtime behind [id], or null once it has been closed. */
    fun runtimeOf(id: String): TerminalRuntime? = synchronized(this) { sessions[id]?.runtime }

    /**
     * Ensures at least one terminal exists and returns the active one.
     *
     * The reattachment path: a rebuilt ViewModel calls this and gets whatever this process already
     * had, rather than spawning a second proot for the same screen.
     */
    fun getOrCreateActive(factory: () -> TerminalRuntime): TerminalRuntime = synchronized(this) {
        activeSession()?.runtime ?: openLocked(factory).runtime
    }

    /**
     * Opens another terminal and makes it active, or returns null when [MAX_SESSIONS] is reached.
     *
     * Null rather than an exception: running out of sessions is a thing the interface should say
     * plainly, not a crash.
     */
    fun open(factory: () -> TerminalRuntime): String? = synchronized(this) {
        if (sessions.size >= MAX_SESSIONS) return null
        openLocked(factory).let { _activeId.value }
    }

    private fun openLocked(factory: () -> TerminalRuntime): Session {
        val id = "terminal-${nextOrdinal.getAndIncrement()}"
        val session = Session(factory())
        // The diagnostics log is one sink shared by every terminal, and until this line nothing in
        // it said which terminal a measurement came from.
        session.runtime.setDiagnosticsLabel(id)
        sessions[id] = session
        publishSessions()
        activate(id)
        return session
    }

    /** Brings [id] to the front. Unknown ids are ignored rather than clearing the active one. */
    fun activate(id: String) = synchronized(this) {
        val session = sessions[id] ?: return
        _activeId.value = id
        _activeRuntime.value = session.runtime
    }

    /**
     * Closes [id], destroying its PTY, and hands the front to whatever remains.
     *
     * The last terminal cannot be closed: a workspace with no terminal in it is a screen with
     * nothing to do, and "close" would read as "quit" without saying so.
     */
    fun close(id: String): Boolean = synchronized(this) {
        if (sessions.size <= 1 || id !in sessions) return false
        val session = sessions.remove(id) ?: return false
        session.runtime.destroy()
        publishSessions()
        if (_activeId.value == id) {
            sessions.keys.lastOrNull()?.let(::activate)
        }
        return true
    }

    private fun publishSessions() {
        _sessionIds.value = sessions.keys.toList()
        _runtimes.value = sessions.values.map { it.runtime }
    }

    private fun activeSession(): Session? = _activeId.value?.let(sessions::get)

    /**
     * Records that [agentType] now occupies [sessionId], and says whether it landed.
     */
    fun claimForeground(sessionId: String, agentType: String, commandIdsBeforeLaunch: Set<String>): Boolean =
        synchronized(this) {
            val session = sessions[sessionId]
            if (session != null) {
                session.foreground = ForegroundBinding(agentType, commandIdsBeforeLaunch.toSet())
                true
            } else if (sessions.isEmpty()) {
                true
            } else {
                false
            }
        }

    /**
     * Records that [agentType] now occupies the terminal in front, and says whether it landed.
     *
     * Returns false when there is no session to occupy. A claim is a statement about a *specific*
     * terminal -- that is the whole reason it exists, since a claim about "the process" is what let
     * two agents both report Running from one PTY. With no terminal there is nothing to claim, and
     * a caller that assumed otherwise should find out rather than carry on believing it worked.
     */
    fun claimForeground(agentType: String, commandIdsBeforeLaunch: Set<String>): Boolean =
        synchronized(this) {
            val session = activeSession() ?: return false
            session.foreground = ForegroundBinding(agentType, commandIdsBeforeLaunch.toSet())
            true
        }

    /** Records that [agentType] has left [sessionId]. */
    fun releaseForeground(sessionId: String, agentType: String) = synchronized(this) {
        val session = sessions[sessionId]
        if (session?.foreground?.agentType == agentType) {
            session.foreground = null
        }
    }

    /** Records that [agentType] has left whichever terminal it was holding. */
    fun releaseForeground(agentType: String) = synchronized(this) {
        sessions.values.forEach { session ->
            if (session.foreground?.agentType == agentType) session.foreground = null
        }
    }

    /**
     * The agent occupying [id], or null when that terminal is at a shell prompt.
     *
     * What makes a list of terminals worth looking at: "Terminal 2 — Claude Code" tells you where
     * the agent is and, by omission, which one is yours to type in.
     */
    fun foregroundAgentOf(id: String): String? = synchronized(this) {
        sessions[id]?.foreground?.agentType
    }

    /** The agent occupying any terminal, or null when every one is back at a shell prompt. */
    fun foregroundAgent(): String? = synchronized(this) {
        sessions.values.firstNotNullOfOrNull { it.foreground }?.agentType
    }

    /** The [ForegroundBinding] occupying [sessionId], or null when that terminal is at a shell prompt. */
    fun foregroundBindingOf(sessionId: String): ForegroundBinding? = synchronized(this) {
        sessions[sessionId]?.foreground
    }

    /**
     * Finds the unique session holding a foreground binding for [agentType].
     *
     * Returns `(sessionId, ForegroundBinding)` if exactly one session holds a binding for [agentType].
     * Returns `null` if 0 sessions hold a binding.
     * Returns `null` (refuses ambiguous guess) if >1 sessions claim to hold a binding for the same agent type.
     */
    fun foregroundBindingForAgent(agentType: String): Pair<String, ForegroundBinding>? = synchronized(this) {
        val matching = sessions.entries.mapNotNull { (id, s) ->
            val fg = s.foreground
            if (fg != null && fg.agentType == agentType) id to fg else null
        }
        when (matching.size) {
            1 -> matching.first()
            else -> null // 0 or >1 (ambiguity refusal)
        }
    }

    /** The unique session ID holding a foreground binding for [agentType], or null if 0 or >1 (ambiguous). */
    fun sessionIdForAgent(agentType: String): String? = foregroundBindingForAgent(agentType)?.first

    /**
     * Runtime-only evidence needed to reattach an agent-exit watch after Activity/ViewModel
     * recreation. The command baseline is deliberately kept beside the PTY, never persisted:
     * command-history IDs and process presence have meaning only inside this Android process.
     */
    fun foregroundBinding(): ForegroundBinding? = synchronized(this) {
        sessions.values.firstNotNullOfOrNull { it.foreground }
    }

    /**
     * Test-only. Without this, a JVM test run's sessions leak into the next test's, since this
     * object is a singleton for the life of the JVM, not just the (simulated) app process.
     */
    fun resetForTests() = synchronized(this) {
        sessions.clear()
        _sessionIds.value = emptyList()
        _runtimes.value = emptyList()
        _activeId.value = null
        _activeRuntime.value = null
        nextOrdinal.set(1)
    }
}
