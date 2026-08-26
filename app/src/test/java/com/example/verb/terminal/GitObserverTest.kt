package com.example.verb.terminal

import java.io.File
import java.time.Instant
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class GitObserverTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private val now: Instant = Instant.parse("2026-08-26T12:00:00Z")

    /** Answers keyed by the git subcommand, so a test says what git said and nothing more. */
    private fun observer(vararg answers: Pair<String, GitObserver.CommandOutput>): GitObserver {
        val byKey = answers.toMap()
        return GitObserver(temporaryFolder.root) { argv, _ ->
            byKey[argv.drop(1).joinToString(" ")] ?: GitObserver.CommandOutput(1, "")
        }
    }

    private fun ok(stdout: String) = GitObserver.CommandOutput(0, stdout)
    private fun failed() = GitObserver.CommandOutput(128, "fatal: not a git repository")

    private fun directory(): File = temporaryFolder.newFolder("work")

    @Test
    fun `a dirty tree reports counts, staged split and HEAD`() {
        val snapshot = observer(
            "rev-parse --is-inside-work-tree" to ok("true"),
            "branch --show-current" to ok("main"),
            "rev-parse --short HEAD" to ok("a1b2c3d"),
            "status --porcelain" to ok("M  staged.kt\n M unstaged.kt\n?? new.kt\n")
        ).observe(directory(), now)

        assertTrue(snapshot.observed)
        assertTrue(snapshot.insideRepository)
        assertTrue(snapshot.onNamedBranch)
        assertEquals(3, snapshot.changedFiles)
        assertEquals(1, snapshot.stagedFiles)
        assertEquals("a1b2c3d", snapshot.headShort)
    }

    @Test
    fun `a clean tree is observed with zero changes, not unobserved`() {
        val snapshot = observer(
            "rev-parse --is-inside-work-tree" to ok("true"),
            "branch --show-current" to ok("main"),
            "rev-parse --short HEAD" to ok("a1b2c3d"),
            "status --porcelain" to ok("")
        ).observe(directory(), now)

        assertTrue(snapshot.observed)
        assertEquals(0, snapshot.changedFiles)
    }

    /** `docs/README.md`: Unknown is not No. A git Verb could not run is not a clean tree. */
    @Test
    fun `git failing to run is unobserved, never clean`() {
        val snapshot = observer("rev-parse --is-inside-work-tree" to failed()).observe(directory(), now)

        assertFalse(snapshot.observed)
        assertFalse(snapshot.insideRepository)
        assertEquals(0, snapshot.changedFiles)
    }

    @Test
    fun `a directory outside any repository is observed and says so`() {
        val snapshot = observer(
            "rev-parse --is-inside-work-tree" to ok("false")
        ).observe(directory(), now)

        assertTrue(snapshot.observed)
        assertFalse(snapshot.insideRepository)
    }

    @Test
    fun `a missing directory is unobserved rather than assumed`() {
        assertFalse(GitObserver(temporaryFolder.root) { _, _ -> ok("true") }.observe(null, now).observed)
    }

    /** A repository before its first commit has no HEAD. That is unknown, not a fabricated value. */
    @Test
    fun `a repository with no commits reports an unknown HEAD`() {
        val snapshot = observer(
            "rev-parse --is-inside-work-tree" to ok("true"),
            "branch --show-current" to ok("main"),
            "rev-parse --short HEAD" to failed(),
            "status --porcelain" to ok("?? first.kt\n")
        ).observe(directory(), now)

        assertTrue(snapshot.observed)
        assertNull(snapshot.headShort)
        assertEquals(1, snapshot.changedFiles)
    }

    @Test
    fun `a detached HEAD is on no named branch`() {
        val snapshot = observer(
            "rev-parse --is-inside-work-tree" to ok("true"),
            "branch --show-current" to ok(""),
            "rev-parse --short HEAD" to ok("deadbee"),
            "status --porcelain" to ok("")
        ).observe(directory(), now)

        assertFalse(snapshot.onNamedBranch)
        assertEquals("deadbee", snapshot.headShort)
    }

    /**
     * The whole point of the type: it can carry how much moved, and cannot carry what moved. If a
     * file name or a branch name ever becomes a field, this test is where that decision gets made.
     */
    @Test
    fun `nothing a snapshot holds can name a file, a path or a branch`() {
        val snapshot = observer(
            "rev-parse --is-inside-work-tree" to ok("true"),
            "branch --show-current" to ok("feature/acme-corp-login"),
            "rev-parse --short HEAD" to ok("a1b2c3d"),
            "status --porcelain" to ok("M  src/acme/Secret.kt\n?? /private/client-name/notes.txt\n")
        ).observe(directory(), now)

        val rendered = snapshot.toString()
        assertFalse(rendered.contains("acme"))
        assertFalse(rendered.contains("client-name"))
        assertFalse(rendered.contains("Secret.kt"))
        assertFalse(rendered.contains("notes.txt"))
    }
}
