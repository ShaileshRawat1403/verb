package com.example.verb.session

import com.example.verb.terminal.TerminalRuntime

/**
 * Owns the process's one live [TerminalRuntime], so its lifetime stops being implicitly the
 * ViewModel's. Without this, a `VerbViewModel` created because the Activity was recreated for
 * real (not merely a config change -- rows 1/3/4 in `docs/DURABLE_SESSION.md` already survive
 * those) would construct a brand new `TerminalRuntime` and spawn a duplicate proot process, while
 * the old one leaked in the background with nothing left pointing to it. A new `VerbViewModel` now
 * reattaches to the same session instead.
 *
 * Deliberately process-scoped, not persisted: this does not survive process death (force-stop,
 * background kill) any more than the ViewModel-owned instance it replaces did -- the whole process,
 * this object included, disappears together. `docs/DURABLE_SESSION.md` already argues force-stop
 * should stay a hard boundary rather than something engineered around; closing the background-kill
 * gap is a foreground service, decided separately once session identity (this) exists.
 */
object VerbTerminalSessionHolder {

    @Volatile
    private var runtime: TerminalRuntime? = null

    /**
     * Which agent is running in that terminal, when one is.
     *
     * The binding proves a PTY survived; it never proved *what* was inside it, and that gap is why
     * two agents could both report "Running" while neither was: each coordinator saw the same live
     * terminal and claimed it. The marker has exactly the lifetime of the thing it describes -- it
     * lives beside the runtime, survives an Activity being recreated with it, and dies with the
     * process, which is precisely when Verb also stops being able to prove anything.
     */
    data class ForegroundBinding(
        val agentType: String,
        val commandIdsBeforeLaunch: Set<String>
    )

    @Volatile
    private var foreground: ForegroundBinding? = null

    /** Returns the process-scoped runtime if this Android process already owns one. */
    fun existing(): TerminalRuntime? = runtime

    /** Records that [agentType] now occupies the terminal. */
    fun claimForeground(agentType: String, commandIdsBeforeLaunch: Set<String>) {
        foreground = ForegroundBinding(agentType, commandIdsBeforeLaunch.toSet())
    }

    /** Records that [agentType] has left the terminal, if it was the one holding it. */
    fun releaseForeground(agentType: String) {
        if (foreground?.agentType == agentType) foreground = null
    }

    /** The agent occupying the terminal, or null when it is back at a shell prompt. */
    fun foregroundAgent(): String? = foreground?.agentType

    /**
     * Runtime-only evidence needed to reattach an agent-exit watch after Activity/ViewModel
     * recreation. The command baseline is deliberately kept beside the PTY, never persisted:
     * command-history IDs and process presence have meaning only inside this Android process.
     */
    fun foregroundBinding(): ForegroundBinding? = foreground

    fun getOrCreate(factory: () -> TerminalRuntime): TerminalRuntime =
        runtime ?: synchronized(this) {
            runtime ?: factory().also { runtime = it }
        }

    /**
     * Test-only. Without this, a JVM test run's [TerminalRuntime] leaks into the next test's, since
     * this object is a singleton for the life of the JVM, not just the (simulated) app process.
     */
    fun resetForTests() {
        runtime = null
        foreground = null
    }
}
