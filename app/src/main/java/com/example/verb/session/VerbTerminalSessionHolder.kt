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
    }
}
