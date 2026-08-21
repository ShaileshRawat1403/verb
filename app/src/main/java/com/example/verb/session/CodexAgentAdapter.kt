package com.example.verb.session

import com.example.verb.terminal.TerminalRuntimeAdapter
import java.io.File

/**
 * [AgentAdapter] for the Codex CLI. Everything here is Codex-specific runtime truth -- where Codex
 * records a conversation, what proves that conversation is worth resuming, what its stable identity
 * is, and how it is resumed. The session lifecycle around it is shared with Claude and lives in
 * [AgentSessionCoordinator]; nothing in this file is allowed to grow a second state machine.
 *
 * Codex records each conversation as a *rollout* file -- `~/.codex/sessions/<yyyy>/<mm>/<dd>/
 * rollout-<timestamp>-<session id>.jsonl` -- whose first line is a session-meta record carrying the
 * conversation's `id` and the `cwd` it ran in. Verb reads only those two fields plus a marker that a
 * real user turn exists; the conversation itself is never read into Verb's state and never
 * persisted.
 *
 * The distinction that matters, and the reason [hasUserTurn] exists: Codex writes a rollout file as
 * soon as it starts, so a file's existence proves only that Codex was *opened*, not that there is
 * anything to recover. Resuming an empty conversation would put Verb's UI in the position of
 * promising recovery and then delivering a blank session. Evidence of at least one user turn is
 * what makes [ResumeVerdict.YES] honest.
 */
class CodexAgentAdapter(
    private val filesDir: File,
    private val projectDirectory: File?,
    private val terminalRuntimeAdapter: TerminalRuntimeAdapter,
    private val resumeSettleMs: Long = DEFAULT_RESUME_SETTLE_MS,
    private val pollIntervalMs: Long = DEFAULT_POLL_INTERVAL_MS
) : AgentAdapter {

    /**
     * [ResumeVerdict.UNKNOWN] means the host gave Verb no readable evidence either way -- no
     * project to match against, or no readable `~/.codex/sessions` tree (Codex never ran, or its
     * files could not be read). It is never guessed as [ResumeVerdict.NO], which would claim an
     * impossibility Verb has not established.
     *
     * [ResumeVerdict.NO] is only returned when the sessions tree *was* readable and positively
     * contains no resumable conversation for this project: either no rollout for this cwd at all,
     * or rollouts that never got past being opened.
     */
    override fun canResume(agent: AgentRef): ResumeVerdict {
        val project = projectDirectory ?: return ResumeVerdict.UNKNOWN
        val rollouts = rolloutFiles() ?: return ResumeVerdict.UNKNOWN

        val matching = matchingRollouts(rollouts, project, agent)
        if (matching.isEmpty()) return ResumeVerdict.NO

        return if (matching.any { hasUserTurn(it.file) }) ResumeVerdict.YES else ResumeVerdict.NO
    }

    /**
     * Codex's own conversation id, taken from the newest rollout for this project that contains a
     * real user turn. Never the rollout filename or a PID: the filename carries a timestamp and the
     * process is gone by the time this matters.
     */
    override fun resumeIdentity(agent: AgentRef): String? {
        val project = projectDirectory ?: return null
        val rollouts = rolloutFiles() ?: return null
        return matchingRollouts(rollouts, project, agent)
            .firstOrNull { hasUserTurn(it.file) }
            ?.sessionId
    }

    /**
     * Sends `codex resume <id>` -- or `codex resume --last` when no id is known, never the bare
     * `codex resume`, which opens an interactive picker the user would have to answer by hand --
     * and waits up to [resumeSettleMs] to see whether it exits. [AgentResumeLauncher] owns the
     * reasoning about why "nothing settled" is the shape of success here.
     */
    override suspend fun resume(agent: AgentRef): ProcessBinding? {
        val resumeArgument = agent.resumeIdentity ?: "--last"
        val stillRunning = AgentResumeLauncher.launch(
            terminalRuntimeAdapter = terminalRuntimeAdapter,
            command = "codex resume $resumeArgument",
            settleMs = resumeSettleMs,
            pollIntervalMs = pollIntervalMs
        )
        return if (stillRunning) CodexProcessBinding else null
    }

    /** Null -- distinct from empty -- when Codex has no readable session tree on this host. */
    private fun rolloutFiles(): List<File>? {
        val sessionsRoot = GuestPathAliases.aliasesOf(filesDir)
            .map { File(it, "home/.codex/sessions") }
            .firstOrNull { it.isDirectory }
            ?: return null
        return runCatching {
            // Codex nests rollouts under year/month/day, so this walks rather than lists. The depth
            // bound keeps a surprise directory layout from turning a cheap check into a full scan.
            sessionsRoot.walkTopDown()
                .maxDepth(MAX_SESSION_TREE_DEPTH)
                .filter { it.isFile && it.extension == "jsonl" }
                .toList()
        }.getOrNull()
    }

    private data class Rollout(val file: File, val sessionId: String, val startedAt: String)

    /** Newest first, so callers can take the most recent conversation for this project. */
    private fun matchingRollouts(rollouts: List<File>, project: File, agent: AgentRef): List<Rollout> =
        rollouts.mapNotNull { file ->
            val header = runCatching { file.useLines { it.firstOrNull() } }.getOrNull()
                ?: return@mapNotNull null
            val cwd = JSON_CWD_PATTERN.find(header)?.groupValues?.get(1) ?: return@mapNotNull null
            val sessionId = JSON_ID_PATTERN.find(header)?.groupValues?.get(1) ?: return@mapNotNull null
            if (!GuestPathAliases.sameDirectory(cwd, project)) return@mapNotNull null
            if (agent.resumeIdentity != null && agent.resumeIdentity != sessionId) return@mapNotNull null
            Rollout(
                file = file,
                sessionId = sessionId,
                startedAt = JSON_TIMESTAMP_PATTERN.find(header)?.groupValues?.get(1).orEmpty()
            )
        }.sortedWith(
            // ISO-8601 timestamps sort lexicographically; the file's own mtime breaks ties and
            // covers a rollout whose header carries no timestamp at all.
            compareByDescending<Rollout> { it.startedAt }.thenByDescending { it.file.lastModified() }
        )

    /**
     * Whether [rollout] contains at least one real user turn, read as a marker only -- no line is
     * ever returned, stored, or logged. The line bound keeps this from reading a long conversation
     * end to end just to answer a yes/no question; a user turn appears within the first few records
     * of any conversation that has one at all.
     */
    private fun hasUserTurn(rollout: File): Boolean = runCatching {
        rollout.useLines { lines ->
            lines.take(MAX_EVIDENCE_LINES).any(::recordsUserTurn)
        }
    }.getOrDefault(false)

    /**
     * The subtlety that decides whether Verb's "Session recoverable" is honest: not every
     * user-role record in a rollout is something the user typed. Codex injects its own
     * `<environment_context>` (and, in other versions, `<user_instructions>`) as a user-role
     * message, so matching `"role":"user"` alone would count a session the user only ever opened.
     * Read from a real rollout written by codex-cli 0.149.0 on the validation device, not assumed.
     */
    private fun recordsUserTurn(line: String): Boolean {
        if (EVENT_USER_MESSAGE_PATTERN.containsMatchIn(line)) return true
        if (!USER_ROLE_PATTERN.containsMatchIn(line)) return false
        return INPUT_TEXT_PATTERN.findAll(line).any { match ->
            SYNTHETIC_TURN_PREFIXES.none { prefix -> match.groupValues[1].startsWith(prefix) }
        }
    }

    private object CodexProcessBinding : ProcessBinding

    private companion object {
        val JSON_CWD_PATTERN = Regex("\"cwd\"\\s*:\\s*\"([^\"]*)\"")
        val JSON_ID_PATTERN = Regex("\"id\"\\s*:\\s*\"([^\"]*)\"")
        val JSON_TIMESTAMP_PATTERN = Regex("\"timestamp\"\\s*:\\s*\"([^\"]*)\"")

        /** The response-item form of a turn: a user-role message carrying `input_text` content. */
        val USER_ROLE_PATTERN = Regex("\"role\"\\s*:\\s*\"user\"")
        val INPUT_TEXT_PATTERN = Regex("\"text\"\\s*:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"")

        /** The event form other Codex versions write instead; matching both avoids pinning to one. */
        val EVENT_USER_MESSAGE_PATTERN = Regex("\"type\"\\s*:\\s*\"user_message\"")

        /** Codex's own injected user-role messages, which prove nothing about a real conversation. */
        val SYNTHETIC_TURN_PREFIXES = listOf("<environment_context>", "<user_instructions>")

        const val MAX_SESSION_TREE_DEPTH = 5
        const val MAX_EVIDENCE_LINES = 200
        const val DEFAULT_RESUME_SETTLE_MS = 5_000L
        const val DEFAULT_POLL_INTERVAL_MS = 200L
    }
}

/**
 * Binds Codex to the one shared session state machine, exactly as `ClaudeSessionCoordinator` does
 * for Claude. No second lifecycle, no second state machine -- only a different adapter.
 */
@Suppress("FunctionName")
fun CodexSessionCoordinator(
    filesDir: File,
    terminalRuntimeAdapter: TerminalRuntimeAdapter,
    coroutineScope: kotlinx.coroutines.CoroutineScope,
    sessionStore: VerbSessionStore = InMemoryVerbSessionStore(),
    processBindingConfirmed: Boolean = false
): AgentSessionCoordinator = AgentSessionCoordinator(
    agentType = CODEX_AGENT_TYPE,
    adapterFactory = { project -> CodexAgentAdapter(filesDir, project, terminalRuntimeAdapter) },
    terminalRuntimeAdapter = terminalRuntimeAdapter,
    coroutineScope = coroutineScope,
    sessionStore = sessionStore,
    processBindingConfirmed = processBindingConfirmed
)

/** The [AgentRef.agentType] and [VerbSession.runtime] value for Codex sessions. */
const val CODEX_AGENT_TYPE = "codex"
