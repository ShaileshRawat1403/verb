package com.example.verb.ui

import com.example.verb.intent.IntentEngine
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The suggestion chips are submitted verbatim to the intent engine, so their wording is not
 * cosmetic: a chip that reads well and resolves to nothing is worse than a blunt one that works.
 *
 * This pins both halves — that each chip still resolves, and that it resolves to the capability its
 * words promise. Reword them freely; this test is what stops a reword from quietly breaking one.
 */
class AskScreenSuggestionsTest {

    private val engine = IntentEngine()

    private fun resolve(prompt: String) = engine.resolveIntent(prompt)

    @Test
    fun `every suggestion chip resolves to the capability its words promise`() {
        assertEquals("file.list", resolve("List files in this directory").id)
        assertEquals("process.list", resolve("What processes are running").id)
        assertEquals("storage.summary", resolve("How much disk space is left").id)
        assertEquals("memory.summary", resolve("How much memory is free").id)
    }

    /**
     * "What processes are running" must not be read as a request to stop one. The engine checks
     * `process.stop` before `process.list`, and a chip that offered to kill something would be the
     * opposite of the safe default `docs/PRD.md` requires.
     */
    @Test
    fun `no suggestion resolves to an action that changes state`() {
        val prompts = listOf(
            "List files in this directory",
            "What processes are running",
            "How much disk space is left",
            "How much memory is free"
        )

        prompts.forEach { prompt ->
            val intent = resolve(prompt)
            assertEquals("'$prompt' must not stop a process", false, intent.id == "process.stop")
        }
    }
}
