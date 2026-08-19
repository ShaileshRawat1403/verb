package com.example.verb.terminal

import java.io.File

/**
 * Describes the process environment passed to the local terminal PTY.
 *
 * Verb deliberately distinguishes Android's small system shell from a complete, locally
 * provisioned userland. The latter is only selected after a Termux-compatible bootstrap has been
 * installed under [rootfsDir] and is launched through the Termux-built [proot] binary (plus its
 * libtalloc/libandroid-shmem dependencies). The bootstrap itself (built by
 * runtime/termux-packages/verb-app-id.patch) has its paths baked for Verb's own application id,
 * `com.aistudio.verb.app` -- not Termux's. That is Verb's real storage location and the only
 * identity ever exposed to the user (HOME, PREFIX, PATH, prompt, diagnostics). A hidden internal
 * compatibility mount for `com.termux` is still bound alongside it: `runtime/README.md`
 * ("Package-management boundary") documents that packages installed via `apt`/`pkg` from the
 * official Termux repository are unmodified upstream Termux builds whose binaries/scripts are
 * path-bound to `com.termux`, and Verb only rebuilds the small bootstrap core -- not the whole
 * repository. See [TerminalEnvironmentResolver] for the exact bind.
 */
data class TerminalEnvironment(
    val kind: Kind,
    val shellExecutable: String,
    val arguments: Array<String>,
    val workingDirectory: File,
    val variables: Array<String>,
    val rootfsDir: File,
    val prefixDir: File? = null
) {
    enum class Kind {
        /** Android's built-in shell; useful for diagnostics but not a CLI development runtime. */
        ANDROID_SYSTEM_SHELL,

        /** A Termux-compatible bootstrap (proot + login) is present and ready to run. */
        VERB_LOCAL_USERLAND,

        /** A separate Linux rootfs launched directly by Android through the existing PRoot binary. */
        VERB_AGENT_LINUX_USERLAND
    }

    val displayName: String
        get() = when (kind) {
            Kind.ANDROID_SYSTEM_SHELL -> "Android system shell (limited)"
            Kind.VERB_LOCAL_USERLAND -> "Verb CLI userland (proot)"
            Kind.VERB_AGENT_LINUX_USERLAND -> "Verb Agent Linux runtime (proot)"
        }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is TerminalEnvironment) return false
        if (kind != other.kind) return false
        if (shellExecutable != other.shellExecutable) return false
        if (!arguments.contentEquals(other.arguments)) return false
        if (workingDirectory != other.workingDirectory) return false
        if (!variables.contentEquals(other.variables)) return false
        if (rootfsDir != other.rootfsDir) return false
        if (prefixDir != other.prefixDir) return false
        return true
    }

    override fun hashCode(): Int {
        var result = kind.hashCode()
        result = 31 * result + shellExecutable.hashCode()
        result = 31 * result + arguments.contentHashCode()
        result = 31 * result + workingDirectory.hashCode()
        result = 31 * result + variables.contentHashCode()
        result = 31 * result + rootfsDir.hashCode()
        result = 31 * result + (prefixDir?.hashCode() ?: 0)
        return result
    }
}

/**
 * Paths as seen from inside the proot guest, under Verb's own application id. The release
 * bootstrap core (bash, apt, dpkg, coreutils, git, tar) is built (via
 * runtime/termux-packages/verb-app-id.patch) with every path baked for `com.aistudio.verb.app`;
 * CI (build-verb-terminal-bootstrap.yml) asserts the shipped `bin/login` contains that path.
 * Verb's own `fullCli` application id (see app/build.gradle.kts) is exactly `com.aistudio.verb.app`.
 *
 * This is the ONLY identity ever exposed to the user: HOME, PREFIX, PATH, the shell prompt, and
 * every diagnostic string are built from these constants. Never format a user-facing string with
 * `/data/data/com.termux` -- see [TerminalEnvironmentResolver] for why a *hidden*, internal-only
 * bind for that legacy path still exists alongside this one.
 */
object VerbGuestPaths {
    const val FILES = "/data/data/com.aistudio.verb.app/files"
    const val PREFIX = "/data/data/com.aistudio.verb.app/files/usr"
    const val HOME = "/data/data/com.aistudio.verb.app/files/home"
    const val CACHE = "/data/data/com.aistudio.verb.app/cache"
    const val VAR = "/data/data/com.aistudio.verb.app/var"
}

/**
 * Internal-only compatibility mount, never surfaced in any user-facing HOME/PREFIX/PATH/prompt/
 * diagnostic string. Packages installed via `apt`/`pkg` from the official signed Termux
 * repository (`runtime/README.md`, "Package-management boundary") are unmodified upstream Termux
 * builds compiled for `com.termux`; only the small bootstrap core Verb rebuilds itself
 * (runtime/termux-packages/verb-app-id.patch) targets `com.aistudio.verb.app`. Binding the same
 * real storage onto this legacy guest path too, in addition to [VerbGuestPaths], lets those
 * packages' hardcoded interpreter/library paths keep resolving without exposing the identity.
 *
 * Retained until proven unnecessary on a physical device (do not remove without that evidence):
 * removing it would silently break every apt-installed package beyond the bootstrap core (clang,
 * node/npm fetched live, jq, ripgrep, ffmpeg, sqlite, openssh, ...).
 */
private const val LEGACY_TERMUX_GUEST_ROOT = "/data/data/com.termux"

/**
 * Resolves the only two supported terminal process environments.
 *
 * The complete userland is a stock Termux bootstrap archive run under the Termux-built proot
 * binary, built (and CI-verified) to expect Verb's own application id. Verb's files directory is
 * bound onto [VerbGuestPaths.FILES] inside the guest so the prebuilt binaries, apt configuration
 * and termux-exec preload all resolve their own baked-in paths. proot runs as the app's own uid
 * (never -0): apt refuses to run as root, exactly as it does on a real device.
 */
class TerminalEnvironmentResolver(
    private val appFilesDir: File,
    private val systemPath: String = System.getenv("PATH") ?: "/system/bin:/system/xbin",
    private val bundledBinDir: File? = null,
    private val projectDirectory: File? = null
) {
    fun resolve(): TerminalEnvironment {
        val prefixDir = File(appFilesDir, "usr")
        val homeDir = File(appFilesDir, "home")

        val projectDir = projectDirectory?.takeIf(::isProjectDirectory)
        return if (hasCompleteBootstrap(prefixDir)) {
            val rootfs = appFilesDir.absolutePath
            TerminalEnvironment(
                kind = TerminalEnvironment.Kind.VERB_LOCAL_USERLAND,
                shellExecutable = File(prefixDir, "bin/proot").absolutePath,
                arguments = guestExecArguments(rootfs, projectDir, listOf("${VerbGuestPaths.FILES}/usr/bin/login")),
                workingDirectory = projectDir ?: homeDir,
                variables = termuxUserlandVariables(rootfs, prefixDir),
                rootfsDir = appFilesDir,
                prefixDir = prefixDir
            )
        } else {
            TerminalEnvironment(
                kind = TerminalEnvironment.Kind.ANDROID_SYSTEM_SHELL,
                shellExecutable = "/system/bin/sh",
                arguments = arrayOf("-l"),
                workingDirectory = projectDir ?: appFilesDir,
                variables = androidShellVariables(),
                rootfsDir = appFilesDir
            )
        }
    }

    /**
     * Builds a single, bounded, non-interactive proot invocation for [guestCommand] instead of the
     * interactive login shell -- same binds, same HOME/PREFIX/PATH/LD_LIBRARY_PATH/certs, same
     * working-directory resolution as [resolve]. Used by GuestCommandRunner so readiness probes run
     * inside the exact environment the user's terminal itself uses, never a host-side approximation
     * of it. Returns null when the guest userland (proot/login/libtermux-exec) is not installed, so
     * there is nothing to probe against.
     */
    fun resolveGuestCommand(guestCommand: List<String>): TerminalEnvironment? {
        val prefixDir = File(appFilesDir, "usr")
        if (!hasCompleteBootstrap(prefixDir)) return null
        val rootfs = appFilesDir.absolutePath
        val projectDir = projectDirectory?.takeIf(::isProjectDirectory)
        return TerminalEnvironment(
            kind = TerminalEnvironment.Kind.VERB_LOCAL_USERLAND,
            shellExecutable = File(prefixDir, "bin/proot").absolutePath,
            arguments = guestExecArguments(rootfs, projectDir, guestCommand),
            workingDirectory = projectDir ?: File(appFilesDir, "home"),
            variables = termuxUserlandVariables(rootfs, prefixDir),
            rootfsDir = appFilesDir,
            prefixDir = prefixDir
        )
    }

    /** True when proot, login, and the termux-exec shim are all present and executable/readable. */
    private fun hasCompleteBootstrap(prefixDir: File): Boolean {
        val login = File(prefixDir, "bin/login")
        val execShim = File(prefixDir, "lib/libtermux-exec.so")
        val proot = File(prefixDir, "bin/proot")
        return login.isFile && login.canExecute() && proot.isFile && proot.canExecute() && execShim.isFile
    }

    /**
     * proot is executed with the guest filesystem bound to this app's private directory. The
     * first element is argv[0] (the executable itself) because the terminal JNI execs the
     * argument array verbatim via execvp(). PRoot canonicalizes the guest path component by
     * component on the HOST side, and the bootstrap's binaries have Verb's own application id
     * baked into their absolute paths at build time (runtime/termux-packages/verb-app-id.patch).
     * The literal string can still differ from the host's actual mount point (e.g. /data/data is a
     * symlink to /data/user/0 on some Android versions), so the app's own directory is bound over
     * the guest's canonical [VerbGuestPaths] parent to make that path resolve regardless of which
     * host mount alias is real on a given device. The Android system directories (/system, /apex,
     * /linkerconfig, ...) are bound so guest dynamic binaries can find the platform linker and
     * bionic libraries. The final path is the `env` wrapper (which sets the guest environment)
     * followed by [guestCommand] -- the interactive login shell for [resolve], or a single bounded
     * probe command for [resolveGuestCommand]. Both callers share this one construction so neither
     * can drift from the real terminal's mounts or environment.
     */
    private fun guestExecArguments(rootfs: String, projectDir: File?, guestCommand: List<String>): Array<String> {
        val args = mutableListOf(File(File(rootfs, "usr"), "bin/proot").absolutePath, "--link2symlink")
        for (dir in listOf("/dev", "/proc", "/sys", "/system", "/apex", "/vendor", "/odm", "/product", "/system_ext")) {
            if (File(dir).isDirectory) {
                args += "-b"
                args += dir
            }
        }
        // Bind an app-local copy of the linker configuration over the guest /linkerconfig. Android
        // 12+ denies lstat() on the real /linkerconfig directory (EACCES), which makes proot's path
        // canonicalization fail and the guest linker spam "WARNING: linker: failed to find generated
        // linker configuration" on every command. The copy (installed by TermuxBootstrapInstaller)
        // lives under filesDir, which the app can stat freely. Falls back to the host dir so older
        // installs still resolve their config (with the harmless warnings).
        val guestLinkerConfig = File(rootfs, "linkerconfig")
        if (File(guestLinkerConfig, "ld.config.txt").isFile) {
            args += "-b"
            args += "${guestLinkerConfig.absolutePath}:/linkerconfig"
        } else {
            val linkerConfig = File("/linkerconfig")
            if (linkerConfig.isDirectory) {
                args += "-b"
                args += linkerConfig.absolutePath
            }
        }
        // Agent CLIs published as musl builds (Claude Code, OpenCode, anything Bun-compiled) are
        // aarch64 ELFs whose only unmet dependency is their interpreter. Binding the bundled loader
        // at the absolute path they hard-code is what lets them exec at all; without it the kernel
        // refuses with ENOENT and reports "No such file or directory" for a file that exists.
        val muslLoader = File(rootfs, MuslLoaderBootstrap.RELATIVE_INSTALL_PATH)
        if (muslLoader.isFile) {
            args += "-b"
            args += "${muslLoader.absolutePath}:${MuslLoaderBootstrap.GUEST_LOADER_PATH}"
        }
        // A writable /tmp. Agent CLIs create scratch state there unconditionally; OpenCode aborts
        // with `EROFS: read-only file system, mkdir '/tmp'` without it, because Android's real /tmp
        // does not exist and the fallback is read-only.
        val guestTmp = File(rootfs, "usr/tmp")
        if (guestTmp.isDirectory || guestTmp.mkdirs()) {
            args += "-b"
            args += "${guestTmp.absolutePath}:/tmp"
        }
        // Static musl tools such as Codex read /etc/resolv.conf, while the Termux userland keeps
        // its resolver under $PREFIX/etc. Bind the synced file into the conventional path too.
        val guestResolver = File(rootfs, "usr/etc/resolv.conf")
        if (guestResolver.isFile) {
            args += "-b"
            args += "${guestResolver.absolutePath}:/etc/resolv.conf"
        }
        val appDir = File(rootfs).parent ?: rootfs
        args += listOf(
            "-b", "$appDir:/data/data/com.aistudio.verb.app",
            "-b", "$rootfs:${VerbGuestPaths.FILES}",
            "-b", "$rootfs/cache:${VerbGuestPaths.CACHE}",
            // Hidden internal compatibility alias -- never exposed via HOME/PREFIX/PATH/prompt/
            // diagnostics below, which all use VerbGuestPaths exclusively. See the
            // LEGACY_TERMUX_GUEST_ROOT KDoc above for why this must stay until proven unnecessary
            // on a physical device: it is what lets `apt`-installed packages from the official
            // Termux repository (baked for com.termux) keep resolving their own paths.
            "-b", "$appDir:$LEGACY_TERMUX_GUEST_ROOT",
            "-b", "$rootfs:$LEGACY_TERMUX_GUEST_ROOT/files",
            "-b", "$rootfs/cache:$LEGACY_TERMUX_GUEST_ROOT/cache",
            "-w", projectDir?.let(::guestProjectPath) ?: VerbGuestPaths.HOME
        )
        args += listOf(
            "${VerbGuestPaths.FILES}/usr/bin/env",
            "HOME=${VerbGuestPaths.HOME}",
            "PREFIX=${VerbGuestPaths.PREFIX}",
            // $HOME/.local/bin is a generic, tool-agnostic convention (not owned by any single
            // CLI) that many installer scripts use. Keeping it on the base PATH means a tool
            // installed there works immediately without a session restart. Installers that use a
            // different directory rely on their PATH edit to $HOME/.bashrc instead, which
            // TermuxBootstrapInstaller.ensureLoginShellSourcesBashrc makes login shells actually
            // read -- see that function for why a fresh session (not this already-running one) is
            // the supported point at which such a change becomes visible.
            "PATH=${VerbGuestPaths.HOME}/.local/bin:${VerbGuestPaths.PREFIX}/bin:${VerbGuestPaths.PREFIX}/bin/applets",
            "LD_LIBRARY_PATH=${VerbGuestPaths.PREFIX}/lib",
            "LD_PRELOAD=${VerbGuestPaths.PREFIX}/lib/libtermux-exec-ld-preload.so",
            "TMPDIR=${VerbGuestPaths.PREFIX}/tmp",
            "CURL_CA_BUNDLE=${VerbGuestPaths.PREFIX}/etc/tls/cert.pem",
            "SSL_CERT_FILE=${VerbGuestPaths.PREFIX}/etc/tls/cert.pem",
            "LANG=C.UTF-8",
            "SHELL=${VerbGuestPaths.PREFIX}/bin/bash",
            "TERM=xterm-256color"
        )
        args += guestCommand
        return args.toTypedArray()
    }

    /**
     * Environment passed to the proot PROCESS itself (not the guest). These are HOST paths so the
     * dynamically linked proot binary can find libtalloc/libandroid-shmem under its own files
     * directory. LD_PRELOAD is intentionally absent here — a preloaded libtermux-exec would hook
     * proot's own execve handling. The guest login shell receives its Termux-style environment
     * through the `env` wrapper command in [guestExecArguments].
     */
    private fun termuxUserlandVariables(rootfs: String, prefixDir: File): Array<String> = arrayOf(
        "TERM=xterm-256color",
        "COLORTERM=truecolor",
        "HOME=$rootfs/home",
        "PREFIX=$rootfs/usr",
        "PATH=$rootfs/usr/bin:$rootfs/usr/bin/applets",
        "LD_LIBRARY_PATH=$rootfs/usr/lib",
        "TMPDIR=$rootfs/usr/tmp",
        "LANG=C.UTF-8",
        "PROOT_TMP_DIR=$rootfs/usr/tmp",
        "TERMUX__ROOTFS=$rootfs",
        "TERMUX__PREFIX=${prefixDir.absolutePath}",
        "TERMUX_APP__PACKAGE_NAME=com.aistudio.verb.app",
        "TERMUX_APP__PACKAGE_VARIANT=apt-android-7",
        "TERMUX_MAIN_PACKAGE_FORMAT=debian",
        "TERMUX_VERSION=0.1.0"
    )

    /**
     * Builds the environment for Android's system shell. When a validated bundled tool directory
     * is present, its path is prepended to PATH (so busybox/curl/jq resolve first) and
     * CURL_CA_BUNDLE points at the bundled certificate store. Otherwise the plain system
     * environment is used unchanged.
     */
    private fun androidShellVariables(): Array<String> {
        val baseVars = mutableListOf(
            "TERM=xterm-256color",
            "COLORTERM=truecolor",
            "HOME=${appFilesDir.absolutePath}",
            "PATH=$systemPath",
            "LANG=C.UTF-8"
        )
        val binDir = bundledBinDir?.takeIf { it.isDirectory }
        if (binDir != null && hasBundledTool(binDir)) {
            baseVars[baseVars.indexOfFirst { it.startsWith("PATH=") }] = "PATH=${binDir.absolutePath}:$systemPath"
            baseVars += "CURL_CA_BUNDLE=${binDir.absolutePath}/cacert.pem"
            baseVars += "TMPDIR=${binDir.absolutePath}/.tmp"
        }
        return baseVars.toTypedArray()
    }

    private fun hasBundledTool(binDir: File): Boolean =
        TOOLS.any { tool -> File(binDir, tool).let { it.isFile && BundledToolBootstrap.isValidElf(it) } }

    private fun isProjectDirectory(directory: File): Boolean = runCatching {
        val root = File(appFilesDir, "projects").canonicalFile
        directory.canonicalFile.parentFile == root && directory.isDirectory
    }.getOrDefault(false)

    private fun guestProjectPath(directory: File): String =
        "${VerbGuestPaths.FILES}/projects/${directory.name}"

    private companion object {
        val TOOLS = listOf("busybox", "curl", "jq")
    }
}
