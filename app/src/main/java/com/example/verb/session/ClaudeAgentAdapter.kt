package com.example.verb.session

import com.example.verb.terminal.TerminalRuntimeAdapter
import java.io.File

/**
 * [AgentAdapter] for Claude Code. Codex has its own ([CodexAgentAdapter]), and OpenCode and `dsh`
 * will each need one too, since resumability is agent-specific knowledge by design (see
 * `docs/VERB_SESSION_CONTRACT.md`). The session lifecycle around all of them is shared and lives in
 * [AgentSessionCoordinator].
 */
class ClaudeAgentAdapter(
    private val filesDir: File,
    private val projectDirectory: File?,
    private val terminalRuntimeAdapter: TerminalRuntimeAdapter,
    private val resumeSettleMs: Long = DEFAULT_RESUME_SETTLE_MS,
    private val pollIntervalMs: Long = DEFAULT_POLL_INTERVAL_MS
) : AgentAdapter {

    /**
     * Checks Claude-owned resume markers without persisting or reading transcript contents.
     *
     * Claude builds use either project transcript files under
     * `~/.claude/projects/<Claude's normalized cwd>/<transcript>.jsonl` or, as the Android arm64 build does,
     * small metadata records under `~/.claude/sessions/` (JSON files). The latter contain the Claude
     * session id, CWD and lifecycle metadata; they are the useful recovery evidence after Verb's
     * process dies. Verb only matches their CWD (and an explicit session id when known).
     *
     * [ResumeVerdict.UNKNOWN] means the host gave us no readable evidence either way. It is never
     * guessed as [ResumeVerdict.NO], which would claim impossibility Verb has not established.
     */
    override fun canResume(agent: AgentRef): ResumeVerdict {
        val project = projectDirectory ?: return ResumeVerdict.UNKNOWN
        val transcriptVerdict = runCatching {
            val transcriptDirs = transcriptDirectories(project).filter { it.isDirectory }
            if (transcriptDirs.isEmpty()) {
                null
            } else {
                transcriptDirs.flatMap { directory ->
                    directory.listFiles { file -> file.isFile && file.extension == "jsonl" }?.toList().orEmpty()
                }
            }
        }.getOrNull()?.let { transcripts ->
            val resumeIdentity = agent.resumeIdentity
            val hasMatch = if (resumeIdentity != null) {
                transcripts.any { it.nameWithoutExtension == resumeIdentity }
            } else {
                transcripts.isNotEmpty()
            }
            if (hasMatch) ResumeVerdict.YES else ResumeVerdict.NO
        }

        if (transcriptVerdict == ResumeVerdict.YES) return ResumeVerdict.YES

        return sessionMetadataVerdict(project, agent) ?: transcriptVerdict ?: ResumeVerdict.UNKNOWN
    }

    /**
     * Returns Claude's stable conversation id when this host exposes one. The filename in the
     * Android session directory is a PID and is intentionally never returned or persisted.
     */
    override fun resumeIdentity(agent: AgentRef): String? {
        val project = projectDirectory ?: return null
        return matchingSessionMetadata(sessionMetadataFiles() ?: return null, project, agent)
            .firstOrNull()
    }

    private fun sessionMetadataFiles(): List<File>? {
        val sessionDirectory = File(filesDir, "home/.claude/sessions")
        if (!sessionDirectory.isDirectory) return null
        return runCatching {
            sessionDirectory.takeIf { it.isDirectory }
                ?.listFiles { file -> file.isFile && file.extension == "json" }
                ?.toList()
        }.getOrNull()
    }

    /** Returns null when this Claude installation exposes no readable session metadata store. */
    private fun sessionMetadataVerdict(project: File, agent: AgentRef): ResumeVerdict? {
        val sessionFiles = sessionMetadataFiles() ?: return null
        return if (matchingSessionMetadata(sessionFiles, project, agent).isNotEmpty()) {
            ResumeVerdict.YES
        } else {
            ResumeVerdict.NO
        }
    }

    /** Newer Claude session metadata wins when several historical sessions share one CWD. */
    private fun matchingSessionMetadata(
        sessionFiles: List<File>,
        project: File,
        agent: AgentRef
    ): List<String> {
        data class Match(val sessionId: String, val updatedAt: Long)

        return sessionFiles.mapNotNull { file ->
            val metadata = runCatching { file.readText() }.getOrNull() ?: return@mapNotNull null
            val cwd = JSON_CWD_PATTERN.find(metadata)?.groupValues?.get(1) ?: return@mapNotNull null
            val sessionId = ResumeIdentity.validOrNull(
                JSON_SESSION_ID_PATTERN.find(metadata)?.groupValues?.get(1)
            ) ?: return@mapNotNull null
            if (pathsRepresentSameProject(cwd, project) &&
                (agent.resumeIdentity == null || agent.resumeIdentity == sessionId)
            ) {
                Match(
                    sessionId = sessionId,
                    updatedAt = JSON_UPDATED_AT_PATTERN.find(metadata)?.groupValues?.get(1)?.toLongOrNull() ?: 0L
                )
            } else null
        }.sortedByDescending { it.updatedAt }.map { it.sessionId }
    }

    /** `/data/user/0` and `/data/data` are aliases on Android app-private storage. */
    private fun pathsRepresentSameProject(metadataCwd: String, project: File): Boolean =
        GuestPathAliases.sameDirectory(metadataCwd, project)

    private fun transcriptDirectories(project: File): Set<File> = buildSet {
        val projectAliases = GuestPathAliases.aliasesOf(project.absolutePath)
        val filesRoots = GuestPathAliases.aliasesOf(filesDir)
        filesRoots.forEach { root ->
            projectAliases.forEach { alias ->
                add(File(root, "home/.claude/projects/${ClaudeProjectDirectory.encode(alias)}"))
            }
        }
    }

    /**
     * Sends `claude --resume <id>` (or `--continue` with no known id) and waits up to
     * [resumeSettleMs] to see whether it exits. [AgentResumeLauncher] owns the reasoning about why
     * "nothing settled" is the shape of success here.
     */
    override suspend fun resume(agent: AgentRef): ProcessBinding? {
        val resumeArgument = ResumeIdentity.validOrNull(agent.resumeIdentity)
            ?.let { "--resume $it" }
            ?: "--continue"
        val stillRunning = AgentResumeLauncher.launch(
            terminalRuntimeAdapter = terminalRuntimeAdapter,
            command = "claude $resumeArgument",
            settleMs = resumeSettleMs,
            pollIntervalMs = pollIntervalMs
        )
        return if (stillRunning) ClaudeProcessBinding else null
    }

    private object ClaudeProcessBinding : ProcessBinding

    private companion object {
        val JSON_CWD_PATTERN = Regex("\"cwd\"\\s*:\\s*\"([^\"]*)\"")
        val JSON_SESSION_ID_PATTERN = Regex("\"sessionId\"\\s*:\\s*\"([^\"]*)\"")
        val JSON_UPDATED_AT_PATTERN = Regex("\"updatedAt\"\\s*:\\s*(\\d+)")
        const val DEFAULT_RESUME_SETTLE_MS = 5_000L
        const val DEFAULT_POLL_INTERVAL_MS = 200L
    }
}
