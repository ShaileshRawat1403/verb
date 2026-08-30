package com.example.verb.terminal

import java.io.File

/**
 * Builds the direct Android -> PRoot -> Linux-rootfs invocation.
 *
 * The Linux guest is a sibling backend to the normal Verb userland; it is never launched from
 * inside that userland.
 */
class AgentRuntimeEnvironment(
    private val filesDir: File,
    private val projectDirectory: File,
    private val manifest: AgentRuntimeManifest
) {
    /** The interactive session: a login shell in the selected project, mounted at `/workspace`. */
    fun resolve(rootfs: File): TerminalEnvironment =
        resolveGuestCommand(rootfs, listOf("/bin/bash", "--login"))

    /**
     * Builds a single bounded, non-interactive invocation of [guestCommand] against the same rootfs,
     * binds, working directory, guest environment and loader isolation the interactive session uses.
     *
     * Shared with [resolve] so a compatibility probe can never drift from the environment it claims
     * to be testing -- a probe that ran under different binds would prove nothing about the session
     * the user is about to open. [guestCommand] is supplied only by Verb's own callers as a literal
     * argv list; nothing here accepts user or AI input, and nothing is passed through a shell.
     */
    fun resolveGuestCommand(rootfs: File, guestCommand: List<String>): TerminalEnvironment {
        require(manifest.validateForArm64().isSuccess) { "Invalid agent runtime manifest." }
        require(rootfs.isDirectory) { "Agent runtime rootfs is unavailable." }
        require(projectDirectory.isDirectory) { "Selected project directory is unavailable." }

        val proot = File(filesDir, "usr/bin/proot")
        require(proot.isFile && proot.canExecute()) { "Verb PRoot is unavailable." }
        val agentHome = AgentRuntimePaths(filesDir).agentHome("default")
        require(agentHome.mkdirs() || agentHome.isDirectory) { "Could not create Agent Runtime home." }

        val args = mutableListOf(proot.absolutePath, "--link2symlink", "-r", rootfs.absolutePath)
        listOf("/dev", "/proc", "/sys", "/system", "/apex", "/vendor").forEach { path ->
            if (File(path).isDirectory) args += listOf("-b", path)
        }
        args += listOf(
            "-b", "${projectDirectory.absolutePath}:/workspace",
            "-b", "${agentHome.absolutePath}:/home/verb",
            "-w", "/workspace",
            "/usr/bin/env",
            "HOME=/home/verb",
            "PATH=/home/verb/.local/bin:/usr/local/bin:/usr/bin:/bin",
            "TERM=xterm-256color",
            "LANG=C.UTF-8",
            "SHELL=/bin/bash",
            // PRoot itself needs Verb's Bionic loader path, but the Debian processes it starts
            // must not inherit those Android/Termux loader overrides.
            "LD_LIBRARY_PATH=",
            "LD_PRELOAD="
        )
        args += guestCommand

        return TerminalEnvironment(
            kind = TerminalEnvironment.Kind.VERB_AGENT_LINUX_USERLAND,
            shellExecutable = proot.absolutePath,
            arguments = args.toTypedArray(),
            workingDirectory = projectDirectory,
            variables = arrayOf(
                "TERM=xterm-256color",
                "COLORTERM=truecolor",
                "HOME=${filesDir.absolutePath}/home",
                "PATH=${filesDir.absolutePath}/usr/bin:${filesDir.absolutePath}/usr/bin/applets",
                "LD_LIBRARY_PATH=${filesDir.absolutePath}/usr/lib",
                "TMPDIR=${filesDir.absolutePath}/usr/tmp",
                "PROOT_TMP_DIR=${filesDir.absolutePath}/usr/tmp"
            ),
            rootfsDir = rootfs,
            prefixDir = File(rootfs, "usr")
        )
    }
}
