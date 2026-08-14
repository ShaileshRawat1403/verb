package com.example.verb.terminal

import java.io.File

/**
 * Describes the process environment passed to the local terminal PTY.
 *
 * Verb deliberately distinguishes Android's small system shell from a complete, locally
 * provisioned userland. The latter is only selected after a Termux-compatible bootstrap has been
 * installed under [rootfsDir] and is launched through the Termux-built [proot] binary (plus its
 * libtalloc/libandroid-shmem dependencies) so the stock Termux userland (whose paths are baked
 * for /data/data/com.termux) can run under Verb's different application id.
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
        VERB_LOCAL_USERLAND
    }

    val displayName: String
        get() = when (kind) {
            Kind.ANDROID_SYSTEM_SHELL -> "Android system shell (limited)"
            Kind.VERB_LOCAL_USERLAND -> "Verb CLI userland (proot)"
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
 * Paths as seen from inside the proot guest. Verb binds its private files directory onto
 * /data/data/com.termux so the stock Termux userland believes it is running under com.termux.
 */
object TermuxGuestPaths {
    const val FILES = "/data/data/com.termux/files"
    const val PREFIX = "/data/data/com.termux/files/usr"
    const val HOME = "/data/data/com.termux/files/home"
    const val CACHE = "/data/data/com.termux/cache"
    const val VAR = "/data/data/com.termux/var"
}

/**
 * Resolves the only two supported terminal process environments.
 *
 * The complete userland is a stock Termux bootstrap archive run under the Termux-built proot
 * binary. Verb's files directory is bound onto /data/data/com.termux inside the guest, so the
 * prebuilt Termux binaries, apt configuration and termux-exec preload all resolve. proot runs as
 * the app's own uid (never -0): apt refuses to run as root, exactly as it does on a real device.
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
        val login = File(prefixDir, "bin/login")
        val execShim = File(prefixDir, "lib/libtermux-exec.so")
        val proot = File(prefixDir, "bin/proot")

        val projectDir = projectDirectory?.takeIf(::isProjectDirectory)
        return if (login.isFile && login.canExecute() && proot.isFile && proot.canExecute() && execShim.isFile) {
            val rootfs = appFilesDir.absolutePath
            TerminalEnvironment(
                kind = TerminalEnvironment.Kind.VERB_LOCAL_USERLAND,
                shellExecutable = proot.absolutePath,
                arguments = prootArguments(rootfs, projectDir),
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
     * proot is executed with the guest filesystem bound to this app's private directory. The
     * first element is argv[0] (the executable itself) because the terminal JNI execs the
     * argument array verbatim via execvp(). PRoot canonicalizes the guest path component by
     * component on the HOST side, so the guest /data/data/com.termux (the real Termux app's
     * private directory, which this app cannot stat) is bound over this app's own directory to
     * make the parent of the rootfs bind resolvable. The /data/data/com.termux/files bind then
     * maps the Termux userland onto this app's files directory. The Android system directories
     * (/system, /apex, /linkerconfig, ...) are bound so guest dynamic binaries can find the
     * platform linker and bionic libraries. The final path is the `env` wrapper (which sets the
     * guest environment) followed by the Termux login script, which starts a bash login shell.
     */
    private fun prootArguments(rootfs: String, projectDir: File?): Array<String> {
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
        // Static musl tools such as Codex read /etc/resolv.conf, while the Termux userland keeps
        // its resolver under $PREFIX/etc. Bind the synced file into the conventional path too.
        val guestResolver = File(rootfs, "usr/etc/resolv.conf")
        if (guestResolver.isFile) {
            args += "-b"
            args += "${guestResolver.absolutePath}:/etc/resolv.conf"
        }
        val appDir = File(rootfs).parent ?: rootfs
        args += listOf(
            "-b", "$appDir:/data/data/com.termux",
            "-b", "$rootfs:${TermuxGuestPaths.FILES}",
            "-b", "$rootfs/cache:${TermuxGuestPaths.CACHE}",
            "-w", projectDir?.let(::guestProjectPath) ?: TermuxGuestPaths.HOME
        )
        args += listOf(
            "${TermuxGuestPaths.FILES}/usr/bin/env",
            "HOME=${TermuxGuestPaths.HOME}",
            "PREFIX=${TermuxGuestPaths.PREFIX}",
            // Codex installs its launcher in ~/.local/bin. Keep that directory visible in the
            // already-running shell so `codex` works immediately after the installer finishes.
            "PATH=${TermuxGuestPaths.HOME}/.local/bin:${TermuxGuestPaths.PREFIX}/bin:${TermuxGuestPaths.PREFIX}/bin/applets",
            "LD_LIBRARY_PATH=${TermuxGuestPaths.PREFIX}/lib",
            "LD_PRELOAD=${TermuxGuestPaths.PREFIX}/lib/libtermux-exec-ld-preload.so",
            "TMPDIR=${TermuxGuestPaths.PREFIX}/tmp",
            "CURL_CA_BUNDLE=${TermuxGuestPaths.PREFIX}/etc/tls/cert.pem",
            "SSL_CERT_FILE=${TermuxGuestPaths.PREFIX}/etc/tls/cert.pem",
            "LANG=C.UTF-8",
            "SHELL=${TermuxGuestPaths.PREFIX}/bin/bash",
            "TERM=xterm-256color",
            "${TermuxGuestPaths.FILES}/usr/bin/login"
        )
        return args.toTypedArray()
    }

    /**
     * Environment passed to the proot PROCESS itself (not the guest). These are HOST paths so the
     * dynamically linked proot binary can find libtalloc/libandroid-shmem under its own files
     * directory. LD_PRELOAD is intentionally absent here — a preloaded libtermux-exec would hook
     * proot's own execve handling. The guest login shell receives its Termux-style environment
     * through the `env` wrapper command in [prootArguments].
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
        "${TermuxGuestPaths.FILES}/projects/${directory.name}"

    private companion object {
        val TOOLS = listOf("busybox", "curl", "jq")
    }
}
