package com.example.verb.terminal

import java.io.File

enum class RuntimeProfileId {
    CORE,
    /** The Termux User Repository, which carries versioned toolchains the main repo does not. */
    TUR,
    /** qemu-user, which executes the Linux agent rootfs's glibc binaries. */
    AGENT_EMULATOR,
    PYTHON,
    HERMES,
    JAVASCRIPT,
    CODEX,
    CLAUDE_CODE,
    GEMINI_CLI,
    OPENCODE,
    ANTIGRAVITY,
    DEEPSEEK_HARNESS,
    NATIVE,
    REMOTE,
    DATA_MEDIA
}

/** Execution target where an agent profile is installed, probed and executed. */
enum class ProfileEnvironment {
    LOCAL_USERLAND,
    AGENT_RUNTIME
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
    val versionProbeArgs: List<String>? = null,

    /**
     * How long this command's probe may take, when the default is not enough.
     *
     * Codex's build is static and runs through `qemu-aarch64`, whose startup on a 300 MB binary
     * exceeds the default bound often enough that the probe timed out and the Agents tab reported an
     * installed, working Codex as not installed. The bound is per-command rather than global so one
     * slow emulated agent cannot make every other probe slower.
     */
    val probeTimeoutMs: Long? = null
)

/**
 * How a resolved agent binary has to be handed to the kernel.
 *
 * The rule this started from -- **a binary runs when its ELF interpreter exists** -- turned out to
 * be only half of it. A dynamically linked musl build runs once launched through the bundled musl
 * loader ([MuslLoaderBootstrap]). A *static* build has no interpreter to supply, and proot refuses
 * it outright, so it needs `qemu-aarch64` instead. Scripts and Bionic builds exec as they are.
 */
enum class AgentBinaryAbi {
    /** Launch through the bundled musl loader, with the musl C++ runtime on LD_LIBRARY_PATH. */
    MUSL,

    /** Exec directly: a Bionic build or an ordinary script. */
    NATIVE,

    /**
     * A statically linked, non-PIE (`ET_EXEC`) binary, launched through `qemu-aarch64`.
     *
     * These run perfectly on the device -- executing one from Android's own shell works -- but
     * **proot rejects them**: `error: "<path>" has unexpected e_type: 2`. Verb's own `proot` binary
     * is itself such a build, and running `proot --version` *inside* proot reproduces the same
     * refusal, which is how the message was finally attributed. Since a terminal session is always
     * inside proot and nothing can escape a ptrace-based sandbox from within, the binary cannot
     * simply be exec'd.
     *
     * `qemu-aarch64` maps the ELF itself instead of handing it to proot's loader, so the rejection
     * never happens. Codex ships exactly this kind of build and is why the branch exists.
     */
    STATIC,

    /**
     * Read the interpreter out of the file at launch time and pick the branch it asks for.
     *
     * Used for anything Verb did not install itself -- a vendor self-installer's binary, or a
     * launcher a package manager wrote -- where the ABI is not knowable from the catalog and
     * guessing it is exactly the mistake this sprint kept making.
     */
    DETECT
}

/**
 * One place a launchable agent's real binary might be, and how to exec it once found.
 *
 * [path] is a guest-absolute path in which the literal tokens `$PREFIX` and `$HOME` are expanded at
 * wrapper-generation time, and which may end in a `*` glob -- the newest match wins, so a vendor
 * self-update lands automatically instead of leaving the wrapper pointing at a deleted version.
 */
data class AgentBinaryCandidate(
    val path: String,
    val abi: AgentBinaryAbi
)

data class RuntimeProfile(
    val id: RuntimeProfileId,
    val displayName: String,
    val packages: List<String>,
    val requirements: List<RuntimeRequirement>,
    val prerequisiteProfiles: List<RuntimeProfileId> = emptyList(),
    val installCommandOverride: String? = null,
    val postInstallHint: String? = null,
    /**
     * The command a user types to start this profile, when it is something you *use* rather than
     * something you install once and forget.
     *
     * Separating the two is what lets the UI stop presenting a toolchain and an agent as equal
     * rows in one long list: Core CLI and Python are setup, `claude` and `codex` are the product.
     * Null means the profile is plumbing and has nothing to launch.
     */
    val launchCommand: String? = null,

    /**
     * Arguments Verb adds to [launchCommand] when it opens this agent.
     *
     * Kept separate from [launchCommand] because that name is also the filename of the Verb-owned
     * wrapper [AgentWrapperBootstrap] writes onto PATH; it has to stay a bare command. These are
     * appended only at the moment the agent is launched, and they are shown on the agent's card
     * along with the command, so what Verb runs is never more than what the user can read.
     */
    val launchArguments: List<String> = emptyList(),

    /**
     * Why this profile can never be installed on Verb's Android userland, when that is known ahead
     * of any probe. Null means "no such obstacle known".
     *
     * The other route to [RuntimeProfileReport.isUnsatisfiable] is discovered at probe time, from
     * an already-installed binary whose version or ABI rules it out. This one is for the case a
     * probe can never reach: an install that fails the same way every time for a reason outside
     * Verb. Recording it as a sentence the user can read is the difference between "Install" that
     * always fails and a card that says what is actually wrong.
     */
    val unavailableReason: String? = null,

    /**
     * Where [launchCommand]'s real binary may live, in preference order, and how to exec each.
     *
     * Consulted by [AgentWrapperBootstrap], which turns this into a Verb-owned wrapper that wins
     * PATH. Empty means "wherever a package manager put it", which
     * [AgentWrapperBootstrap.candidatesFor] covers with a default list -- so an agent still gets a
     * wrapper that survives a vendor installer even when the catalog knows nothing specific.
     */
    val binaryCandidates: List<AgentBinaryCandidate> = emptyList(),

    /**
     * Paths, relative to the guest `$HOME`, whose **existence** means this agent has an
     * authenticated session. Checked by [AgentSignInDetector], which never opens them.
     *
     * Empty means Verb does not know where this agent keeps credentials, and it will say so rather
     * than report a sign-in state it cannot support. Add an entry only once the file has been seen
     * on a real device -- a guessed path reporting "signed out" is worse than admitting ignorance.
     */
    val signedInMarkers: List<String> = emptyList(),
    val environment: ProfileEnvironment = ProfileEnvironment.LOCAL_USERLAND
) {
    /** True when this profile is something to open, not merely something to install. */
    val isAgent: Boolean get() = launchCommand != null

    /**
     * Exactly the line Verb types into the terminal to open this agent -- command plus
     * [launchArguments]. Null for profiles that are plumbing rather than product.
     */
    val launchLine: String?
        get() = launchCommand?.let { command ->
            (listOf(command) + launchArguments).joinToString(" ")
        }

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
     * yet" state is the difference between a button that will work and a button that never can.
     *
     * Deliberately derived rather than stored, so it cannot drift from the report it describes.
     */
    val isUnsatisfiable: Boolean
        get() = incompatibleCommands.isNotEmpty() || profile.unavailableReason != null

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

    /**
     * Install command for an agent CLI published only as a musl build.
     *
     * npm refuses musl packages on Android. Bionic reports no libc, so npm sees
     * `Actual libc: undefined` against `Valid libc: musl` and calls a package unsupported that is in
     * fact the correct architecture. `--force` is the documented override.
     *
     * That is now the *whole* install. This function used to also write a launcher into
     * `$PREFIX/bin`, and that launcher is precisely what kept disappearing: npm overwrote
     * `$PREFIX/bin/claude` with a symlink to a Windows launcher when the wrapper package installed,
     * and Claude's own self-installer added `$HOME/.local/bin/claude`, which won PATH and failed
     * with `has unexpected e_type: 2`. A file written once, into a directory other installers own,
     * cannot survive them. Launching is therefore no longer an install-time artifact at all --
     * [AgentWrapperBootstrap] owns it, in a directory nothing else writes to, regenerated every
     * launch.
     */
    private fun muslAgentInstall(platformPackage: String): String =
        "npm install -g --force $platformPackage"

    /** Where a musl agent's npm platform package unpacks its binary. */
    private fun nodeModulesBinary(platformPackage: String, binaryRelativePath: String): String =
        "\$PREFIX/lib/node_modules/$platformPackage/$binaryRelativePath"

    /**
     * The target triple Codex names its aarch64 build after, read from the published tarball
     * (`package/vendor/aarch64-unknown-linux-musl/bin/codex`) rather than assumed.
     */
    private const val CODEX_TARGET_TRIPLE = "aarch64-unknown-linux-musl"

    /**
     * Codex's feature flag for the account-side app connectors. Shared with `CodexAgentAdapter` so
     * a resumed session is launched exactly like a new one.
     */
    const val CODEX_APPS_FEATURE = "apps"

    /**
     * Install command for Codex, which needs its platform build fetched by hand.
     *
     * `@openai/codex` is a launcher whose real binary lives in an *optional* dependency, declared as
     * an alias (`"@openai/codex-linux-arm64": "npm:@openai/codex@0.147.0-linux-arm64"`). npm skips
     * it here, and the launcher then stops with
     * `Missing optional dependency @openai/codex-linux-arm64`. Verb reporting that as "cannot
     * launch" would be giving up on a dependency it can plainly resolve.
     *
     * So it is resolved. `codex.js` falls back to `<package>/vendor/<triple>/bin/codex` when the
     * optional package is absent (read from `codex.js`, not guessed), so unpacking the published
     * platform tarball there is enough -- no npm platform check to fight, and the version is taken
     * from the launcher actually installed, so the two can never drift apart.
     *
     * `npm pack` is used rather than a hand-built registry URL so npm resolves the tarball location
     * itself.
     */
    private fun codexInstall(): String {
        val pkg = "\$PREFIX/lib/node_modules/@openai/codex"
        val work = "\$TMPDIR/verb-codex-platform"
        return "npm install -g @openai/codex && " +
            "codex_version=\$(node -p \"require('$pkg/package.json').version\") && " +
            "rm -rf $work && mkdir -p $work && " +
            "(cd $work && npm pack --silent \"@openai/codex@${'$'}{codex_version}-linux-arm64\") && " +
            "mkdir -p $pkg/vendor && " +
            "tar -xzf $work/*.tgz -C $pkg/vendor --strip-components=2 package/vendor && " +
            "chmod -R +x $pkg/vendor/$CODEX_TARGET_TRIPLE/bin && " +
            "rm -rf $work"
    }

    /**
     * Install command for a Python agent running in an isolated virtual environment.
     *
     * The agent gets its own virtualenv under `$HOME/.venvs/$venvName` with `--system-site-packages`
     * to access prebuilt system binary modules (such as `python-cryptography` and `python-psutil`),
     * while compiling native Rust/C extensions directly on ARM64 Android with appropriate temporary
     * directory and compiler flags.
     *
     * Launching remains [AgentWrapperBootstrap]'s responsibility. It owns a stable directory ahead
     * of package-manager paths and resolves this venv's declared console script at execution time.
     * The install must never write to `$PREFIX/bin`: that directory belongs to the package manager
     * and can contain unrelated user commands.
     */
    private fun pythonAgentInstall(
        interpreter: String,
        venvName: String,
        pipSpec: String,
        extraPipFlags: String = ""
    ): String {
        val venv = "\$HOME/.venvs/$venvName"
        val flags = if (extraPipFlags.isNotBlank()) " $extraPipFlags" else ""
        return "$interpreter -m venv --system-site-packages $venv && " +
            "TMPDIR=\$PREFIX/tmp TEMP=\$PREFIX/tmp TMP=\$PREFIX/tmp " +
            "SSL_CERT_FILE=\$PREFIX/etc/tls/cert.pem CARGO_HTTP_CAINFO=\$PREFIX/etc/tls/cert.pem " +
            "CARGO_BUILD_TARGET=aarch64-linux-android ANDROID_API_LEVEL=24 " +
            "CC=clang CXX=clang++ CC_aarch64_linux_android=clang CXX_aarch64_linux_android=clang++ " +
            "AR=llvm-ar CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=clang " +
            "RUSTFLAGS=\"-C link-arg=-landroid-support\" " +
            "$venv/bin/pip install --upgrade$flags $pipSpec"
    }

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
            // tur-repo writes a new sources list; refreshing the apt cache is part of installing it,
            // otherwise the packages it introduces -- such as versioned Python interpreters -- cannot
            // be resolved until an install happens to refresh the index by accident.
            installCommandOverride =
                "apt-get update && apt-get install -y --no-install-recommends tur-repo && apt-get update"
        ),
        RuntimeProfile(
            RuntimeProfileId.AGENT_EMULATOR,
            "Agent emulator",
            listOf("qemu-user-aarch64"),
            listOf(RuntimeRequirement("qemu-aarch64", "qemu-user-aarch64")),
            postInstallHint =
                "Runs statically linked agent builds that proot refuses to exec directly, such as " +
                    "Codex's."
        ),
        RuntimeProfile(
            RuntimeProfileId.PYTHON,
            "Python",
            listOf("python", "python-pip"),
            listOf(
                RuntimeRequirement("python", "python"),
                RuntimeRequirement("pip", "python-pip")
            )
        ),
        RuntimeProfile(
            RuntimeProfileId.HERMES,
            "Hermes Agent",
            listOf("python", "python-pip", "python-cryptography", "python-psutil", "openssl", "libffi"),
            listOf(
                RuntimeRequirement("python", "python"),
                RuntimeRequirement("hermes", "", versionProbeArgs = listOf("--help"))
            ),
            prerequisiteProfiles = listOf(RuntimeProfileId.NATIVE),
            installCommandOverride =
                "apt-get update && apt-get install -y --no-install-recommends python python-pip python-cryptography python-psutil openssl libffi && " +
                    pythonAgentInstall(
                        interpreter = "python",
                        venvName = "hermes",
                        // Hermes 0.15.2 is the exact release physically proven on the validation
                        // device. Pinning keeps a fresh Verb install repeatable instead of silently
                        // accepting a future PyPI release with a different native build graph.
                        pipSpec = "hermes-agent==0.15.2"
                    ),
            postInstallHint = "Hermes Agent 0.15.2 runs in its own venv (\$HOME/.venvs/hermes) with native ARM64 cryptography.",
            launchCommand = "hermes",
            // The venv's own console script is authoritative. AgentWrapperBootstrap resolves it
            // from Verb's private libexec directory, without writing to $PREFIX/bin.
            binaryCandidates = listOf(
                AgentBinaryCandidate("\$HOME/.venvs/hermes/bin/hermes", AgentBinaryAbi.NATIVE)
            )
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
            // Emulated through qemu, so it needs longer than a natively executed binary.
            listOf(RuntimeRequirement("codex", "", versionProbeArgs = listOf("--version"), probeTimeoutMs = 12_000L)),
            // qemu is not optional here: Codex's only aarch64 build is static, and proot refuses to
            // exec a static binary. See AgentBinaryAbi.STATIC.
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT, RuntimeProfileId.AGENT_EMULATOR),
            installCommandOverride = codexInstall(),
            postInstallHint = "In Terminal, run codex and complete its sign-in flow.",
            launchCommand = "codex",
            // Codex boots the account's app connectors (`codex_apps`, and whatever the account has
            // linked) at startup. On the validation device that cost 94.7s for a one-turn prompt
            // against 57.0s with them off -- roughly 38 seconds per launch, spent before the user
            // can type. MCP servers the user configures themselves are unaffected.
            launchArguments = listOf("--disable", CODEX_APPS_FEATURE),
            // Observed on the validation device while Codex was authenticated.
            signedInMarkers = listOf(".codex/auth.json"),
            binaryCandidates = listOf(
                // The build Verb fetches itself, so its ABI is certain.
                AgentBinaryCandidate(
                    "\$PREFIX/lib/node_modules/@openai/codex/vendor/$CODEX_TARGET_TRIPLE/bin/codex",
                    AgentBinaryAbi.STATIC
                ),
                // Codex's own standalone installer keeps every release it has fetched; newest wins.
                AgentBinaryCandidate(
                    "\$HOME/.codex/packages/standalone/releases/*/bin/codex",
                    AgentBinaryAbi.STATIC
                )
            )
        ),
        RuntimeProfile(
            RuntimeProfileId.CLAUDE_CODE,
            "Claude Code",
            emptyList(),
            listOf(RuntimeRequirement("claude", "", versionProbeArgs = listOf("--version"))),
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT),
            // No android-arm64 build is published, but the musl build runs here once its
            // interpreter exists (see MuslLoaderBootstrap). Installed directly rather than through
            // the wrapper package, whose postinstall only looks for an android binary.
            installCommandOverride = muslAgentInstall("@anthropic-ai/claude-code-linux-arm64-musl"),
            postInstallHint = "In Terminal, run claude and complete its sign-in flow.",
            launchCommand = "claude",
            // Observed on the validation device while Claude Code was authenticated. Both are
            // listed because the CLI has moved this between releases.
            signedInMarkers = listOf(".claude/.credentials.json", ".claude.json"),
            // Ordered by how much Verb knows about each. The npm platform package is the build Verb
            // installed itself, so its ABI is certain. Claude Code also self-updates into
            // $HOME/.local/share/claude/versions, which is why that entry is a glob resolved newest
            // -first at launch: a self-update must not be able to leave this wrapper pointing at a
            // version that no longer exists.
            binaryCandidates = listOf(
                AgentBinaryCandidate(
                    nodeModulesBinary("@anthropic-ai/claude-code-linux-arm64-musl", "claude"),
                    AgentBinaryAbi.MUSL
                ),
                AgentBinaryCandidate("\$HOME/.local/share/claude/versions/*", AgentBinaryAbi.DETECT)
            )
        ),
        RuntimeProfile(
            RuntimeProfileId.GEMINI_CLI,
            "Gemini CLI",
            emptyList(),
            listOf(RuntimeRequirement("gemini", "", versionProbeArgs = listOf("--version"))),
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT),
            installCommandOverride = "npm install -g @google/gemini-cli",
            postInstallHint = "In Terminal, run gemini and complete its sign-in flow.",
            launchCommand = "gemini"
        ),
        RuntimeProfile(
            RuntimeProfileId.OPENCODE,
            "OpenCode",
            emptyList(),
            listOf(RuntimeRequirement("opencode", "", versionProbeArgs = listOf("--version"))),
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT),
            installCommandOverride = muslAgentInstall("opencode-linux-arm64-musl"),
            postInstallHint = "In Terminal, run opencode and complete its sign-in flow.",
            launchCommand = "opencode",
            binaryCandidates = listOf(
                AgentBinaryCandidate(
                    nodeModulesBinary("opencode-linux-arm64-musl", "bin/opencode"),
                    AgentBinaryAbi.MUSL
                )
            )
        ),
        RuntimeProfile(
            RuntimeProfileId.ANTIGRAVITY,
            "Antigravity",
            emptyList(),
            listOf(RuntimeRequirement("agy", "", versionProbeArgs = listOf("--version"), probeTimeoutMs = 15_000L)),
            installCommandOverride = "curl -fsSL https://antigravity.google/cli/install.sh | bash",
            postInstallHint = "In Terminal, run agy and complete its sign-in flow.",
            launchCommand = "agy",
            binaryCandidates = listOf(
                AgentBinaryCandidate("\$HOME/.local/bin/agy", AgentBinaryAbi.DETECT)
            ),
            environment = ProfileEnvironment.AGENT_RUNTIME
        ),
        RuntimeProfile(
            RuntimeProfileId.DEEPSEEK_HARNESS,
            "DeepSeek Harness",
            emptyList(),
            listOf(RuntimeRequirement("dsh", "", versionProbeArgs = listOf("--version"))),
            prerequisiteProfiles = listOf(RuntimeProfileId.JAVASCRIPT),
            // Pure JavaScript with no platform-specific binary, so unlike Claude Code and OpenCode
            // it needs neither a musl build nor a wrapper -- a plain global install is enough.
            installCommandOverride = "npm install -g @deepseek-ai/dsh",
            postInstallHint = "In Terminal, run dsh to boot a DeepSeek Harness profile.",
            launchCommand = "dsh",
            // Established by installing it on the validation device, not assumed. `dsh` depends on
            // the `koffi` native module, which ships no prebuilt binary for this platform and then
            // cannot be compiled here either: with cmake, clang and make installed, its build gets
            // as far as the C++ and stops at `fatal error: 'spawn.h' file not found`, because
            // Android's libc headers do not provide it. Fixing that is upstream work in koffi or in
            // the Termux headers -- no install command Verb can write will resolve it.
            unavailableReason = "Needs the koffi native module, which has no build for Android: " +
                "its source build stops at a libc header (spawn.h) this platform does not ship."
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
            val outcome = requirement.probeTimeoutMs
                ?.let { guestCommandRunner.probe(requirement, profile.environment, timeoutMs = it) }
                ?: guestCommandRunner.probe(requirement, profile.environment)
            when (outcome.outcome) {
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
