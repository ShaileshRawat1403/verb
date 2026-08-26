package com.example.verb.terminal

import java.io.File
import java.time.Instant

/**
 * What Verb knows about the working tree, as facts rather than as content.
 *
 * Deliberately no paths, no file names, no diff and no branch name. The repository root is the same
 * kind of disclosure as the working directory the envelope already withholds -- `/private/acme-corp`
 * names a client -- and a branch name carries it just as readily (`feature/acme-login`). What is
 * actually needed to answer *"what did the agent just change?"* is how much moved and whether HEAD
 * moved with it, and none of that requires naming anything.
 *
 * [observed] is false when Verb could not look. That is not the same claim as a clean tree, and the
 * two must never be collapsed -- `docs/README.md`: Unknown is not No.
 */
data class GitSnapshot(
    /** False when the guest was unavailable, git was missing, or the command did not finish. */
    val observed: Boolean,
    /** False when the directory is not inside a work tree at all. Meaningless when not [observed]. */
    val insideRepository: Boolean = false,
    /** True when HEAD is on a named branch rather than detached. The name itself is withheld. */
    val onNamedBranch: Boolean = false,
    /** Files git reports as changed, staged and unstaged together. */
    val changedFiles: Int = 0,
    /** Of [changedFiles], how many are staged. */
    val stagedFiles: Int = 0,
    /** The abbreviated commit HEAD points at -- opaque, and the only way to see that HEAD moved. */
    val headShort: String? = null,
    val observedAt: Instant = Instant.EPOCH
) {
    companion object {
        /** Verb looked and could not tell. Distinct from a clean tree in every rendering. */
        fun unobserved(at: Instant = Instant.now()) = GitSnapshot(observed = false, observedAt = at)
    }
}

/**
 * What the most recent command did to the working tree.
 *
 * This is the question `docs/PRODUCT_VISION.md` names the product after -- *what did the agent just
 * do to my world* -- and it needs two snapshots, not one. A count on its own says the tree is dirty;
 * a count taken either side of a command says what that command actually did.
 *
 * An earlier version measured against "the last command that exited 0", which read well and was
 * useless: the command that made the change usually exits 0 too, so it became its own baseline and
 * every delta was zero. Verified on a device, where creating three files reported no change. The
 * baseline has to be the tree *before* the command, not after some earlier one.
 *
 * Both sides must be observed for the delta to mean anything. One unobserved snapshot makes the
 * comparison unknown, never zero.
 */
data class GitDelta(
    val comparable: Boolean,
    /** Positive when files changed since; negative when they were reverted or committed away. */
    val changedFilesDelta: Int = 0,
    /** True when HEAD is not the commit it was -- something committed, checked out or reset. */
    val headMoved: Boolean = false
) {
    companion object {
        val UNKNOWN = GitDelta(comparable = false)

        /**
         * [before] is the tree as it stood before the most recent command; [after] is after it.
         * A missing HEAD on either side is not evidence of a move, so [headMoved] stays false
         * unless both sides named a commit and the names differ.
         */
        fun between(before: GitSnapshot?, after: GitSnapshot?): GitDelta {
            if (before == null || after == null) return UNKNOWN
            if (!before.observed || !after.observed) return UNKNOWN
            if (!before.insideRepository || !after.insideRepository) return UNKNOWN
            val bothNamedACommit = before.headShort != null && after.headShort != null
            return GitDelta(
                comparable = true,
                changedFilesDelta = after.changedFiles - before.changedFiles,
                headMoved = bothNamedACommit && before.headShort != after.headShort
            )
        }
    }
}

/**
 * Reads the working tree through the guest userland, the same way a user's own `git` would run.
 *
 * Three bounded commands, each of which either answers or is treated as unknown. Nothing here
 * fabricates: a failed `status` does not become "clean", and a guest that is not installed produces
 * [GitSnapshot.unobserved] rather than an empty repository.
 *
 * The timeout is short on purpose. This runs under proot on a phone, off the terminal's hot path,
 * and an answer that arrives late is worth less than the session staying responsive. A snapshot
 * that times out is simply not observed.
 */
class GitObserver(
    private val appFilesDir: File,
    private val runCommand: (List<String>, File) -> CommandOutput = ::runInGuest
) {

    /** Stdout of a guest command, or null when it did not run or exited nonzero. */
    data class CommandOutput(val exitCode: Int?, val stdout: String) {
        val succeeded: Boolean get() = exitCode == 0
        fun valueOrNull(): String? = stdout.trim().takeIf { succeeded && it.isNotEmpty() }
    }

    fun observe(directory: File?, now: Instant = Instant.now()): GitSnapshot {
        val target = directory?.takeIf { it.isDirectory } ?: return GitSnapshot.unobserved(now)

        val insideWorkTree = runCommand(listOf("git", "rev-parse", "--is-inside-work-tree"), target)
        if (!insideWorkTree.succeeded) {
            // Could not run git at all, or this is not a repository. The two are told apart by
            // whether git answered: a non-repository answers "false" on stderr with a nonzero exit,
            // which is indistinguishable here from git being absent -- so neither is claimed.
            return GitSnapshot.unobserved(now)
        }
        if (insideWorkTree.valueOrNull() != "true") {
            return GitSnapshot(observed = true, insideRepository = false, observedAt = now)
        }

        val branch = runCommand(listOf("git", "branch", "--show-current"), target).valueOrNull()
        val head = runCommand(listOf("git", "rev-parse", "--short", "HEAD"), target).valueOrNull()
        val status = runCommand(listOf("git", "status", "--porcelain"), target)

        // A repository with no commits has no HEAD, and a status that did not run is unknown rather
        // than zero -- so the count is only reported when git actually produced one.
        val lines = if (status.succeeded) {
            status.stdout.lines().filter { it.isNotBlank() }
        } else {
            null
        }

        return GitSnapshot(
            observed = true,
            insideRepository = true,
            onNamedBranch = branch != null,
            changedFiles = lines?.size ?: 0,
            // Porcelain v1 puts the staged status in column one; a space there means unstaged only.
            stagedFiles = lines?.count { it.length >= 2 && it[0] != ' ' && it[0] != '?' } ?: 0,
            headShort = head,
            observedAt = now
        )
    }

    companion object {
        /** Short: this runs under proot, off the hot path, and lateness is worth less than silence. */
        const val TIMEOUT_MS = 4_000L

        private fun runInGuest(argv: List<String>, directory: File): CommandOutput {
            val resolver = TerminalEnvironmentResolver(
                appFilesDir = directory.guestAppFilesDir(),
                projectDirectory = directory
            )
            val environment = resolver.resolveGuestCommand(argv)
                ?: return CommandOutput(null, "")
            val result = BoundedProcessRunner.run(
                argv = environment.arguments.toList(),
                environment = environment.variables,
                workingDirectory = environment.workingDirectory,
                timeoutMs = TIMEOUT_MS
            )
            return CommandOutput(result.exitCode, result.output)
        }

        /**
         * The guest rootfs is the app's files directory, which every project directory lives under.
         * Walking up to it keeps the caller from having to thread it through separately.
         */
        private fun File.guestAppFilesDir(): File {
            var candidate: File? = this
            while (candidate != null) {
                if (File(candidate, "usr/bin/proot").exists()) return candidate
                candidate = candidate.parentFile
            }
            return this
        }
    }
}
