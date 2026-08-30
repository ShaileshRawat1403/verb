package com.example.verb.session

import android.database.sqlite.SQLiteDatabase
import com.example.verb.terminal.TerminalRuntimeAdapter
import java.io.File

/**
 * [AgentAdapter] for OpenCode, whose recovery evidence looks nothing like Claude's or Codex's:
 * there are no transcript files at all. OpenCode keeps its sessions in a **SQLite database** at
 * `~/.local/share/opencode/opencode.db` (schema read off the installed build on the validation
 * device: `session(id, project_id, directory, parent_id, time_updated, …)` and
 * `message(id, session_id, data, …)`, where `data` is the JSON message payload).
 *
 * That difference is exactly why adapters exist. The lifecycle around this is the same shared
 * [AgentSessionCoordinator] Claude and Codex use; only the evidence lookup below is OpenCode's.
 *
 * The database is copied before it is read, never opened in place. OpenCode may be running and
 * writing to it, the file is in WAL mode, and Verb has no business taking locks on another
 * program's live database to answer a read-only question. The copy is opened read/write only so
 * SQLite can replay the WAL into it, then deleted.
 */
class OpenCodeAgentAdapter(
    private val filesDir: File,
    private val projectDirectory: File?,
    private val terminalRuntimeAdapter: TerminalRuntimeAdapter? = null,
    private val scratchDir: File,
    private val resumeSettleMs: Long = DEFAULT_RESUME_SETTLE_MS,
    private val pollIntervalMs: Long = DEFAULT_POLL_INTERVAL_MS
) : AgentAdapter {

    /**
     * [ResumeVerdict.UNKNOWN] means no readable evidence either way: no project to match, no
     * database (OpenCode never ran), or a database Verb could not read. [ResumeVerdict.NO] is
     * returned only when the database *was* read and positively holds no resumable conversation for
     * this project -- which includes a session row with no messages, since OpenCode records a
     * session as soon as it starts and that proves only that it was opened.
     */
    override fun canResume(agent: AgentRef): ResumeVerdict {
        val project = projectDirectory ?: return ResumeVerdict.UNKNOWN
        val sessions = usedSessionIds(project, agent) ?: return ResumeVerdict.UNKNOWN
        return if (sessions.isEmpty()) ResumeVerdict.NO else ResumeVerdict.YES
    }

    /** OpenCode's own session id -- the newest conversation for this project that was used. */
    override fun resumeIdentity(agent: AgentRef): String? {
        val project = projectDirectory ?: return null
        return usedSessionIds(project, agent)?.firstOrNull()
    }

    /**
     * Sends `opencode --session <id>` -- or `-c` (continue the last session) when no id is known --
     * and waits up to [resumeSettleMs] to see whether it exits. Flags read from `opencode --help`
     * on the installed build, not assumed. [AgentResumeLauncher] owns the reasoning about why
     * "nothing settled" is the shape of success here.
     */
    override suspend fun resume(agent: AgentRef): ProcessBinding? {
        val runtime = terminalRuntimeAdapter ?: return null
        val resumeArgument = ResumeIdentity.validOrNull(agent.resumeIdentity)
            ?.let { "--session $it" }
            ?: "--continue"
        val stillRunning = AgentResumeLauncher.launch(
            terminalRuntimeAdapter = runtime,
            command = "opencode $resumeArgument",
            settleMs = resumeSettleMs,
            pollIntervalMs = pollIntervalMs
        )
        return if (stillRunning) OpenCodeProcessBinding else null
    }

    /**
     * Session ids for [project] that hold at least one user message, newest first. `null` -- as
     * distinct from an empty list -- means the database could not be read, which is the absence of
     * an answer rather than a "no".
     */
    private fun usedSessionIds(project: File, agent: AgentRef): List<String>? {
        val database = databaseFile() ?: return null
        val copy = copyForReading(database) ?: return null
        return try {
            readUsedSessionIds(copy, project, agent)
        } finally {
            copy.parentFile?.deleteRecursively()
        }
    }

    private fun databaseFile(): File? = GuestPathAliases.aliasesOf(filesDir)
        .map { File(it, DATABASE_RELATIVE_PATH) }
        .firstOrNull { it.isFile }

    /**
     * Copies the database and its write-ahead log into a private scratch directory. The WAL matters
     * more than the database file here: OpenCode killed with Verb's process leaves the newest
     * messages -- the very ones that decide recoverability -- in the WAL, and a copy without it
     * would report an out-of-date "no".
     */
    private fun copyForReading(database: File): File? = runCatching {
        val walFile = File(database.path + "-wal")
        if (database.length() + walFile.length() > MAX_DATABASE_BYTES) return null

        val directory = File(scratchDir, "opencode-session-read-${System.nanoTime()}")
        if (!directory.mkdirs()) return null
        val copy = File(directory, database.name)
        database.copyTo(copy, overwrite = true)
        if (walFile.isFile) walFile.copyTo(File(directory, walFile.name), overwrite = true)
        copy
    }.getOrNull()

    private fun readUsedSessionIds(copy: File, project: File, agent: AgentRef): List<String>? =
        runCatching {
            // Read/write, deliberately: replaying the copied WAL is a write, and without it the
            // newest messages stay invisible. The copy is thrown away immediately afterwards.
            SQLiteDatabase.openDatabase(
                copy.path,
                null,
                SQLiteDatabase.OPEN_READWRITE
            ).use { database ->
                val identityFilter = agent.resumeIdentity?.let { " AND s.id = ?" }.orEmpty()
                database.rawQuery(
                    """
                    SELECT s.id, s.directory FROM session s
                    WHERE s.parent_id IS NULL
                      AND EXISTS (
                        SELECT 1 FROM message m
                        WHERE m.session_id = s.id AND m.data LIKE '%"role":"user"%'
                      )$identityFilter
                    ORDER BY s.time_updated DESC
                    """.trimIndent(),
                    agent.resumeIdentity?.let { arrayOf(it) }
                ).use { cursor ->
                    buildList {
                        while (cursor.moveToNext()) {
                            // Directory matching happens here rather than in SQL so it goes through
                            // the same Android path-alias rule every other adapter uses; SQL could
                            // only compare the literal strings.
                            if (GuestPathAliases.sameDirectory(cursor.getString(1), project)) {
                                ResumeIdentity.validOrNull(cursor.getString(0))?.let(::add)
                            }
                        }
                    }
                }
            }
        }.getOrNull()

    private object OpenCodeProcessBinding : ProcessBinding

    private companion object {
        const val DATABASE_RELATIVE_PATH = "home/.local/share/opencode/opencode.db"

        /** A guard, not a policy: a database this large means something unexpected, so report UNKNOWN. */
        const val MAX_DATABASE_BYTES = 128L * 1024 * 1024
        const val DEFAULT_RESUME_SETTLE_MS = 5_000L
        const val DEFAULT_POLL_INTERVAL_MS = 200L
    }
}

/**
 * Binds OpenCode to the one shared session state machine, exactly as `ClaudeSessionCoordinator` and
 * `CodexSessionCoordinator` do. Third agent, same lifecycle, no new state machine.
 */
@Suppress("FunctionName")
fun OpenCodeSessionCoordinator(
    filesDir: File,
    scratchDir: File,
    terminalRuntimeProvider: (sessionId: String) -> TerminalRuntimeAdapter?,
    coroutineScope: kotlinx.coroutines.CoroutineScope,
    sessionStore: VerbSessionStore = InMemoryVerbSessionStore(),
    processBindingConfirmed: Boolean = false
): AgentSessionCoordinator = AgentSessionCoordinator(
    agentType = OPENCODE_AGENT_TYPE,
    adapterFactory = { project, runtime ->
        OpenCodeAgentAdapter(filesDir, project, runtime, scratchDir)
    },
    terminalRuntimeProvider = terminalRuntimeProvider,
    coroutineScope = coroutineScope,
    sessionStore = sessionStore,
    processBindingConfirmed = processBindingConfirmed,
    eventLog = VerbEventLog(filesDir)
)

@Suppress("FunctionName")
fun OpenCodeSessionCoordinator(
    filesDir: File,
    scratchDir: File,
    terminalRuntimeAdapter: TerminalRuntimeAdapter,
    coroutineScope: kotlinx.coroutines.CoroutineScope,
    sessionStore: VerbSessionStore = InMemoryVerbSessionStore(),
    processBindingConfirmed: Boolean = false
): AgentSessionCoordinator = AgentSessionCoordinator(
    agentType = OPENCODE_AGENT_TYPE,
    adapterFactory = { project, runtime ->
        OpenCodeAgentAdapter(filesDir, project, runtime ?: terminalRuntimeAdapter, scratchDir)
    },
    terminalRuntimeAdapter = terminalRuntimeAdapter,
    coroutineScope = coroutineScope,
    sessionStore = sessionStore,
    processBindingConfirmed = processBindingConfirmed,
    eventLog = VerbEventLog(filesDir)
)

/** The [AgentRef.agentType] and [VerbSession.runtime] value for OpenCode sessions. */
const val OPENCODE_AGENT_TYPE = "opencode"
