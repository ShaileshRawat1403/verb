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

    private fun snapshot(changed: Int, head: String?, observed: Boolean = true, inRepo: Boolean = true) =
        GitSnapshot(
            observed = observed,
            insideRepository = inRepo,
            onNamedBranch = true,
            changedFiles = changed,
            headShort = head
        )

    /**
     * The question the product is named for: what did the last thing to run do to the tree.
     *
     * The baseline is the tree *before* that command. Measuring from "the last command that exited
     * 0" made the changing command its own baseline, and every delta came out zero.
     */
    @Test
    fun `the delta counts files the most recent command changed`() {
        val delta = GitDelta.between(snapshot(1, "a1b2c3d"), snapshot(4, "a1b2c3d"))

        assertTrue(delta.comparable)
        assertEquals(3, delta.changedFilesDelta)
        assertFalse(delta.headMoved)
    }

    @Test
    fun `a commit shows as HEAD moving and files leaving the working tree`() {
        val delta = GitDelta.between(snapshot(4, "a1b2c3d"), snapshot(0, "e4f5a6b"))

        assertTrue(delta.headMoved)
        assertEquals(-4, delta.changedFilesDelta)
    }

    /** One unobserved side makes the comparison unknown. It must never read as "nothing changed". */
    @Test
    fun `a delta against an unobserved snapshot is unknown, not zero`() {
        assertFalse(GitDelta.between(snapshot(2, "a1b2c3d"), GitSnapshot.unobserved()).comparable)
        assertFalse(GitDelta.between(GitSnapshot.unobserved(), snapshot(2, "a1b2c3d")).comparable)
        assertFalse(GitDelta.between(null, snapshot(2, "a1b2c3d")).comparable)
        assertFalse(GitDelta.between(snapshot(2, "a1b2c3d"), null).comparable)
    }

    /** A tree with no commits on either side cannot evidence a move, so it does not claim one. */
    @Test
    fun `an unknown HEAD on either side is not evidence that HEAD moved`() {
        assertFalse(GitDelta.between(snapshot(1, null), snapshot(1, "a1b2c3d")).headMoved)
        assertFalse(GitDelta.between(snapshot(1, "a1b2c3d"), snapshot(1, null)).headMoved)
        assertFalse(GitDelta.between(snapshot(1, null), snapshot(1, null)).headMoved)
    }

    @Test
    fun `leaving the repository makes the comparison unknown`() {
        assertFalse(GitDelta.between(snapshot(1, "a1b2c3d"), snapshot(0, null, inRepo = false)).comparable)
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
