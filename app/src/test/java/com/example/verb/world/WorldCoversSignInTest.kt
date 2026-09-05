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

    /**
     * All four admitted agents that keep a credential file are covered, named rather than counted.
     *
     * The generic test above passes trivially if a marker is quietly dropped from the catalog: an
     * agent Verb says nothing about is an agent this comparison has nothing to compare. This one
     * fails instead, which is the case that actually happened -- Hermes and Antigravity were signed
     * in on the validation device for two days while Verb reported neither, and `verb export` would
     * have restored a Hermes that was logged out.
     */
    @Test
    fun `the agents with an observed credential file are the ones Verb reports on`() {
        assertEquals(
            listOf("Antigravity", "Claude Code", "Codex CLI", "Hermes Agent"),
            RuntimeProfiles.all
                .filter { it.signedInMarkers.isNotEmpty() }
                .map { it.displayName }
                .sorted()
        )
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

    /**
     * Adding a path to the world is not free: whatever lives under it has to be archivable.
     *
     * `assert_payload_restorable` refuses an archive containing anything that is not a regular file
     * or a directory, because import refuses those too and an export that writes what import
     * rejects is not a backup. One symlink anywhere under a declared path stops the *whole* export.
     *
     * beta.8 added `.gemini` to cover Antigravity's sign-in and did not add exclusions with it.
     * Antigravity keeps `cli.log` as a symlink to its newest log file, so on a device with
     * Antigravity installed every `verb export` failed -- the backup gap was closed by removing
     * backups. Found by running an export on hardware, which is the only place it could be found.
     *
     * This does not re-run the export; it pins the pairing, so a future path added here without
     * exclusions being re-derived on a device fails the build instead of failing a user's restore.
     */
    @Test
    fun `every world path that is known to contain links carries exclusions for them`() {
        val paths = worldPaths()
        val excludes = script.readText()
            .substringAfter("WORLD_EXCLUDES=(")
            .substringBefore(")")

        // path fragment in WORLD_PATHS -> a fragment that must appear in WORLD_EXCLUDES.
        // Each entry was observed on the validation device, not guessed.
        val knownLinkSources = mapOf(
            ".codex" to ".codex/tmp",
            ".config/opencode" to "node_modules/.bin",
            ".gemini" to ".gemini/antigravity-cli/log"
        )

        for ((pathFragment, requiredExclude) in knownLinkSources) {
            val declared = paths.any { pathFragment in it }
            if (!declared) continue
            assertTrue(
                "world.sh archives a path containing '$pathFragment' but has no exclusion " +
                    "matching '$requiredExclude'. One symlink under a declared path makes every " +
                    "export fail, so the pairing is not optional.",
                requiredExclude in excludes
            )
        }
    }

    /** The refusal these exclusions exist to satisfy must still be in the script. */
    @Test
    fun `the export still refuses to write an archive import would reject`() {
        val text = script.readText()
        assertTrue(
            "assert_payload_restorable is gone; exports could start writing unrestorable archives",
            "assert_payload_restorable" in text
        )
        assertTrue(
            "the restorable check no longer rejects non-file, non-directory members",
            "tr -d 'd-'" in text
        )
    }

}
