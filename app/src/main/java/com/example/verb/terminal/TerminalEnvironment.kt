package com.example.verb.terminal

import java.io.File

/**
 * Describes the process environment passed to the local terminal PTY.
 *
 * Verb deliberately distinguishes Android's small system shell from a complete, locally
 * provisioned userland. The latter is only selected after a bootstrap built specifically for
 * Verb's Android application id has been installed under [rootfsDir].
 */
data class TerminalEnvironment(
    val kind: Kind,
    val shellExecutable: String,
    val workingDirectory: File,
    val variables: Array<String>,
    val rootfsDir: File,
    val prefixDir: File? = null
) {
    enum class Kind {
        /** Android's built-in shell; useful for diagnostics but not a CLI development runtime. */
        ANDROID_SYSTEM_SHELL,

        /** A Verb-specific Termux-compatible bootstrap is present and ready to run. */
        VERB_LOCAL_USERLAND
    }

    val displayName: String
        get() = when (kind) {
            Kind.ANDROID_SYSTEM_SHELL -> "Android system shell (limited)"
            Kind.VERB_LOCAL_USERLAND -> "Verb local CLI userland"
        }
}

/**
 * Resolves the only two supported terminal process environments.
 *
 * Stock Termux archives are intentionally not accepted. Their ELF interpreter and package paths
 * are compiled for com.termux, whereas Verb is installed by Android under a different private
 * directory. Running them here would fail at load time or, worse, misrepresent a broken runtime
 * as a working one.
 */
class TerminalEnvironmentResolver(
    private val appFilesDir: File,
    private val systemPath: String = System.getenv("PATH") ?: "/system/bin:/system/xbin"
) {
    fun resolve(): TerminalEnvironment {
        val prefixDir = File(appFilesDir, "usr")
        val homeDir = File(appFilesDir, "home")
        val login = File(prefixDir, "bin/login")
        val execShim = File(prefixDir, "lib/libtermux-exec.so")

        return if (login.isFile && login.canExecute() && execShim.isFile) {
            TerminalEnvironment(
                kind = TerminalEnvironment.Kind.VERB_LOCAL_USERLAND,
                shellExecutable = login.absolutePath,
                workingDirectory = homeDir,
                variables = arrayOf(
                    "TERM=xterm-256color",
                    "COLORTERM=truecolor",
                    "HOME=${homeDir.absolutePath}",
                    "PREFIX=${prefixDir.absolutePath}",
                    "TERMUX__ROOTFS=${appFilesDir.absolutePath}",
                    "TERMUX__PREFIX=${prefixDir.absolutePath}",
                    "PATH=${prefixDir.absolutePath}/bin",
                    "TMPDIR=${prefixDir.absolutePath}/tmp",
                    "LANG=C.UTF-8"
                ),
                rootfsDir = appFilesDir,
                prefixDir = prefixDir
            )
        } else {
            TerminalEnvironment(
                kind = TerminalEnvironment.Kind.ANDROID_SYSTEM_SHELL,
                shellExecutable = "/system/bin/sh",
                workingDirectory = appFilesDir,
                variables = arrayOf(
                    "TERM=xterm-256color",
                    "COLORTERM=truecolor",
                    "HOME=${appFilesDir.absolutePath}",
                    "PATH=$systemPath",
                    "LANG=C.UTF-8"
                ),
                rootfsDir = appFilesDir
            )
        }
    }
}
