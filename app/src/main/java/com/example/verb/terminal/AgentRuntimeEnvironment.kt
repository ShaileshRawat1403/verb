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
    fun resolve(rootfs: File): TerminalEnvironment {
        require(manifest.validateForArm64().isSuccess) { "Invalid agent runtime manifest." }
        require(rootfs.isDirectory) { "Agent runtime rootfs is unavailable." }
        require(projectDirectory.isDirectory) { "Selected project directory is unavailable." }

        val proot = File(filesDir, "usr/bin/proot")
        require(proot.isFile && proot.canExecute()) { "Verb PRoot is unavailable." }

        val args = mutableListOf(proot.absolutePath, "--link2symlink", "-r", rootfs.absolutePath)
        listOf("/dev", "/proc", "/sys", "/system", "/apex", "/vendor").forEach { path ->
            if (File(path).isDirectory) args += listOf("-b", path)
        }
        args += listOf(
            "-b", "${projectDirectory.absolutePath}:/workspace",
            "-w", "/workspace",
            "/usr/bin/env",
            "HOME=/home/verb",
            "PATH=/usr/local/bin:/usr/bin:/bin",
            "TERM=xterm-256color",
            "LANG=C.UTF-8",
            "SHELL=/bin/bash",
            "/bin/bash", "--login"
        )

        return TerminalEnvironment(
            kind = TerminalEnvironment.Kind.VERB_AGENT_LINUX_USERLAND,
            shellExecutable = proot.absolutePath,
            arguments = args.toTypedArray(),
            workingDirectory = projectDirectory,
            variables = arrayOf(
                "TERM=xterm-256color",
                "COLORTERM=truecolor",
                "PROOT_TMP_DIR=${filesDir.absolutePath}/usr/tmp"
            ),
            rootfsDir = rootfs,
            prefixDir = File(rootfs, "usr")
        )
    }
}
