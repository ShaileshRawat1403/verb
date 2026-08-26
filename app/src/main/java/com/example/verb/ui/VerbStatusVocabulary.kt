package com.example.verb.ui

import com.example.verb.session.VerbSessionState
import com.example.verb.terminal.CommandLifecycleState
import com.example.verb.terminal.TerminalSessionState

/**
 * One place that turns a state into something a person can read.
 *
 * Two rules from `docs/UX_FOUNDATION.md` are enforced here rather than left to each screen:
 *
 * * **Colour never carries meaning alone.** Every state has a glyph *and* a word, so the status
 *   survives a colour-blind reader, a greyscale screenshot and a screen reader. The Android tab dot
 *   this replaces carried its meaning in nothing but a 7dp coloured circle.
 * * **The glyph set is narrow on purpose.** `● ◐ ◌ ○ ✕` and nothing wider, matching
 *   `desktop/src/tui/theme.rs` exactly, so a person who learns one host has learned the other.
 *
 * Nothing here computes state. These are total functions over states other code already resolved --
 * `TerminalSessionState` from the process host, `VerbSessionState` from the session coordinator --
 * which is what keeps the UI from becoming a second, disagreeing status calculation
 * (`docs/VERB_SESSION_CONTRACT.md`, invariant 2).
 *
 * The two vocabularies are deliberately *not* merged. `TerminalSessionState` describes the
 * `ProcessBinding`'s own lifecycle; `VerbSessionState` is one level up and answers a different
 * question. Blurring them into one string is the mistake the contract calls out by name.
 */
object VerbStatusVocabulary {

    /** No state at all: the host has not reported one yet. Not the same claim as "ended". */
    const val UNKNOWN_GLYPH: String = "◌"
    const val UNKNOWN_WORD: String = "not ready"

    /**
     * The glyph for the process host's own lifecycle.
     *
     * There is deliberately no `◐` here: "recoverable" is a positive claim about an agent
     * conversation, which a terminal session state cannot make.
     */
    fun processGlyph(state: TerminalSessionState?): String = when (state) {
        TerminalSessionState.RUNNING -> "●"
        TerminalSessionState.STARTING, TerminalSessionState.STOPPING -> "◌"
        TerminalSessionState.EXITED -> "○"
        TerminalSessionState.FAILED -> "✕"
        null -> UNKNOWN_GLYPH
    }

    /** The word beside [processGlyph]. Unchanged from what the workspace already displayed. */
    fun processWord(state: TerminalSessionState?): String = when (state) {
        TerminalSessionState.RUNNING -> "running"
        TerminalSessionState.STARTING -> "starting"
        TerminalSessionState.STOPPING -> "stopping"
        TerminalSessionState.EXITED -> "exited"
        TerminalSessionState.FAILED -> "failed"
        null -> UNKNOWN_WORD
    }

    /**
     * What a screen reader announces for the process status control.
     *
     * The restart hint is included only when restarting is actually what a tap would do, because a
     * control that describes an action it will not take is the ambiguity Verb exists to remove.
     */
    fun processDescription(state: TerminalSessionState?): String {
        val word = processWord(state)
        return if (state == TerminalSessionState.RUNNING) {
            "Terminal session $word"
        } else {
            "Terminal session $word. Activate to start a new session."
        }
    }

    /** The glyph for a tracked agent session, in the contract's own four states. */
    fun sessionGlyph(state: VerbSessionState?): String = when (state) {
        VerbSessionState.LIVE -> "●"
        VerbSessionState.RECOVERABLE -> "◐"
        VerbSessionState.INTERRUPTED -> "◌"
        VerbSessionState.ENDED -> "○"
        null -> UNKNOWN_GLYPH
    }

    /**
     * The plain word for a tracked agent session.
     *
     * `docs/UX_FOUNDATION.md`: plain language on screen, the contract's exact vocabulary
     * underneath. `LIVE` stays what the record and `--json` say; "running" is the reading.
     */
    fun sessionWord(state: VerbSessionState?): String = when (state) {
        VerbSessionState.LIVE -> "running"
        VerbSessionState.RECOVERABLE -> "recoverable"
        VerbSessionState.INTERRUPTED -> "recovery status unknown"
        VerbSessionState.ENDED -> "ended"
        null -> "no session"
    }

    /** What a screen reader announces for an agent's session status. */
    fun sessionDescription(agentName: String, state: VerbSessionState?): String =
        "$agentName session ${sessionWord(state)}"

    /**
     * The glyph for one command boundary the shell reported.
     *
     * `◐` is absent for the same reason it is absent from [processGlyph]: "recoverable" is a claim
     * about an agent conversation, and a finished command cannot make it.
     */
    fun commandGlyph(state: CommandLifecycleState): String = when (state) {
        CommandLifecycleState.RUNNING -> "●"
        CommandLifecycleState.COMPLETED -> "○"
        CommandLifecycleState.FAILED -> "✕"
        CommandLifecycleState.ABANDONED -> "◌"
    }

    /**
     * The plain word beside [commandGlyph].
     *
     * `ABANDONED` reads as "never finished" rather than "abandoned": the record says no exit code
     * was ever observed, which is a statement about what Verb knows, not about what the command
     * did. Calling it "failed" would be the inference-as-fact mistake the contract forbids.
     */
    fun commandWord(state: CommandLifecycleState): String = when (state) {
        CommandLifecycleState.RUNNING -> "running"
        CommandLifecycleState.COMPLETED -> "finished"
        CommandLifecycleState.FAILED -> "failed"
        CommandLifecycleState.ABANDONED -> "never finished"
    }
}
