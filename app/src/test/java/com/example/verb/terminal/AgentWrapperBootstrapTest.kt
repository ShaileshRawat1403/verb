package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * Agent launching used to be an install-time artifact, and it kept being destroyed: npm overwrote
 * `$PREFIX/bin/claude` with a symlink to a Windows launcher, and Claude Code's self-installer added
 * `$HOME/.local/bin/claude`, which won PATH and failed with `has unexpected e_type: 2`. The Agents
 * tab then correctly reported an installed, authenticated agent as not installed.
 *
 * These tests pin the properties that make the replacement survive both, rather than merely fixing
 * the state once.
 */
class AgentWrapperBootstrapTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun binDir(filesDir: File) = File(filesDir, AgentWrapperBootstrap.RELATIVE_BIN_DIR)

    private fun claude() = RuntimeProfiles.forId(RuntimeProfileId.CLAUDE_CODE)

    @Test
    fun `every launchable agent gets an executable wrapper`() {
        val filesDir = temporaryFolder.newFolder("files")

        val wrapped = AgentWrapperBootstrap.install(filesDir)

        val expected = RuntimeProfiles.all.mapNotNull { it.launchCommand }.sorted()
        assertEquals(expected, wrapped)
        expected.forEach { command ->
            val wrapper = File(binDir(filesDir), command)
            assertTrue("$command must be wrapped", wrapper.isFile)
            assertTrue("$command wrapper must be executable", wrapper.canExecute())
            assertTrue(wrapper.readText().contains(AgentWrapperBootstrap.MARKER))
        }
    }

    /** Nothing that is not an agent gets a launcher; the directory stays a known, closed set. */
    @Test
    fun `plumbing profiles get no wrapper`() {
        val filesDir = temporaryFolder.newFolder("files")

        AgentWrapperBootstrap.install(filesDir)

        listOf("bash", "python", "node", "npm", "clang", "ssh").forEach { command ->
            assertFalse("$command must not be wrapped", File(binDir(filesDir), command).exists())
        }
    }

    /**
     * The whole reason the directory is Verb's alone: a wrapper an installer damaged must come back
     * without the user knowing anything happened, on the next app start.
     */
    @Test
    fun `a damaged wrapper is repaired by the next run`() {
        val filesDir = temporaryFolder.newFolder("files")
        AgentWrapperBootstrap.install(filesDir)
        val wrapper = File(binDir(filesDir), "claude")
        wrapper.writeText("#!/bin/sh\nexit 1\n")

        AgentWrapperBootstrap.install(filesDir)

        assertTrue(wrapper.readText().contains(AgentWrapperBootstrap.MARKER))
        assertTrue(wrapper.canExecute())
    }

    @Test
    fun `a deleted wrapper is recreated by the next run`() {
        val filesDir = temporaryFolder.newFolder("files")
        AgentWrapperBootstrap.install(filesDir)
        val wrapper = File(binDir(filesDir), "claude")
        assertTrue(wrapper.delete())

        AgentWrapperBootstrap.install(filesDir)

        assertTrue(wrapper.isFile)
    }

    /** A wrapper for an agent the catalog dropped would otherwise keep winning PATH forever. */
    @Test
    fun `a wrapper for an unknown command is pruned`() {
        val filesDir = temporaryFolder.newFolder("files")
        AgentWrapperBootstrap.install(filesDir)
        val stale = File(binDir(filesDir), "retired-agent").apply { writeText("#!/bin/sh\n") }

        AgentWrapperBootstrap.install(filesDir)

        assertFalse(stale.exists())
    }

    /**
     * Verb's directory only helps if it is searched before the two directories other people's
     * installers write to. This is the assertion that would fail if that ordering were ever
     * "tidied".
     */
    @Test
    fun `the wrapper directory wins PATH ahead of every installer-writable directory`() {
        val filesDir = temporaryFolder.newFolder("files")
        File(filesDir, "usr/bin/login").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(filesDir, "usr/bin/proot").apply { createNewFile(); setExecutable(true) }
        File(filesDir, "usr/lib/libtermux-exec.so").apply { parentFile?.mkdirs(); createNewFile() }

        val path = TerminalEnvironmentResolver(filesDir).resolve()
            .arguments.first { it.startsWith("PATH=") }
            .removePrefix("PATH=")
            .split(":")

        assertEquals(AgentWrapperBootstrap.GUEST_BIN_DIR, path.first())
        assertTrue(path.indexOf("${VerbGuestPaths.HOME}/.local/bin") > 0)
        assertTrue(path.indexOf("${VerbGuestPaths.PREFIX}/bin") > 0)
    }

    /**
     * The install-time launcher went stale whenever a tool self-updated. Resolution must therefore
     * happen when the command is run, and a versioned install directory must be searched newest
     * -first.
     */
    @Test
    fun `claude resolves its binary at launch, newest version first`() {
        val script = AgentWrapperBootstrap.wrapperScript(claude())

        assertTrue(
            "the npm platform package Verb installs must be preferred",
            script.contains(
                "${VerbGuestPaths.PREFIX}/lib/node_modules/@anthropic-ai/claude-code-linux-arm64-musl/claude"
            )
        )
        // Unquoted so the shell expands it, and fed through verb_newest so a self-update wins.
        assertTrue(
            "the self-installer's version directory must be searched as a glob",
            script.contains("verb_newest ${VerbGuestPaths.HOME}/.local/share/claude/versions/*")
        )
        assertTrue(script.contains("-nt"))
    }

    /**
     * `$HOME/.local/bin/claude` is the file that used to win PATH and fail. It is now searched
     * last, so it can only ever be a fallback -- never the thing that shadows a working install.
     */
    @Test
    fun `the vendor self-installer directory is the last resort, not the first`() {
        val script = AgentWrapperBootstrap.wrapperScript(claude())

        val vendor = script.indexOf("${VerbGuestPaths.HOME}/.local/bin/claude")
        val verbInstalled = script.indexOf("claude-code-linux-arm64-musl/claude")
        assertTrue("the vendor launcher must be searched", vendor > 0)
        assertTrue("Verb's own install must be searched first", verbInstalled < vendor)
    }

    /**
     * An agent the catalog knows nothing specific about still needs to survive a vendor installer,
     * so the two installer-writable locations are appended for every agent.
     */
    @Test
    fun `an agent with no declared candidates still gets the default search`() {
        val codex = RuntimeProfiles.forId(RuntimeProfileId.CODEX)
        assertTrue(codex.binaryCandidates.isEmpty())

        val candidates = AgentWrapperBootstrap.candidatesFor(codex).map { it.path }

        assertEquals(listOf("\$PREFIX/bin/codex", "\$HOME/.local/bin/codex"), candidates)
    }

    /**
     * The wrapper always exists, even before its agent is installed. It must therefore report
     * absence in the vocabulary [GuestCommandRunner] already reads, or a missing agent would show
     * up as installed-but-broken instead of missing.
     */
    @Test
    fun `an unresolved wrapper exits with the POSIX not-found and not-executable codes`() {
        val script = AgentWrapperBootstrap.wrapperScript(claude())

        assertTrue("absence must be reported as env's 127", script.contains("exit 127"))
        assertTrue("a non-executable find must be reported as env's 126", script.contains("exit 126"))
    }

    /**
     * The ABI is read out of the file rather than assumed -- the mistake this sprint kept making.
     * Reading a bounded prefix matters: an agent binary can be hundreds of megabytes, and this runs
     * on every single invocation.
     */
    @Test
    fun `an undeclared binary has its interpreter read, from a bounded prefix`() {
        val script = AgentWrapperBootstrap.wrapperScript(claude())

        assertTrue(script.contains("head -c 4096"))
        assertTrue(script.contains("grep -q ld-musl-aarch64"))
    }

    /**
     * `$PREFIX` and `$HOME` are absent from a stripped environment, which is exactly what an agent
     * creates when it spawns a subprocess. The wrapper must not depend on them.
     */
    @Test
    fun `wrappers bake absolute guest paths rather than trusting the environment`() {
        RuntimeProfiles.all.filter { it.isAgent }.forEach { profile ->
            val script = AgentWrapperBootstrap.wrapperScript(profile)
            val resolutionLines = script.lines().filter { it.startsWith("verb_bin=") }
            assertTrue("${profile.id} must search somewhere", resolutionLines.isNotEmpty())
            resolutionLines.forEach { line ->
                assertFalse("${profile.id} must not depend on \$PREFIX: $line", line.contains("\$PREFIX"))
                assertFalse("${profile.id} must not depend on \$HOME: $line", line.contains("\$HOME"))
                assertTrue("${profile.id} must use an absolute path: $line", line.contains(VerbGuestPaths.FILES))
            }
        }
    }

    /** A wrapper that could resolve to itself would fork-bomb the device rather than fail. */
    @Test
    fun `no wrapper can resolve back into the wrapper directory`() {
        RuntimeProfiles.all.filter { it.isAgent }.forEach { profile ->
            AgentWrapperBootstrap.candidatesFor(profile).forEach { candidate ->
                assertFalse(
                    "${profile.id} would loop through ${candidate.path}",
                    candidate.path
                        .replace("\$PREFIX", VerbGuestPaths.PREFIX)
                        .replace("\$HOME", VerbGuestPaths.HOME)
                        .startsWith(AgentWrapperBootstrap.GUEST_BIN_DIR)
                )
            }
        }
    }

    /** Provisioning runs on every launch, so it must be reachable from the one entry point. */
    @Test
    fun `keeping guest startup current also writes the wrappers`() {
        val filesDir = temporaryFolder.newFolder("files").apply { File(this, "home").mkdirs() }

        TermuxBootstrapInstaller.ensureGuestShellStartupCurrent(filesDir)

        assertTrue(File(binDir(filesDir), "claude").isFile)
    }
}
