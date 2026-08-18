package com.example.verb.terminal

import java.io.File

/**
 * The shell's live working directory, expressed in both namespaces Verb has to deal with.
 *
 * Guest paths and host paths are NOT the same namespace. The shell reports a path as seen from
 * inside the proot guest (e.g. `/data/data/com.aistudio.verb.app/files/projects/demo`, or
 * `/workspace` in the Agent Runtime); that string is meaningless to `java.io.File` on the host
 * unless it is translated through one of the binds Verb itself established. Constructing
 * `File(guestPath)` directly would silently produce a host path that either does not exist or --
 * worse -- points somewhere unrelated. That is why the two are separate fields here rather than
 * one "path" string.
 *
 * @param guestPath exactly what the shell reported (advisory OSC 7; see [ShellIntegrationEvent]).
 *   Always present -- if Verb has no cwd at all, the whole [TerminalWorkingDirectory] is null.
 * @param hostPath the corresponding host directory, and only when [GuestPathMapper] could map the
 *   guest path through a known, allowlisted bind. Null means "the shell is somewhere Verb cannot
 *   address on the host" -- an honest unknown, never a fallback to the launch directory.
 */
data class TerminalWorkingDirectory(
    val guestPath: String,
    val hostPath: File?
)

/**
 * Translates guest paths to host paths through an explicit allowlist of binds, and refuses
 * everything else.
 *
 * The allowlist mirrors the binds Verb actually establishes in [TerminalEnvironmentResolver] and
 * [AgentRuntimeEnvironment], deliberately narrowed to the roots a working directory can legitimately
 * live under. Anything outside them -- including `/data/data/com.termux`, the hidden internal
 * compatibility alias that must never be surfaced as Verb's identity (see [VerbGuestPaths]) --
 * resolves to null rather than being guessed at.
 *
 * Traversal is handled structurally, not by string inspection: the candidate host path is
 * canonicalized and then required to still sit inside the canonical host root of the bind it
 * matched. A guest path containing `..` that escapes its bind therefore fails the containment
 * check and is rejected, exactly like a path under an unknown root.
 */
class GuestPathMapper private constructor(private val binds: List<Bind>) {

    /** One allowlisted guest root and the host directory it is bound to. */
    data class Bind(val guestRoot: String, val hostRoot: File)

    /**
     * Returns the host directory for [guestPath], or null when it cannot be safely mapped.
     *
     * Null is returned for: a relative or blank path, a path under no allowlisted bind, and a path
     * that canonicalizes outside the host root of the bind it matched.
     */
    fun toHostPath(guestPath: String): File? {
        if (!guestPath.startsWith("/")) return null
        // Longest guest root first so a more specific bind always wins over a broader one.
        val bind = binds
            .filter { matchesRoot(guestPath, it.guestRoot) }
            .maxByOrNull { it.guestRoot.length }
            ?: return null

        val relative = guestPath.removePrefix(bind.guestRoot).trimStart('/')
        return runCatching {
            val hostRoot = bind.hostRoot.canonicalFile
            val candidate = (if (relative.isEmpty()) hostRoot else File(hostRoot, relative)).canonicalFile
            candidate.takeIf { isContained(it, hostRoot) }
        }.getOrNull()
    }

    /** Matches only on a whole path component, so `/workspaceX` never matches the `/workspace` bind. */
    private fun matchesRoot(guestPath: String, guestRoot: String): Boolean =
        guestPath == guestRoot || guestPath.startsWith("$guestRoot/")

    private fun isContained(candidate: File, root: File): Boolean {
        var current: File? = candidate
        while (current != null) {
            if (current == root) return true
            current = current.parentFile
        }
        return false
    }

    companion object {
        /**
         * Maps nothing. Used where Verb has established no guest filesystem at all (the Android
         * system shell), so every guest path is honestly unmappable rather than mis-mapped.
         */
        val NONE: GuestPathMapper = GuestPathMapper(emptyList())

        /**
         * The normal Verb CLI userland: [VerbGuestPaths.FILES] is bound to the app's real files
         * directory, so guest project/home/prefix paths all resolve beneath it.
         *
         * Deliberately narrower than the full bind set in [TerminalEnvironmentResolver]: only the
         * one root a working directory can legitimately sit under is allowlisted here. The legacy
         * `/data/data/com.termux` compatibility alias is intentionally absent -- it is an internal
         * mount, never Verb's user-visible identity, so a cwd reported under it stays unmapped.
         */
        fun verbUserland(filesDir: File): GuestPathMapper =
            GuestPathMapper(listOf(Bind(VerbGuestPaths.FILES, filesDir)))

        /**
         * The Agent Runtime: the selected project is bound at `/workspace` (see
         * [AgentRuntimeEnvironment]). The agent rootfs's own `/usr`, `/home/verb`, etc. are
         * deliberately not allowlisted -- they are not host-addressable app storage.
         *
         * Registered now so the mapping is already correct if/when shell integration reaches that
         * runtime; today the agent rootfs ships no shell-integration script, so no OSC 7 arrives
         * and the current working directory simply stays unknown.
         */
        fun agentRuntime(projectDirectory: File): GuestPathMapper =
            GuestPathMapper(listOf(Bind(AGENT_WORKSPACE_GUEST_ROOT, projectDirectory)))

        /** Guest mount point the selected project is bound to in the Agent Runtime. */
        const val AGENT_WORKSPACE_GUEST_ROOT = "/workspace"
    }
}
