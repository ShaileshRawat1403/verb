package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Verb knows the dependency graph, so asking for a profile should mean asking for whatever it
 * needs. It used to detect a missing prerequisite and refuse, handing the work back to the user.
 */
class RuntimeInstallPlanTest {

    private fun plan(id: RuntimeProfileId, ready: Set<RuntimeProfileId> = emptySet()) =
        RuntimeProfiles.installPlan(id) { it in ready }.map { it.id }

    @Test
    fun `a profile with no prerequisites plans only itself`() {
        assertEquals(listOf(RuntimeProfileId.PYTHON), plan(RuntimeProfileId.PYTHON))
    }

    @Test
    fun `prerequisites are planned before the profile that needs them`() {
        val ordered = plan(RuntimeProfileId.CLAUDE_CODE)

        assertEquals(listOf(RuntimeProfileId.JAVASCRIPT, RuntimeProfileId.CLAUDE_CODE), ordered)
    }

    /**
     * Codex needs two: npm to fetch it, and the emulator to run it. Its only aarch64 build is
     * statically linked, and proot refuses to exec a static binary -- so planning the emulator is
     * what turns "cannot launch" into an install that actually works.
     */
    @Test
    fun `codex plans the emulator it needs to run at all`() {
        val ordered = plan(RuntimeProfileId.CODEX)

        assertEquals(
            listOf(RuntimeProfileId.JAVASCRIPT, RuntimeProfileId.AGENT_EMULATOR, RuntimeProfileId.CODEX),
            ordered
        )
    }

    @Test
    fun `an already-ready prerequisite is skipped but the profile still installs`() {
        val ordered = plan(RuntimeProfileId.CLAUDE_CODE, ready = setOf(RuntimeProfileId.JAVASCRIPT))

        assertEquals(listOf(RuntimeProfileId.CLAUDE_CODE), ordered)
    }

    @Test
    fun `a fully ready profile plans no work at all`() {
        val ordered = plan(
            RuntimeProfileId.CLAUDE_CODE,
            ready = setOf(RuntimeProfileId.JAVASCRIPT, RuntimeProfileId.CLAUDE_CODE)
        )

        assertTrue(ordered.isEmpty())
    }

    @Test
    fun `every agent CLI resolves its javascript prerequisite`() {
        listOf(RuntimeProfileId.CODEX, RuntimeProfileId.CLAUDE_CODE, RuntimeProfileId.GEMINI_CLI)
            .forEach { id ->
                val ordered = plan(id)
                assertEquals("install plan for $id ends with it", id, ordered.last())
                assertTrue(
                    "install plan for $id fetches npm first",
                    ordered.indexOf(RuntimeProfileId.JAVASCRIPT) == 0
                )
            }
    }

    @Test
    fun `a plan never repeats a profile`() {
        val ordered = RuntimeProfiles.installPlan(RuntimeProfileId.CODEX) { false }.map { it.id }

        assertEquals(ordered.distinct(), ordered)
    }
}

/**
 * Hermes needs Python below 3.14 while the package repository ships only 3.14, so it can never be
 * made ready. That is a different fact from "not installed yet", and the difference decides whether
 * an install button should exist at all.
 */
class RuntimeProfileSatisfiabilityTest {

    private fun report(
        incompatible: List<String> = emptyList(),
        missingPackages: List<String> = emptyList()
    ) = RuntimeProfileReport(
        profile = RuntimeProfiles.forId(RuntimeProfileId.HERMES),
        missingPackages = missingPackages,
        missingCommands = emptyList(),
        incompatibleCommands = incompatible
    )

    @Test
    fun `an incompatible version makes a profile unsatisfiable, not merely unready`() {
        val r = report(incompatible = listOf("python"))

        assertTrue(r.isUnsatisfiable)
        assertFalse(r.isReady)
        assertFalse("no install action may be offered", r.isInstallable)
    }

    @Test
    fun `a merely missing package stays installable`() {
        val r = report(missingPackages = listOf("python"))

        assertFalse(r.isUnsatisfiable)
        assertTrue(r.isInstallable)
    }

    @Test
    fun `a ready profile is neither unsatisfiable nor installable`() {
        val r = report()

        assertTrue(r.isReady)
        assertFalse(r.isUnsatisfiable)
        assertFalse(r.isInstallable)
    }

    /**
     * Hermes is no longer a dead end: it targets the versioned interpreter that satisfies its
     * constraint, so the profile is ordinary installable work rather than a permanent refusal.
     */
    @Test
    fun `hermes targets a compatible interpreter instead of an unsatisfiable constraint`() {
        val hermes = RuntimeProfiles.forId(RuntimeProfileId.HERMES)
        val interpreter = hermes.requirements.single { it.command == "python3.13" }

        assertEquals(null, interpreter.maxVersionExclusive)
        // Readiness also depends on the agent itself, not only on an interpreter existing.
        assertTrue(hermes.requirements.any { it.command == "hermes" })
    }
}
