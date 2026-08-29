package com.example.verb.terminal

import java.io.File

/**
 * Owns how an agent CLI is launched, so no vendor installer or self-update can take the command
 * away again.
 *
 * ## The failure this replaces
 *
 * Verb used to generate a launcher into `$PREFIX/bin` as part of each agent's install command. Both
 * halves of that were wrong, and both were measured on the validation device:
 *
 * - `$PREFIX/bin/claude` was **overwritten by npm** with a symlink to a Windows launcher
 *   (`claude.exe`) when the wrapper package installed, destroying Verb's file outright.
 * - `$HOME/.local/bin/claude` was added by Claude Code's **own self-installer**, won PATH ahead of
 *   `$PREFIX/bin`, and failed with `has unexpected e_type: 2`.
 *
 * The result was the worst kind of bug: the Agents tab correctly reported Claude Code as not
 * installed while it was installed *and authenticated*. The card was telling the truth; the
 * underlying state was wrong.
 *
 * ## Why this fix is permanent rather than another repair
 *
 * Three properties, each aimed at one of the ways the old scheme broke:
 *
 * 1. **A directory nothing else writes to.** [RELATIVE_BIN_DIR] is Verb-owned. npm installs into
 *    `$PREFIX/bin`, vendor self-installers into `$HOME/.local/bin`; neither has any reason to touch
 *    `$PREFIX/libexec/verb/bin`, and neither did.
 * 2. **First on PATH** (see [TerminalEnvironmentResolver]), ahead of `$HOME/.local/bin`, so a
 *    vendor launcher can no longer shadow a working command.
 * 3. **Rewritten unconditionally on every launch**, like the shell-integration script: these files
 *    are 100% Verb-authored with no user content to preserve, so regenerating them is always safe
 *    and makes the state self-healing. Anything that does manage to corrupt one is repaired by the
 *    next app start rather than by a support instruction.
 *
 * ## Resolution happens at launch, not at install
 *
 * A wrapper baked with one absolute path goes stale the moment the tool self-updates. These
 * wrappers instead walk the profile's [RuntimeProfile.binaryCandidates] at exec time, newest match
 * first for glob entries, so an update to a versioned install directory is picked up with no
 * reinstall. That also means every wrapper can be written before anything is installed: a wrapper
 * with no resolvable candidate exits 127, and starts working the moment the package lands.
 *
 * Those exit codes are deliberate. 127 and 126 are POSIX `env`'s own conventions for "not found on
 * PATH" and "found but not executable", which is exactly what [GuestCommandRunner] already reads to
 * decide a [ReadinessStage] -- so an agent that is genuinely absent still reports MISSING rather
 * than masquerading as installed-but-broken behind a wrapper that always exists.
 */
object AgentWrapperBootstrap {

    /** Host-side location, relative to the app's files directory. */
    const val RELATIVE_BIN_DIR = "usr/libexec/verb/bin"

    /** The same directory as the guest sees it. Must be first on the guest PATH. */
    const val GUEST_BIN_DIR = "${VerbGuestPaths.PREFIX}/libexec/verb/bin"

    /** Identifies a file in [RELATIVE_BIN_DIR] as Verb-generated, for tests and diagnostics. */
    const val MARKER = "VERB_AGENT_WRAPPER"

    /** POSIX `env`'s convention for "command not found", reused so probes read the truth. */
    private const val EXIT_NOT_FOUND = 127

    /** POSIX `env`'s convention for "found but not executable". */
    private const val EXIT_NOT_EXECUTABLE = 126

    /** The emulator that runs statically linked builds proot refuses. See [AgentBinaryAbi.STATIC]. */
    private const val QEMU_COMMAND = "qemu-aarch64"

    /**
     * Writes one wrapper per launchable agent and removes anything else in the directory.
     *
     * Safe to call on every launch; that is the point. Returns the commands wrapped, for logging.
     */
    fun install(filesDir: File): List<String> = runCatching {
        val binDir = File(filesDir, RELATIVE_BIN_DIR)
        if (!binDir.isDirectory && !binDir.mkdirs()) {
            TerminalSessionLogger.warn(LogCategory.IO, "Agent wrapper directory could not be created")
            return emptyList()
        }

        val agents = RuntimeProfiles.all.filter { it.isAgent }
        val wrapped = agents.mapNotNull { it.launchCommand }.toSet()
        // Verb's own command shares this directory, so the prune below must not treat it as a
        // leftover -- but it is not a wrapper, and does not belong in what this function reports.
        val expected = wrapped + VerbCliBootstrap.COMMAND

        // The directory is entirely Verb's, so anything unrecognised is a wrapper for an agent the
        // catalog no longer has. Leaving it behind would keep a dead command winning PATH.
        binDir.listFiles()?.forEach { existing ->
            if (existing.name !in expected) existing.delete()
        }

        for (profile in agents) {
            val command = profile.launchCommand ?: continue
            val wrapper = File(binDir, command)
            wrapper.writeText(wrapperScript(profile))
            wrapper.setExecutable(true, false)
            wrapper.setReadable(true, false)
        }
        TerminalSessionLogger.info(
            LogCategory.IO,
            "Agent launch wrappers written for: ${wrapped.sorted().joinToString(", ")}"
        )
        wrapped.sorted()
    }.onFailure {
        TerminalSessionLogger.warn(LogCategory.IO, "Agent wrapper install failed: ${it.message}")
    }.getOrDefault(emptyList())

    /**
     * The candidates actually searched for [profile], catalog entries first.
     *
     * The two trailing defaults are appended for every agent, including ones the catalog says
     * nothing about: `$PREFIX/bin/<command>` is where a package manager puts a launcher, and
     * `$HOME/.local/bin/<command>` is where vendor self-installers put theirs. Searching the second
     * one last is what turns the shadowing problem inside out -- the file that used to win PATH and
     * fail is now the last resort, reached only when nothing better exists, and reached through the
     * ABI detection that gives it its best chance of running at all.
     */
    fun candidatesFor(profile: RuntimeProfile): List<AgentBinaryCandidate> {
        val command = profile.launchCommand ?: return emptyList()
        return profile.binaryCandidates + listOf(
            AgentBinaryCandidate("\$PREFIX/bin/$command", AgentBinaryAbi.DETECT),
            AgentBinaryCandidate("\$HOME/.local/bin/$command", AgentBinaryAbi.DETECT)
        )
    }

    /** Visible for tests: the exact script written for [profile]. */
    fun wrapperScript(profile: RuntimeProfile): String {
        val command = requireNotNull(profile.launchCommand) { "${profile.id} has nothing to launch" }
        val candidates = candidatesFor(profile).map { it.copy(path = expand(it.path)) }
        require(candidates.none { it.path.startsWith(GUEST_BIN_DIR) }) {
            "$command would resolve back into the wrapper directory and loop"
        }

        val attempts = candidates.joinToString("\n\n") { candidate ->
            val resolve = if (candidate.path.contains('*')) {
                // Unquoted so the shell expands it; an unmatched glob stays literal and fails the
                // -f test below, which is the intended "nothing here" answer.
                "verb_bin=\$(verb_newest ${candidate.path})"
            } else {
                "verb_bin=\$(verb_newest \"${candidate.path}\")"
            }
            """
            |$resolve
            |if [ -n "${'$'}verb_bin" ]; then
            |    if [ -x "${'$'}verb_bin" ]; then
            |        ${execFunction(candidate.abi)} "${'$'}verb_bin" "${'$'}@"
            |    fi
            |    verb_unexecutable=${'$'}verb_bin
            |fi
            """.trimMargin()
        }

        return """
            |#!/bin/sh
            |# $MARKER -- generated by Verb on every launch. Do not edit; your edits are overwritten.
            |#
            |# Why this file is here and not in ${'$'}PREFIX/bin: npm overwrote ${'$'}PREFIX/bin/claude with a
            |# symlink to a Windows launcher when the wrapper package installed, and Claude Code's own
            |# self-installer added ${'$'}HOME/.local/bin/claude, which won PATH and failed with
            |# "has unexpected e_type: 2". Both destroyed a command that was installed and working.
            |# This directory is Verb-owned, first on PATH, and rewritten every launch, so neither a
            |# vendor installer nor a self-update can take ${'$'}(basename "${'$'}0") away again.
            |#
            |# Exit codes follow POSIX env's conventions, which Verb's readiness probe reads:
            |#   $EXIT_NOT_FOUND -> nothing installed to run;  $EXIT_NOT_EXECUTABLE -> found, but not executable.
            |
            |VERB_MUSL_LOADER=${MuslLoaderBootstrap.GUEST_LOADER_PATH}
            |VERB_QEMU=$QEMU_COMMAND
            |VERB_MUSL_LIB=${VerbGuestPaths.PREFIX}/lib/musl
            |
            |# A musl build is ET_EXEC (non-PIE), which Termux's exec shim rejects outright before the
            |# kernel sees it. Invoking the loader explicitly sidesteps that check -- the loader is
            |# itself static-PIE, and loading a non-PIE program is exactly its job. LD_PRELOAD is
            |# cleared with a shell builtin (not `env`, which would reintroduce the shim) because
            |# termux-exec is a Bionic library and fails inside a musl process with
            |# "__register_atfork: symbol not found".
            |verb_exec_musl() {
            |    verb_target=${'$'}1
            |    shift
            |    unset LD_PRELOAD
            |    LD_LIBRARY_PATH=${'$'}VERB_MUSL_LIB
            |    export LD_LIBRARY_PATH
            |    exec "${'$'}VERB_MUSL_LOADER" "${'$'}verb_target" "${'$'}@"
            |}
            |
            |# Bionic build or ordinary script. LD_PRELOAD is deliberately left alone: a script with a
            |# `#!/usr/bin/env node` shebang needs termux-exec to resolve it.
            |verb_exec_native() {
            |    verb_target=${'$'}1
            |    shift
            |    exec "${'$'}verb_target" "${'$'}@"
            |}
            |
            |# Statically linked, non-PIE (ET_EXEC). These run fine on the device -- Android's own
            |# shell executes them -- but proot refuses them with
            |#   error: "<path>" has unexpected e_type: 2
            |# and a terminal session is always inside proot, which nothing can escape from within.
            |# qemu maps the ELF itself instead of handing it to proot's loader, so the refusal never
            |# happens. Verb's own proot binary is such a build, and `proot --version` inside proot
            |# reproduces the same message -- which is how this was finally attributed.
            |verb_exec_static() {
            |    verb_target=${'$'}1
            |    shift
            |    if command -v ${'$'}VERB_QEMU >/dev/null 2>&1; then
            |        for verb_rootfs in "${VerbGuestPaths.FILES}"/agent-runtime/versions/*/rootfs; do
            |            if [ -d "${'$'}verb_rootfs" ]; then
            |                exec ${'$'}VERB_QEMU -L "${'$'}verb_rootfs" -E LD_LIBRARY_PATH=/usr/lib/aarch64-linux-gnu:/lib/aarch64-linux-gnu -U LD_PRELOAD "${'$'}verb_target" "${'$'}@"
            |            fi
            |        done
            |        exec ${'$'}VERB_QEMU -U LD_PRELOAD "${'$'}verb_target" "${'$'}@"
            |    fi
            |    printf '%s\n' "verb: $command needs the Agent Emulator (${'$'}VERB_QEMU) to run its statically linked build." >&2
            |    printf '%s\n' "verb: install it from the Agents tab, then run $command again." >&2
            |    exit $EXIT_NOT_EXECUTABLE
            |}
            |
            |# A binary runs on-device when its ELF interpreter exists -- so read the interpreter out
            |# of the file rather than assuming it. The .interp section sits in the first few hundred
            |# bytes, so this reads a fixed 4 KiB instead of scanning a binary that may be enormous.
            |verb_exec_detect() {
            |    verb_target=${'$'}1
            |    shift
            |    if command -v head >/dev/null 2>&1 && command -v grep >/dev/null 2>&1; then
            |        # Each test pipes the bytes straight into grep. Capturing them in a variable
            |        # would truncate at the first NUL, which every ELF has within a few bytes.
            |        # Every branch execs, so the first match wins and nothing falls through.
            |        if head -c 2 "${'$'}verb_target" 2>/dev/null | grep -q '#!'; then
            |            verb_exec_native "${'$'}verb_target" "${'$'}@"
            |        fi
            |        # An ELF names its interpreter in .interp, within the first few hundred bytes.
            |        if head -c 4096 "${'$'}verb_target" 2>/dev/null | grep -q ld-musl-aarch64; then
            |            verb_exec_musl "${'$'}verb_target" "${'$'}@"
            |        fi
            |        if head -c 4096 "${'$'}verb_target" 2>/dev/null | grep -qE 'ld-android|linker64'; then
            |            verb_exec_native "${'$'}verb_target" "${'$'}@"
            |        fi
            |        # An ELF naming no interpreter at all is static, or glibc needing rootfs; qemu runs it.
            |        if head -c 4 "${'$'}verb_target" 2>/dev/null | grep -q 'ELF'; then
            |            verb_exec_static "${'$'}verb_target" "${'$'}@"
            |        fi
            |        verb_exec_native "${'$'}verb_target" "${'$'}@"
            |    fi
            |    # No tools to read the interpreter with. Every binary that has reached this branch so
            |    # far has been a musl build, and the loader is the branch that works for them.
            |    verb_exec_musl "${'$'}verb_target" "${'$'}@"
            |}
            |
            |# Newest existing regular file among the arguments; prints nothing when none exist. This
            |# is what makes a vendor self-update land automatically: the version directory grows a new
            |# file and the next launch picks it, with no reinstall and no stale absolute path.
            |verb_newest() {
            |    verb_newest_match=
            |    for verb_candidate in "${'$'}@"; do
            |        [ -f "${'$'}verb_candidate" ] || continue
            |        if [ -z "${'$'}verb_newest_match" ] || [ "${'$'}verb_candidate" -nt "${'$'}verb_newest_match" ]; then
            |            verb_newest_match=${'$'}verb_candidate
            |        fi
            |    done
            |    [ -n "${'$'}verb_newest_match" ] && printf '%s\n' "${'$'}verb_newest_match"
            |}
            |
            |verb_unexecutable=
            |
            |$attempts
            |
            |if [ -n "${'$'}verb_unexecutable" ]; then
            |    printf '%s\n' "verb: found $command at ${'$'}verb_unexecutable, but it is not executable." >&2
            |    exit $EXIT_NOT_EXECUTABLE
            |fi
            |printf '%s\n' "verb: $command is not installed. Install ${profile.displayName} from the Agents tab." >&2
            |exit $EXIT_NOT_FOUND
            |""".trimMargin()
    }

    private fun execFunction(abi: AgentBinaryAbi): String = when (abi) {
        AgentBinaryAbi.MUSL -> "verb_exec_musl"
        AgentBinaryAbi.NATIVE -> "verb_exec_native"
        AgentBinaryAbi.STATIC -> "verb_exec_static"
        AgentBinaryAbi.DETECT -> "verb_exec_detect"
    }

    /**
     * Bakes the guest's absolute paths into the script rather than depending on `$PREFIX`/`$HOME`
     * being set. The wrapper then still resolves when something invokes it with a stripped
     * environment, which is the situation agents create constantly when they spawn subprocesses.
     */
    private fun expand(path: String): String = path
        .replace("\$PREFIX", VerbGuestPaths.PREFIX)
        .replace("\$HOME", VerbGuestPaths.HOME)
}
