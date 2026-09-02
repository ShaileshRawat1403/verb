package com.example.verb.world

import com.example.verb.terminal.ProfileEnvironment
import com.example.verb.terminal.RuntimeProfiles
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test

/**
 * If Verb can tell you an agent is signed in, the archive must be able to give that sign-in back.
 *
 * Those two facts lived in different languages and drifted apart. `AgentSignInDetector` reads
 * credential markers from a Kotlin catalog; `world.sh` carries a separate hand-written list of
 * paths. Antigravity was added to the first and not the second, so Verb reported an agent's login
 * while its own documented recovery path quietly excluded it -- and nothing failed, because nothing
 * compared the two.
 *
 * This is that comparison. It is a static read of the script rather than an execution of it, so it
 * runs anywhere and fails at the moment the catalog gains an agent the archive does not cover.
 */
class WorldCoversSignInTest {

    private val script = File("src/main/assets/verb/world.sh")

    /** The `WORLD_PATHS` array, as paths, ignoring comments and the array syntax around them. */
    private fun worldPaths(): List<String> {
        val body = script.readText()
            .substringAfter("WORLD_PATHS=(")
            .substringBefore("\n)")
        return body.lineSequence()
            .map { it.trim() }
            .filter { it.startsWith("\"") && it.endsWith("\"") }
            .map { it.trim('"') }
            .toList()
    }

    /** Where a marker declared relative to the guest `$HOME` actually lives under the app dir. */
    private fun appRelativePath(environment: ProfileEnvironment, marker: String): String =
        when (environment) {
            ProfileEnvironment.LOCAL_USERLAND -> "files/home/$marker"
            ProfileEnvironment.AGENT_RUNTIME -> "files/agent-runtime/homes/default/$marker"
        }

    @Test
    fun `every credential marker Verb reports on is inside the working world`() {
        assumeTrue(script.isFile)
        val paths = worldPaths()
        assertTrue("WORLD_PATHS could not be parsed", paths.isNotEmpty())

        val uncovered = RuntimeProfiles.all
            .filter { it.signedInMarkers.isNotEmpty() }
            .flatMap { profile ->
                profile.signedInMarkers.map { marker ->
                    profile.displayName to appRelativePath(profile.environment, marker)
                }
            }
            .filterNot { (_, path) ->
                paths.any { allowed -> path == allowed || path.startsWith("$allowed/") }
            }
            .map { (agent, path) -> "$agent -> $path" }

        assertEquals(
            "these agents report a sign-in state the Working World archive would not restore",
            emptyList<String>(),
            uncovered
        )
    }

    /**
     * The Agent Runtime home is a second credential location, not a variant of the first. Pinning
     * it here means removing it is a deliberate act with a failing test attached, rather than a
     * tidy-up nobody notices until a restore comes up short.
     */
    @Test
    fun `the agent runtime home is covered by the working world`() {
        assumeTrue(script.isFile)

        assertTrue(
            "Antigravity and any future Agent Runtime agent keep their sign-in here",
            worldPaths().any { it.startsWith("files/agent-runtime/homes/default/") }
        )
    }
}
