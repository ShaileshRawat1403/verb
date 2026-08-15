package com.example.verb.terminal

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/**
 * Exercises GuestCommandRunner's execution mechanics: bounded timeout, bounded output, exit-code
 * interpretation, and refusal of unregistered probes. Since a real proot binary cannot run inside
 * a JVM unit test, these use a small POSIX `sh` test double standing in for proot: real proot's
 * trailing argv is `env VAR=val... TARGET ARGS...`, and this double ignores proot's own bind/flag
 * arguments (which have no meaning without a real mount namespace) and directly execs the last two
 * arguments -- exactly the shape every catalog probe uses (`<command> --version`). This still
 * exercises the real code path: GuestCommandRunner builds the argv/env exactly as
 * TerminalEnvironmentResolver.resolveGuestCommand does (covered separately, by content assertions,
 * in TerminalEnvironmentResolverTest), and this suite verifies what happens once that argv actually
 * runs.
 */
class GuestCommandRunnerTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun setUpGuestBootstrap(filesDir: File) {
        File(filesDir, "usr/bin/login").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(filesDir, "usr/lib/libtermux-exec.so").apply { parentFile?.mkdirs(); createNewFile() }
        File(filesDir, "home").mkdirs()
        File(filesDir, "usr/bin/proot").apply {
            parentFile?.mkdirs()
            writeText("#!/bin/sh\nwhile [ \"\$#\" -gt 2 ]; do shift; done\nexec \"\$1\" \"\$2\"\n")
            setExecutable(true)
        }
    }

    private fun writeGuestTool(filesDir: File, name: String, script: String) {
        File(filesDir, "usr/bin/$name").apply {
            parentFile?.mkdirs()
            writeText(script)
            setExecutable(true)
        }
    }

    @Test
    fun `shell-script probe succeeds when the guest binary exits 0`() {
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        writeGuestTool(filesDir, "shell-tool", "#!/bin/sh\necho \"shell-tool v1.0.0\"\nexit 0\n")

        val result = GuestCommandRunner(filesDir)
            .probe(RuntimeRequirement("shell-tool", "", versionProbeArgs = listOf("--version")))

        assertEquals(GuestCommandRunner.Outcome.READY, result.outcome)
        assertEquals(0, result.exitCode)
        assertTrue(result.output.contains("shell-tool"))
    }

    @Test
    fun `native binary probe succeeds`() {
        // Stand-in for a compiled ELF guest binary (e.g. git): a real, pre-compiled native
        // executable, not an interpreted script. Compiling a real cross-platform ELF fixture isn't
        // practical inside a JVM unit test, so the host's own `true` binary plays that role here --
        // what matters for this test is that GuestCommandRunner treats it identically to a script.
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        val trueBinary = listOf("/usr/bin/true", "/bin/true").map(::File).firstOrNull { it.isFile }
        Assume.assumeTrue("no native 'true' binary found on this machine", trueBinary != null)
        val target = File(filesDir, "usr/bin/native-tool").apply { parentFile?.mkdirs() }
        trueBinary!!.copyTo(target, overwrite = true)
        target.setExecutable(true)

        val result = GuestCommandRunner(filesDir)
            .probe(RuntimeRequirement("native-tool", "", versionProbeArgs = listOf("--version")))

        assertEquals(GuestCommandRunner.Outcome.READY, result.outcome)
        assertEquals(0, result.exitCode)
    }

    @Test
    fun `env-node-shebang probe succeeds when node is available`() {
        val nodeDir = System.getenv("PATH").orEmpty()
            .split(File.pathSeparatorChar)
            .map { File(it) }
            .firstOrNull { File(it, "node").isFile }
        Assume.assumeTrue("node not available on PATH in this environment", nodeDir != null)

        val filesDir = temporaryFolder.newFolder("files")
        File(filesDir, "home").mkdirs()
        val proot = File(filesDir, "proot-double.sh").apply {
            writeText("#!/bin/sh\nwhile [ \"\$#\" -gt 2 ]; do shift; done\nexec \"\$1\" \"\$2\"\n")
            setExecutable(true)
        }
        val toolDir = File(filesDir, "bin").apply { mkdirs() }
        File(toolDir, "node-tool").apply {
            writeText("#!/usr/bin/env node\nconsole.log('node-tool v1.0.0');\n")
            setExecutable(true)
        }

        // Hand-built environment (bypassing TerminalEnvironmentResolver, which is covered
        // separately) so the PATH can include the real host node directory alongside the tool --
        // the resolver's own argv/env construction is asserted independently in
        // TerminalEnvironmentResolverTest.
        val environment = TerminalEnvironment(
            kind = TerminalEnvironment.Kind.VERB_LOCAL_USERLAND,
            shellExecutable = proot.absolutePath,
            arguments = arrayOf(proot.absolutePath, "node-tool", "--version"),
            workingDirectory = File(filesDir, "home"),
            variables = arrayOf("PATH=${toolDir.absolutePath}${File.pathSeparator}${nodeDir!!.absolutePath}"),
            rootfsDir = filesDir
        )

        val result = GuestCommandRunner(filesDir).execute(environment, 3_000)

        assertEquals(GuestCommandRunner.Outcome.READY, result.outcome)
        assertTrue(result.output.contains("node-tool"))
    }

    @Test
    fun `nonzero exit is reported as NONZERO_EXIT, not ready`() {
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        writeGuestTool(filesDir, "broken-tool", "#!/bin/sh\necho boom\nexit 1\n")

        val result = GuestCommandRunner(filesDir)
            .probe(RuntimeRequirement("broken-tool", "", versionProbeArgs = listOf("--version")))

        assertEquals(GuestCommandRunner.Outcome.NONZERO_EXIT, result.outcome)
        assertEquals(1, result.exitCode)
    }

    @Test
    fun `command not found via guest PATH is reported as NOT_FOUND`() {
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        // No usr/bin/missing-tool is written -- the fake proot's `exec` fails to resolve it via
        // PATH and sh reports exit 127, matching POSIX env's own not-found convention.

        val result = GuestCommandRunner(filesDir)
            .probe(RuntimeRequirement("missing-tool", "", versionProbeArgs = listOf("--version")))

        assertEquals(GuestCommandRunner.Outcome.NOT_FOUND, result.outcome)
    }

    @Test
    fun `a command found on guest PATH but not executable is reported as NOT_EXECUTABLE, distinct from NOT_FOUND`() {
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        // Present on the guest PATH (unlike the NOT_FOUND case above), but lacking the executable
        // bit -- POSIX shells (including the fake proot double's `exec`) report this as exit 126,
        // distinct from 127 for "not found at all".
        File(filesDir, "usr/bin/unexecutable-tool").apply {
            parentFile?.mkdirs()
            writeText("#!/bin/sh\necho should-not-run\nexit 0\n")
            setExecutable(false)
        }

        val result = GuestCommandRunner(filesDir)
            .probe(RuntimeRequirement("unexecutable-tool", "", versionProbeArgs = listOf("--version")))

        assertEquals(GuestCommandRunner.Outcome.NOT_EXECUTABLE, result.outcome)
        assertEquals(126, result.exitCode)
    }

    @Test
    fun `a hanging binary times out instead of blocking`() {
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        // Blocks on a stdin read that never arrives (the probe never writes to or closes the
        // child's stdin), so this hangs deterministically regardless of `sleep` support.
        writeGuestTool(filesDir, "hanging-tool", "#!/bin/sh\nread _unused\necho done\n")

        val result = GuestCommandRunner(filesDir)
            .probe(RuntimeRequirement("hanging-tool", "", versionProbeArgs = listOf("--version")), timeoutMs = 300)

        assertEquals(GuestCommandRunner.Outcome.TIMEOUT, result.outcome)
        assertEquals(null, result.exitCode)
    }

    @Test
    fun `timeout is always clamped to the 3 second bound even if a caller asks for longer`() {
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        writeGuestTool(filesDir, "hanging-tool", "#!/bin/sh\nread _unused\necho done\n")

        val started = System.nanoTime()
        val result = GuestCommandRunner(filesDir).probe(
            RuntimeRequirement("hanging-tool", "", versionProbeArgs = listOf("--version")),
            timeoutMs = 60_000
        )
        val elapsedMs = (System.nanoTime() - started) / 1_000_000

        assertEquals(GuestCommandRunner.Outcome.TIMEOUT, result.outcome)
        assertTrue("probe ran for ${elapsedMs}ms, expected it clamped near the 3s bound", elapsedMs < 5_000)
    }

    @Test
    fun `output is bounded even when the guest binary is chatty`() {
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        writeGuestTool(
            filesDir,
            "chatty-tool",
            "#!/bin/sh\ni=0\nwhile [ \$i -lt 20000 ]; do printf 'x'; i=\$((i + 1)); done\nexit 0\n"
        )

        val result = GuestCommandRunner(filesDir)
            .probe(RuntimeRequirement("chatty-tool", "", versionProbeArgs = listOf("--version")))

        assertEquals(GuestCommandRunner.Outcome.READY, result.outcome)
        assertTrue(result.output.length <= GuestCommandRunner.MAX_OUTPUT_CHARS + "...[truncated]".length)
        assertTrue(result.output.contains("...[truncated]"))
    }

    @Test
    fun `a requirement with no registered probe is refused before anything runs`() {
        val filesDir = temporaryFolder.newFolder("files")
        setUpGuestBootstrap(filesDir)
        // No versionProbeArgs -- e.g. an apt-tracked requirement that was never meant to be probed.
        val unregistered = RuntimeRequirement("git", "git")

        val result = GuestCommandRunner(filesDir).probe(unregistered)

        assertEquals(GuestCommandRunner.Outcome.REFUSED, result.outcome)
        assertEquals(null, result.exitCode)
    }

    @Test
    fun `probing without an installed guest userland reports GUEST_UNAVAILABLE`() {
        val filesDir = temporaryFolder.newFolder("files")
        // No login/proot/execShim written at all.

        val result = GuestCommandRunner(filesDir)
            .probe(RuntimeRequirement("git", "", versionProbeArgs = listOf("--version")))

        assertEquals(GuestCommandRunner.Outcome.GUEST_UNAVAILABLE, result.outcome)
    }
}
