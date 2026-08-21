package com.example.verb.session

import com.example.verb.terminal.CommandLifecycleState
import com.example.verb.terminal.TerminalRuntimeAdapter
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull
import java.io.File

/**
 * [AgentAdapter] for Claude Code, the first (and so far only) implementation -- Codex, OpenCode and
 * `dsh` each need their own, since resumability is agent-specific knowledge by design (see
 * `docs/VERB_SESSION_CONTRACT.md`).
 */
class ClaudeAgentAdapter(
    private val filesDir: File,
    private val projectDirectory: File?,
    private val terminalRuntimeAdapter: TerminalRuntimeAdapter,
    private val resumeSettleMs: Long = DEFAULT_RESUME_SETTLE_MS,
    private val pollIntervalMs: Long = DEFAULT_POLL_INTERVAL_MS
) : AgentAdapter {

    /**
     * Reads presence from Claude's own transcript files under
     * `~/.claude/projects/<cwd, "/" replaced by "-">/<session-uuid>.jsonl` -- the encoding Claude
     * Code itself uses, observed on device and recorded in `docs/DURABLE_SESSION.md`. Verb never
     * opens these files; presence (and, when a specific id is known, an exact filename match) is
     * the whole of the check, the same boundary [AgentSignInDetector] already holds for credentials.
     *
     * [ResumeVerdict.UNKNOWN] when there is no project directory to check against (nothing was ever
     * launched under a project) or the transcript directory cannot be listed -- never guessed as
     * [ResumeVerdict.NO], which would claim impossibility Verb has no evidence for.
     */
    override fun canResume(agent: AgentRef): ResumeVerdict {
        val project = projectDirectory ?: return ResumeVerdict.UNKNOWN
        val transcriptDir = File(filesDir, "home/.claude/projects/${encode(project)}")
        val transcripts = runCatching {
            transcriptDir.takeIf { it.isDirectory }?.listFiles { file -> file.isFile && file.extension == "jsonl" }
        }.getOrNull() ?: return ResumeVerdict.UNKNOWN

        val resumeIdentity = agent.resumeIdentity
        val hasMatch = if (resumeIdentity != null) {
            transcripts.any { it.nameWithoutExtension == resumeIdentity }
        } else {
            transcripts.isNotEmpty()
        }
        return if (hasMatch) ResumeVerdict.YES else ResumeVerdict.NO
    }

    /**
     * Sends `claude --resume <id>` (or `--continue` with no known id) and waits up to
     * [resumeSettleMs] to see whether it exits.
     *
     * Deliberately reads [TerminalRuntimeAdapter.commandHistory], not [TerminalRuntimeAdapter.terminalOutput]:
     * `terminalOutput` mirrors the terminal emulator's own screen, which echoes typed input as soon
     * as it is written to the PTY -- so a text marker embedded in the very command being sent would
     * appear to "complete" instantly, before the shell had done anything at all. `commandHistory` is
     * driven by real OSC 633 command-boundary events the shell's own prompt hooks emit, which fire
     * only when a command genuinely finishes, so it is immune to that race.
     *
     * The check is inverted from the usual "wait for completion" pattern (compare
     * `VerbViewModel.installOneProfile`), because resuming launches an interactive session rather
     * than running a script to completion: **a new, settled command-history record appearing at
     * all -- with any exit code -- means Claude already exited**, so resume did not produce a live
     * session. No such record appearing within the window is the expected shape of success, since a
     * genuinely resumed, still-running Claude will not return to the prompt until the user quits,
     * arbitrarily later.
     *
     * A device without shell integration active has no signal either way and this conservatively
     * reports failure -- see [AgentAdapter.resume]'s contract: a caller only ever advances to LIVE
     * on a non-null result, so "cannot confirm" and "confirmed failed" are safe to treat the same.
     */
    override suspend fun resume(agent: AgentRef): ProcessBinding? {
        val resumeArgument = agent.resumeIdentity?.let { "--resume $it" } ?: "--continue"
        val idsBefore = terminalRuntimeAdapter.commandHistory.value.mapTo(mutableSetOf()) { it.id }

        terminalRuntimeAdapter.sendCommand("claude $resumeArgument")

        val exitedWithinWindow = withTimeoutOrNull(resumeSettleMs) {
            while (true) {
                val settled = terminalRuntimeAdapter.commandHistory.value.firstOrNull {
                    it.id !in idsBefore && it.state != CommandLifecycleState.RUNNING
                }
                if (settled != null) break
                delay(pollIntervalMs)
            }
        }

        return if (exitedWithinWindow == null) ClaudeProcessBinding else null
    }

    private fun encode(project: File): String = project.absolutePath.replace('/', '-')

    private object ClaudeProcessBinding : ProcessBinding

    private companion object {
        const val DEFAULT_RESUME_SETTLE_MS = 5_000L
        const val DEFAULT_POLL_INTERVAL_MS = 200L
    }
}
