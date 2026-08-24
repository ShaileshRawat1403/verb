package com.example.verb.session

import com.example.verb.terminal.TerminalRuntimeAdapter
import kotlinx.coroutines.CoroutineScope
import java.io.File

/**
 * Binds Claude Code to the one shared session state machine, [AgentSessionCoordinator]. There is
 * nothing Claude-specific left here on purpose: everything Claude knows about itself -- where its
 * recovery evidence lives, what its stable conversation id is, how `--resume` behaves -- is in
 * [ClaudeAgentAdapter]. This function only says *which* adapter the shared coordinator should use.
 *
 * Kept as a function with the coordinator's original constructor shape so that adding Codex (and
 * later OpenCode) generalised the lifecycle code without disturbing the Claude path that was
 * proven end-to-end on a physical device.
 */
@Suppress("FunctionName")
fun ClaudeSessionCoordinator(
    filesDir: File,
    terminalRuntimeAdapter: TerminalRuntimeAdapter,
    coroutineScope: CoroutineScope,
    sessionStore: VerbSessionStore = InMemoryVerbSessionStore(),
    processBindingConfirmed: Boolean = false
): AgentSessionCoordinator = AgentSessionCoordinator(
    agentType = CLAUDE_AGENT_TYPE,
    adapterFactory = { project -> ClaudeAgentAdapter(filesDir, project, terminalRuntimeAdapter) },
    terminalRuntimeAdapter = terminalRuntimeAdapter,
    coroutineScope = coroutineScope,
    sessionStore = sessionStore,
    processBindingConfirmed = processBindingConfirmed,
    eventLog = VerbEventLog(filesDir)
)

/** The [AgentRef.agentType] and [VerbSession.runtime] value for Claude Code sessions. */
const val CLAUDE_AGENT_TYPE = "claude"
