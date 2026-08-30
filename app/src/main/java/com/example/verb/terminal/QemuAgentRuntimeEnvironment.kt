package com.example.verb.terminal

import java.io.File

/**
 * Runs the Debian agent rootfs by pairing two tools, each doing only what this sandbox permits.
 *
 * PRoot alone cannot run this rootfs: launching a glibc binary from the app process exits 255,
 * silently, while the identical invocation succeeds outside the sandbox. QEMU alone can execute
 * those binaries, but `qemu-user` does not chroot -- it only redirects the ELF interpreter -- so the
 * guest sees Android's filesystem, where `/etc/resolv.conf` and `/etc/ssl/certs` do not exist. The
 * glibc resolver then falls back to `127.0.0.1:53` and every lookup fails with `ECONNREFUSED`.
 *
 * Combining them solves both halves:
 *
 * ```
 * PRoot        supplies the filesystem view: /etc/resolv.conf, the CA bundle, /workspace, /home/verb
 *   -> QEMU    supplies execution: emulates the aarch64 glibc binaries
 *     -> agent
 * ```
 *
 * The ordering matters and is not interchangeable with PRoot's own `-q` option. PRoot's first exec
 * is QEMU, which is a Bionic binary Android executes natively and therefore permits; QEMU then
 * emulates everything after it. Nothing ever asks the kernel to execute a glibc binary directly,
 * which is the operation that fails here.
 *
 * This is one layer, not nested PRoot: a single PRoot process providing paths, with QEMU as the
 * program it launches.
 */
class QemuAgentRuntimeEnvironment(
    private val filesDir: File,
    private val projectDirectory: File,
    private val manifest: AgentRuntimeManifest
) {

    /** The interactive session: a login shell in the selected project, mounted at [GUEST_WORKSPACE]. */
    fun resolve(rootfs: File): TerminalEnvironment =
        resolveGuestCommand(rootfs, listOf("/bin/bash", "--login"))

    /**
     * Builds a bounded, non-interactive invocation of [guestCommand] against the same rootfs, binds
     * and guest environment the interactive session uses, so a compatibility probe cannot drift from
     * the session it claims to be testing. [guestCommand] is always a literal argv list supplied by
     * Verb itself; nothing here accepts user or AI input and nothing passes through a shell.
     */
    fun resolveGuestCommand(rootfs: File, guestCommand: List<String>): TerminalEnvironment {
        require(manifest.validateForArm64().isSuccess) { "Invalid agent runtime manifest." }
        require(rootfs.isDirectory) { "Agent runtime rootfs is unavailable." }
        require(projectDirectory.isDirectory) { "Selected project directory is unavailable." }

        val proot = File(filesDir, "usr/bin/proot")
        require(proot.isFile && proot.canExecute()) { "Verb PRoot is unavailable." }
        val qemu = File(filesDir, QEMU_RELATIVE_PATH)
        require(qemu.isFile && qemu.canExecute()) {
            "The emulator this runtime needs is not installed. Install the Agent Emulator profile."
        }
        val agentHome = AgentRuntimePaths(filesDir).agentHome("default")
        require(agentHome.mkdirs() || agentHome.isDirectory) { "Could not create Agent Runtime home." }
        val agentBin = File(agentHome, ".local/bin")
        if (!agentBin.isDirectory) agentBin.mkdirs()
        val agentTmp = File(agentHome, ".tmp")
        if (!agentTmp.isDirectory) agentTmp.mkdirs()

        val hostTmp = File(filesDir, "usr/tmp")
        if (!hostTmp.isDirectory) hostTmp.mkdirs()

        // Go and glibc networking require localhost resolution via /etc/hosts; external DNS will return NXDOMAIN.
        val hostsFile = File(rootfs, "etc/hosts")
        if (!hostsFile.exists() || hostsFile.length() == 0L) {
            runCatching {
                hostsFile.parentFile?.mkdirs()
                hostsFile.writeText("127.0.0.1 localhost\n::1 localhost ip6-localhost ip6-loopback\n")
            }
        }

        val geminiSettings = File(agentHome, ".gemini/antigravity-cli/settings.json")
        if (!geminiSettings.exists()) {
            runCatching {
                geminiSettings.parentFile?.mkdirs()
                geminiSettings.writeText("{\n  \"modelProvider\": \"gemini\"\n}\n")
            }
        }

        val appDir = filesDir.parentFile ?: filesDir
        val args = mutableListOf(proot.absolutePath, "-r", rootfs.absolutePath)

        for (path in listOf("/dev", "/proc", "/sys", "/system", "/apex", "/vendor")) {
            if (File(path).isDirectory) args += listOf("-b", path)
        }

        // QEMU is a Bionic executable, so its own loader configuration must resolve inside the
        // guest. Verb keeps an app-local copy because Android 12+ denies access to the real
        // /linkerconfig, which would otherwise make PRoot's path canonicalisation fail.
        val guestLinkerConfig = File(filesDir, "linkerconfig")
        if (File(guestLinkerConfig, "ld.config.txt").isFile) {
            args += listOf("-b", "${guestLinkerConfig.absolutePath}:/linkerconfig")
        }

        // The app directory is bound onto ITSELF. QEMU is launched by its absolute host path, so
        // that exact path has to resolve inside the guest too -- otherwise PRoot reports
        // `execve(...): No such file or directory` and the runtime never starts.
        args += listOf("-b", "${appDir.absolutePath}:${appDir.absolutePath}")

        // Without this bind the guest has no resolver at all: qemu-user does not chroot, so
        // /etc/resolv.conf would resolve against Android's read-only /etc, where it does not exist.
        val resolver = File(filesDir, "usr/etc/resolv.conf")
        if (resolver.isFile) args += listOf("-b", "${resolver.absolutePath}:/etc/resolv.conf")

        args += listOf(
            "-b", "${projectDirectory.absolutePath}:$GUEST_WORKSPACE",
            "-b", "${agentHome.absolutePath}:$GUEST_HOME",
            "-w", GUEST_WORKSPACE
        )

        args += qemu.absolutePath
        args += listOf(
            // The guest root is already PRoot's, so QEMU's own prefix is simply "/".
            "-L", "/",
            "-E", "LD_LIBRARY_PATH=/usr/lib/aarch64-linux-gnu:/lib/aarch64-linux-gnu",
            "-E", "HOME=$GUEST_HOME",
            "-E", "TMPDIR=$GUEST_HOME/.tmp",
            "-E", "PATH=$GUEST_HOME/.local/bin:/usr/local/bin:/usr/bin:/bin",
            "-E", "LANG=C.UTF-8",
            "-E", "TERM=xterm-256color",
            "-E", "SHELL=/bin/bash",
            // The agent launchers are Bun-compiled binaries; their JavaScriptCore JIT crashes under
            // emulation. Node's V8 needs the equivalent, which is a command-line flag rather than an
            // environment variable, so callers that launch Node add `--jitless` themselves.
            // Verb's own Bionic loader settings must not reach the glibc guest.
            "-U", "LD_PRELOAD"
        )
        // QEMU user-mode does not perform a PATH search for the binary argument.
        // Wrapping bare command names with /usr/bin/env ensures PATH resolution inside the guest.
        val resolvedCommand = if (guestCommand.firstOrNull()?.startsWith("/") == true) {
            guestCommand
        } else {
            listOf("/usr/bin/env") + guestCommand
        }
        args += resolvedCommand

        return TerminalEnvironment(
            kind = TerminalEnvironment.Kind.VERB_AGENT_LINUX_USERLAND,
            shellExecutable = proot.absolutePath,
            arguments = args.toTypedArray(),
            workingDirectory = projectDirectory,
            // The environment of the PRoot process itself: host paths, so the dynamically linked
            // PRoot and QEMU executables can find their own Bionic libraries.
            variables = arrayOf(
                "TERM=xterm-256color",
                "COLORTERM=truecolor",
                "HOME=${filesDir.absolutePath}/home",
                "PATH=${filesDir.absolutePath}/usr/bin:${filesDir.absolutePath}/usr/bin/applets",
                "LD_LIBRARY_PATH=${filesDir.absolutePath}/usr/lib",
                "TMPDIR=${filesDir.absolutePath}/usr/tmp",
                "PROOT_TMP_DIR=${filesDir.absolutePath}/usr/tmp",
                // Emulating a CPU the guest binaries were built for; the default model lacks
                // instructions they use.
                "QEMU_CPU=$QEMU_CPU_MODEL"
            ),
            rootfsDir = rootfs,
            prefixDir = File(rootfs, "usr")
        )
    }

    companion object {
        const val GUEST_WORKSPACE = "/workspace"
        const val GUEST_HOME = "/home/verb"
        const val QEMU_CPU_MODEL = "cortex-a76"
        const val QEMU_RELATIVE_PATH = "usr/bin/qemu-aarch64"

        /** True when the emulator this backend depends on is installed. */
        fun isEmulatorInstalled(filesDir: File): Boolean =
            File(filesDir, QEMU_RELATIVE_PATH).let { it.isFile && it.canExecute() }
    }
}
