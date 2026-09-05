package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * "Ready" only ever meant the binary runs. Codex and Claude Code were signed in while OpenCode and
 * `dsh` were not, and the Agents tab looked identical in two states that behave completely
 * differently the moment you tap Open.
 */
class AgentSignInDetectorTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    // Uniquely named: the marker loop below builds several in one test, and TemporaryFolder
    // refuses to hand out the same name twice.
    private var counter = 0
    private fun filesDir() =
        temporaryFolder.newFolder("files${counter++}").also { File(it, "home").mkdirs() }

    private fun write(filesDir: File, relative: String) {
        File(filesDir, "home/$relative").apply { parentFile?.mkdirs(); writeText("{}") }
    }

    @Test
    fun `a credential file means signed in`() {
        val filesDir = filesDir()
        write(filesDir, ".codex/auth.json")

        assertEquals(
            AgentSignInState.SIGNED_IN,
            AgentSignInDetector(filesDir).stateFor(RuntimeProfiles.forId(RuntimeProfileId.CODEX))
        )
    }

    @Test
    fun `no credential file means signed out`() {
        val filesDir = filesDir()

        assertEquals(
            AgentSignInState.SIGNED_OUT,
            AgentSignInDetector(filesDir).stateFor(RuntimeProfiles.forId(RuntimeProfileId.CODEX))
        )
    }

    /** Claude Code has moved this file between releases, so either location counts. */
    @Test
    fun `any declared marker is enough`() {
        val claude = RuntimeProfiles.forId(RuntimeProfileId.CLAUDE_CODE)
        claude.signedInMarkers.forEach { marker ->
            val filesDir = filesDir()
            write(filesDir, marker)
            assertEquals(
                "marker $marker must count as signed in",
                AgentSignInState.SIGNED_IN,
                AgentSignInDetector(filesDir).stateFor(claude)
            )
        }
    }

    /**
     * Hermes keeps its credential in the local userland home, like Claude and Codex.
     *
     * Observed on the Vivo I2202: `~/.hermes/auth.json`, written the moment a sign-in completed.
     */
    @Test
    fun `Hermes reports signed in from its own home directory`() {
        val filesDir = filesDir()
        write(filesDir, ".hermes/auth.json")

        assertEquals(
            AgentSignInState.SIGNED_IN,
            AgentSignInDetector(filesDir).stateFor(RuntimeProfiles.forId(RuntimeProfileId.HERMES))
        )
    }

    /**
     * Antigravity's credential is *not* in the local userland home, and putting one there must not
     * make it look signed in.
     *
     * It runs in the Agent Runtime, so `AgentSignInDetector` resolves its marker under
     * `agent-runtime/homes/default`. Getting this wrong in either direction is a false statement
     * about someone's login, so both directions are asserted.
     */
    @Test
    fun `Antigravity is read from the Agent Runtime home, not the local one`() {
        val agy = RuntimeProfiles.forId(RuntimeProfileId.ANTIGRAVITY)
        val marker = agy.signedInMarkers.single()

        val wrongHome = filesDir()
        write(wrongHome, marker)
        assertEquals(
            "a file in the local userland home must not count for an Agent Runtime agent",
            AgentSignInState.SIGNED_OUT,
            AgentSignInDetector(wrongHome).stateFor(agy)
        )

        val rightHome = filesDir()
        File(AgentRuntimePaths(rightHome).agentHome(AgentRuntimePaths.DEFAULT_AGENT), marker)
            .apply { parentFile?.mkdirs(); writeText("{}") }
        assertEquals(
            AgentSignInState.SIGNED_IN,
            AgentSignInDetector(rightHome).stateFor(agy)
        )
    }

    /**
     * The important one. Guessing a path and reporting absence from it would invent a fact, which
     * is the same mistake as the reverted `claude install` command. An agent whose credential
     * location has not been observed on a real device must say nothing at all.
     */
    @Test
    fun `an agent with no observed credential location reports unknown, never signed out`() {
        val filesDir = filesDir()

        listOf(RuntimeProfileId.OPENCODE, RuntimeProfileId.DEEPSEEK_HARNESS).forEach { id ->
            val profile = RuntimeProfiles.forId(id)
            assertEquals("$id declares no marker", emptyList<String>(), profile.signedInMarkers)
            assertEquals(
                "$id must not claim a sign-in state it cannot support",
                AgentSignInState.UNKNOWN,
                AgentSignInDetector(filesDir).stateFor(profile)
            )
        }
    }

    /**
     * Presence is the entire check. These files hold live credentials, and the boundary is the same
     * one the API keys card holds: never read, never logged, never sent anywhere.
     */
    @Test
    fun `an unreadable credential file still reports signed in`() {
        val filesDir = filesDir()
        val marker = File(filesDir, "home/.codex/auth.json").apply {
            parentFile?.mkdirs()
            writeText("{}")
            setReadable(false, false)
        }

        assertEquals(
            AgentSignInState.SIGNED_IN,
            AgentSignInDetector(filesDir).stateFor(RuntimeProfiles.forId(RuntimeProfileId.CODEX))
        )
        assertEquals(true, marker.exists())
    }
}
