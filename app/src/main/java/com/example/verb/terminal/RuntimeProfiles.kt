package com.example.verb.terminal

import java.io.File

enum class RuntimeProfileId {
    CORE,
    /** The Termux User Repository, which carries versioned toolchains the main repo does not. */
    TUR,
    PYTHON,
    HERMES,
    JAVASCRIPT,
    CODEX,
    CLAUDE_CODE,
    GEMINI_CLI,
    OPENCODE,
    NATIVE,
    REMOTE,
    DATA_MEDIA
}

data class RuntimeRequirement(
    val command: String,
    val packageName: String,
    val maxVersionExclusive: String? = null,
    /**
     * Catalog-owned, fixed argument list (e.g. `["--version"]`) used to bounded-probe the
     * resolved binary once it is executable. Never derived from user input. Left null for
     * apt-tracked packages, where dpkg's Version field already gives a reliable signal and
     * spawning a process on every check would be unnecessary overhead.
     */
    val versionProbeArgs: List<String>? = null
)

data class RuntimeProfile(
    val id: RuntimeProfileId,
    val displayName: String,
    val packages: List<String>,
    val requirements: List<RuntimeRequirement>,
    val prerequisiteProfiles: List<RuntimeProfileId> = emptyList(),
    val installCommandOverride: String? = null,
    val postInstallHint: String? = null
) {
    /** Safe because package names are catalog-owned, not user-provided shell input. */
    val installCommand: String
        get() = installCommandOverride
            ?: "apt-get update && apt-get install -y --no-install-recommends ${packages.joinToString(" ")}"
}

/**
 * Where a single requirement sits on the preflight -> install -> resolve -> execute/version probe
 * -> ready pipeline. Distinguishes readiness signals a plain "is the command present" check would
 * conflate: a package can be recorded installed by dpkg without its command file existing yet
 * (mid-install), a command file can exist without its executable bit set, a command can be
 * executable yet still fail when actually run (wrong ABI, crashing launcher, etc.), and a probe
 * that ran out of time is a distinct, actionable state from one that ran and failed outright.
 */
enum class ReadinessStage {
    MISSING,
    INSTALLED_UNRESOLVED,
    RESOLVED_NOT_EXECUTABLE,
    EXECUTABLE_INCOMPATIBLE,
    PROBE_TIMEOUT,
    READY
}

data class RuntimeProfileReport(
    val profile: RuntimeProfile,
    val missingPackages: List<String>,
    val missingCommands: List<String>,
    val incompatibleCommands: List<String>,
    val nonExecutableCommands: List<String> = emptyList(),
    val unverifiedCommands: List<String> = emptyList(),
    val timedOutCommands: List<String> = emptyList()
) {
    val isReady: Boolean
        get() = missingPackages.isEmpty() && missingCommands.isEmpty() &&
            incompatibleCommands.isEmpty() && nonExecutableCommands.isEmpty() &&
            unverifiedCommands.isEmpty() && timedOutCommands.isEmpty()

    /**
     * True when no amount of installing can make this profile ready.
     *
     * A version constraint is violated by the version that is *already installed*, and the only way
     * out would be a downgrade -- which Verb does not perform, and which would break every other
     * profile depending on the newer version. Distinguishing this from an ordinary "not installed
     * yet" state is the difference between a button that will work and a button that never can:
     * Hermes requires Python below 3.14 while the package repository ships only 3.14, so offering
     * an install action for it is an invitation to fail.
     *
     * Deliberately derived rather than stored, so it cannot drift from the report it describes.
     */
    val isUnsatisfiable: Boolean get() = incompatibleCommands.isNotEmpty()

    /** True when the profile is not ready but installing could still resolve it. */
    val isInstallable: Boolean get() = !isReady && !isUnsatisfiable

    fun stageFor(requirement: RuntimeRequirement): ReadinessStage = when {
        requirement.packageName.isNotEmpty() && missingPackages.contains(requirement.packageName) ->
            ReadinessStage.MISSING
        requirement.command in missingCommands -> ReadinessStage.INSTALLED_UNRESOLVED
        requirement.command in nonExecutableCommands -> ReadinessStage.RESOLVED_NOT_EXECUTABLE
        requirement.command in timedOutCommands -> ReadinessStage.PROBE_TIMEOUT
        requirement.command in incompatibleCommands || requirement.command in unverifiedCommands ->
            ReadinessStage.EXECUTABLE_INCOMPATIBLE
        else -> ReadinessStage.READY
    }
}

object RuntimeProfiles {
    val all: List<RuntimeProfile> = listOf(
        RuntimeProfile(
            RuntimeProfileId.CORE,
            "Core CLI",
            listOf("ca-certificates", "curl", "git", "tar"),
            listOf(
                RuntimeRequirement("bash", "bash"),
                RuntimeRequirement("apt-get", "apt"),
                RuntimeRequirement("curl", "curl"),
                RuntimeRequirement("git", "git")
            )
        ),
        RuntimeProfile(
            RuntimeProfileId.TUR,
            "Extra package repository",
            listOf("tur-repo"),
            listOf(RuntimeRequirement("tur-repo", "tur-repo")),
            // The repository is only useful once its index has been fetched, and its signing key
            // ships inside the package itself -- so the update must follow the install, in the same
            // command, or every later resolve still sees the old package list.
            installCommandOverride =
                "apt-get update && apt-get install -y --no-install-recommends tur-repo && apt-get update"
        ),
        RuntimeProfile(
            RuntimeProfileId.PYTHON,
            "Python",
            listOf("python"),
            listOf(RuntimeRequirement("python", "python"))
        ),
        RuntimeProfile(
            RuntimeProfileId.HERMES,
            "Hermes Agent",
            // Hermes needs an interpreter below 3.14, and the main repository ships only 3.14. The
            // fix is a compatible package rather than a permanent refusal: the Termux User
            // Repository carries versioned interpreters, so Hermes targets python3.13 explicitly
            // instead of the unversioned `python` whose only candidate it can never use.
            listOf("python3.13"),
            listOf(RuntimeRequirement("python3.13", "python3.13")),
            prerequisiteProfiles = listOf(RuntimeProfileId.TUR),
            postInstallHint = "Hermes runs on python3.13; the default `python` stays at 3.14."
        ),
        RuntimeProfile(
            RuntimeProfileId.JAVASCRIPT,
            "JavaScript",
            listOf("nodejs-lts", "npm"),
            listOf(
                RuntimeRequirement("node", "nodejs-lts"),
                RuntimeRequirement("npm", "npm")
            )
        ),
        RuntimeProfile(
            RuntimeProfileId.CODEX,
            "Codex CLI",
            emptyList(),
            listOf(RuntimeRequirement("codex", "", versionProbeArgs = listOf("--version"))),
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT),
            installCommandOverride = "npm install -g @openai/codex",
            postInstallHint = "In Terminal, run codex and complete its sign-in flow."
        ),
        RuntimeProfile(
            RuntimeProfileId.CLAUDE_CODE,
            "Claude Code",
            emptyList(),
            listOf(RuntimeRequirement("claude", "", versionProbeArgs = listOf("--version"))),
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT),
            installCommandOverride = "npm install -g @anthropic-ai/claude-code",
            // Verified on-device: the vendor postinstall reports "Native binaries for
            // linux-arm64-android are not available on this release channel" and lists only
            // darwin/linux/win32 targets. The npm package installs, but the launcher has no binary
            // to run, so this profile cannot become ready in the phone-native userland -- it needs
            // the Debian-based Agent Runtime, where the platform is linux-arm64.
            postInstallHint = "Needs the Linux Agent Runtime: no android-arm64 build is published."
        ),
        RuntimeProfile(
            RuntimeProfileId.GEMINI_CLI,
            "Gemini CLI",
            emptyList(),
            listOf(RuntimeRequirement("gemini", "", versionProbeArgs = listOf("--version"))),
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT),
            installCommandOverride = "npm install -g @google/gemini-cli",
            postInstallHint = "In Terminal, run gemini and complete its sign-in flow."
        ),
        RuntimeProfile(
            RuntimeProfileId.OPENCODE,
            "OpenCode",
            emptyList(),
            listOf(RuntimeRequirement("opencode", "", versionProbeArgs = listOf("--version"))),
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT),
            installCommandOverride = "npm install -g opencode-ai",
            postInstallHint = "In Terminal, run opencode and complete its sign-in flow."
        ),
        RuntimeProfile(
            RuntimeProfileId.NATIVE,
            "Native Development",
            listOf("clang", "make", "cmake", "pkg-config", "rust"),
            listOf(
                RuntimeRequirement("clang", "clang"),
                RuntimeRequirement("make", "make"),
                RuntimeRequirement("cmake", "cmake"),
                RuntimeRequirement("pkg-config", "pkg-config"),
                RuntimeRequirement("cargo", "rust")
            )
        ),
        RuntimeProfile(
            RuntimeProfileId.REMOTE,
            "Remote Development",
            listOf("openssh"),
            listOf(RuntimeRequirement("ssh", "openssh"))
        ),
        RuntimeProfile(
            RuntimeProfileId.DATA_MEDIA,
            "Data and Media",
            listOf("jq", "ripgrep", "ffmpeg", "sqlite"),
            listOf(
                RuntimeRequirement("jq", "jq"),
                RuntimeRequirement("rg", "ripgrep"),
                RuntimeRequirement("ffmpeg", "ffmpeg"),
                RuntimeRequirement("sqlite3", "sqlite")
            )
        )
    )

    fun forId(id: RuntimeProfileId): RuntimeProfile = all.first { it.id == id }

    /**
     * The profiles that must be installed, in order, to make [id] ready -- prerequisites first,
     * [id] last.
     *
     * Verb used to detect a missing prerequisite and refuse, telling the user to go and install it
     * themselves. It already knew the dependency graph, so that was busywork handed back to the
     * person: asking for Codex CLI means asking for whatever Codex needs. This resolves the graph
     * instead.
     *
     * Depth-first with a visited set, so a diamond dependency appears once and a cycle in the
     * catalog terminates rather than recursing forever. [isReady] profiles are skipped, so a plan
     * contains only real work. A profile that is [RuntimeProfileReport.isUnsatisfiable] is still
     * included -- the caller must refuse the whole plan rather than silently install part of it and
     * leave the user with a half-provisioned runtime.
     */
    fun installPlan(
        id: RuntimeProfileId,
        isReady: (RuntimeProfileId) -> Boolean
    ): List<RuntimeProfile> {
        val ordered = mutableListOf<RuntimeProfile>()
        val visited = mutableSetOf<RuntimeProfileId>()

        fun visit(current: RuntimeProfileId) {
            if (!visited.add(current)) return
            val profile = forId(current)
            profile.prerequisiteProfiles.forEach(::visit)
            if (!isReady(current)) ordered += profile
        }

        visit(id)
        return ordered
    }
}

/**
 * Reports readiness for the catalog's [RuntimeProfile]s. Presence/executable-bit checks on
 * apt-tracked commands stay host-side file stats (cheap, no process spawn, and dpkg's own Version
 * field is already a reliable compatibility signal for them). Commands that declare a
 * [RuntimeRequirement.versionProbeArgs] -- currently the npm-installed agent CLIs, which apt never
 * tracks -- are instead verified with a real, bounded probe run inside the same proot guest
 * environment the terminal itself uses, via [GuestCommandRunner]. That is what makes "Ready" mean
 * "resolves in the guest PATH and `<tool> --version` exits 0 inside the guest", not "a file exists
 * on the host's view of the guest filesystem".
 */
class RuntimeCapabilityDetector(
    private val filesDir: File,
    private val guestCommandRunner: GuestCommandRunner = GuestCommandRunner(filesDir)
) {
    private val prefixDir = File(filesDir, "usr")
    private val statusFile = File(prefixDir, "var/lib/dpkg/status")

    fun inspect(profile: RuntimeProfile): RuntimeProfileReport {
        val packages = installedPackages()
        val missingPackages = profile.packages.filterNot(packages::contains)

        val (probedRequirements, fileCheckedRequirements) = profile.requirements.partition { it.versionProbeArgs != null }

        val resolved = fileCheckedRequirements.filter { commandFile(it.command).isFile }
        val missingFileCommands = (fileCheckedRequirements - resolved.toSet()).map { it.command }
        val executable = resolved.filter { commandFile(it.command).canExecute() }
        val nonExecutableFileCommands = (resolved - executable.toSet()).map { it.command }
        val incompatibleCommands = fileCheckedRequirements.mapNotNull { requirement ->
            val version = packages[requirement.packageName] ?: return@mapNotNull null
            val maximum = requirement.maxVersionExclusive ?: return@mapNotNull null
            if (compareVersions(version, maximum) >= 0) requirement.command else null
        }

        val missingProbeCommands = mutableListOf<String>()
        val nonExecutableProbeCommands = mutableListOf<String>()
        val unverifiedCommands = mutableListOf<String>()
        val timedOutCommands = mutableListOf<String>()
        for (requirement in probedRequirements) {
            when (guestCommandRunner.probe(requirement).outcome) {
                GuestCommandRunner.Outcome.READY -> Unit
                GuestCommandRunner.Outcome.TIMEOUT -> timedOutCommands += requirement.command
                GuestCommandRunner.Outcome.GUEST_UNAVAILABLE,
                GuestCommandRunner.Outcome.NOT_FOUND -> missingProbeCommands += requirement.command
                GuestCommandRunner.Outcome.NOT_EXECUTABLE -> nonExecutableProbeCommands += requirement.command
                GuestCommandRunner.Outcome.NONZERO_EXIT,
                GuestCommandRunner.Outcome.LAUNCH_FAILED,
                GuestCommandRunner.Outcome.REFUSED -> unverifiedCommands += requirement.command
            }
        }

        return RuntimeProfileReport(
            profile = profile,
            missingPackages = missingPackages,
            missingCommands = missingFileCommands + missingProbeCommands,
            incompatibleCommands = incompatibleCommands,
            nonExecutableCommands = nonExecutableFileCommands + nonExecutableProbeCommands,
            unverifiedCommands = unverifiedCommands,
            timedOutCommands = timedOutCommands
        )
    }

    private fun commandFile(command: String): File = File(prefixDir, "bin/$command")

    private fun installedPackages(): Map<String, String> {
        if (!statusFile.isFile) return emptyMap()
        val result = mutableMapOf<String, String>()
        statusFile.readText().split("\n\n").forEach { stanza ->
            val fields = stanza.lines().associate { line ->
                val separator = line.indexOf(':')
                if (separator <= 0) "" to "" else line.substring(0, separator) to line.substring(separator + 1).trim()
            }
            if (fields["Status"] == "install ok installed") {
                fields["Package"]?.let { name -> fields["Version"]?.let { version -> result[name] = version } }
            }
        }
        return result
    }

    private fun compareVersions(left: String, right: String): Int {
        val leftParts = numericVersionParts(left)
        val rightParts = numericVersionParts(right)
        for (index in 0 until maxOf(leftParts.size, rightParts.size)) {
            val leftPart = leftParts.getOrElse(index) { 0 }
            val rightPart = rightParts.getOrElse(index) { 0 }
            if (leftPart != rightPart) return leftPart.compareTo(rightPart)
        }
        return 0
    }

    private fun numericVersionParts(version: String): List<Int> =
        version.takeWhile { it.isDigit() || it == '.' }
            .split('.')
            .filter { it.isNotEmpty() }
            .map { it.toIntOrNull() ?: 0 }
}
