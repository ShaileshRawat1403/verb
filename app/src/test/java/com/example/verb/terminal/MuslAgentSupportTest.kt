package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * Agent CLIs published only as musl builds -- Claude Code, OpenCode, anything Bun-compiled -- are
 * aarch64 ELFs whose one unmet dependency on Android is their interpreter. These tests pin the two
 * things that make them start, both found by measurement rather than reasoning:
 *
 * 1. `/lib/ld-musl-aarch64.so.1` must exist, or the kernel refuses the exec with `ENOENT` and
 *    reports "No such file or directory" for a file that is plainly there.
 * 2. Verb's Bionic `termux-exec` preload must not be injected into a musl process, or it fails with
 *    `__register_atfork: symbol not found`.
 */
class MuslAgentSupportTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun resolverWithBootstrap(): Pair<TerminalEnvironmentResolver, File> {
        val filesDir = temporaryFolder.newFolder("files")
        val prefix = File(filesDir, "usr").apply { mkdirs() }
        // Minimum for the resolver to treat this as a complete userland.
        File(prefix, "bin/login").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(prefix, "bin/proot").apply { createNewFile(); setExecutable(true) }
        File(prefix, "lib/libtermux-exec.so").apply { parentFile?.mkdirs(); createNewFile() }
        return TerminalEnvironmentResolver(filesDir) to filesDir
    }

    @Test
    fun `the musl loader is bound at the path the binaries hard-code`() {
        val (resolver, filesDir) = resolverWithBootstrap()
        val loader = File(filesDir, MuslLoaderBootstrap.RELATIVE_INSTALL_PATH)
            .apply { parentFile?.mkdirs(); createNewFile() }

        val args = resolver.resolve().arguments.toList()

        val bindIndex = args.indexOfFirst { it == "${loader.absolutePath}:${MuslLoaderBootstrap.GUEST_LOADER_PATH}" }
        assertTrue("musl loader must be bound into the guest", bindIndex > 0)
        assertEquals("-b", args[bindIndex - 1])
    }

    @Test
    fun `no musl bind is added when the loader is absent`() {
        val (resolver, _) = resolverWithBootstrap()

        val args = resolver.resolve().arguments.toList()

        assertTrue(args.none { it.endsWith(MuslLoaderBootstrap.GUEST_LOADER_PATH) })
    }

    @Test
    fun `the loader path is the one musl binaries actually request`() {
        assertEquals("/lib/ld-musl-aarch64.so.1", MuslLoaderBootstrap.GUEST_LOADER_PATH)
    }

    /**
     * npm rejects musl packages on Android because Bionic reports no libc at all
     * (`Actual libc: undefined`), so the override is required for a package that is genuinely the
     * correct architecture.
     */
    @Test
    fun `musl agent installs force past npm's libc check`() {
        listOf(RuntimeProfileId.CLAUDE_CODE, RuntimeProfileId.OPENCODE).forEach { id ->
            val command = RuntimeProfiles.forId(id).installCommand
            assertTrue("$id must force the musl package", command.contains("npm install -g --force"))
            assertTrue("$id must install a musl build", command.contains("-musl"))
        }
    }

    /**
     * The launcher is no longer written by the install command, and that is the point: a file
     * written once into `$PREFIX/bin` was overwritten by npm and shadowed by a vendor
     * self-installer. Launching belongs to [AgentWrapperBootstrap]; the install must not recreate a
     * competing copy for either of them to destroy again.
     */
    @Test
    fun `musl agent installs no longer write a launcher of their own`() {
        listOf(RuntimeProfileId.CLAUDE_CODE, RuntimeProfileId.OPENCODE).forEach { id ->
            val command = RuntimeProfiles.forId(id).installCommand
            assertTrue("$id must not write into \$PREFIX/bin", !command.contains("\$PREFIX/bin/"))
            assertTrue("$id must not generate a launcher", !command.contains("chmod +x"))
        }
    }

    @Test
    fun `the generated wrapper clears the bionic preload and uses the musl loader`() {
        listOf(RuntimeProfileId.CLAUDE_CODE, RuntimeProfileId.OPENCODE).forEach { id ->
            val script = AgentWrapperBootstrap.wrapperScript(RuntimeProfiles.forId(id))
            // Cleared with shell builtins, not `env`: Termux's `env` reintroduces the exec shim
            // that rejects these non-PIE binaries with "has unexpected e_type: 2".
            assertTrue("$id wrapper must clear the preload", script.contains("unset LD_PRELOAD"))
            assertTrue(
                "$id must use the musl library path",
                script.contains("VERB_MUSL_LIB=${VerbGuestPaths.PREFIX}/lib/musl")
            )
            assertTrue("$id must invoke the musl loader", script.contains(MuslLoaderBootstrap.GUEST_LOADER_PATH))
        }
    }

    @Test
    fun `musl agents still require JavaScript for npm`() {
        listOf(RuntimeProfileId.CLAUDE_CODE, RuntimeProfileId.OPENCODE).forEach { id ->
            assertEquals(
                listOf(RuntimeProfileId.JAVASCRIPT),
                RuntimeProfiles.forId(id).prerequisiteProfiles
            )
        }
    }

    /**
     * DeepSeek Harness is plain JavaScript with no platform binary, so it must not acquire the musl
     * machinery the Bun-compiled agents need.
     */
    @Test
    fun `the deepseek harness installs plainly, with no musl wrapper`() {
        val command = RuntimeProfiles.forId(RuntimeProfileId.DEEPSEEK_HARNESS).installCommand

        assertEquals("npm install -g @deepseek-ai/dsh", command)
        assertEquals(
            listOf(RuntimeProfileId.JAVASCRIPT),
            RuntimeProfiles.forId(RuntimeProfileId.DEEPSEEK_HARNESS).prerequisiteProfiles
        )
    }

    /**
     * Codex is statically linked, so the musl loader is the wrong answer for it -- the loader
     * reports `Not a valid dynamic program`. It must never acquire the musl machinery.
     */
    @Test
    fun `codex is not treated as a musl agent`() {
        val command = RuntimeProfiles.forId(RuntimeProfileId.CODEX).installCommand
        val script = AgentWrapperBootstrap.wrapperScript(RuntimeProfiles.forId(RuntimeProfileId.CODEX))

        assertTrue(command.startsWith("npm install -g @openai/codex"))
        assertTrue("codex must not be forced past npm's libc check", !command.contains("--force"))
        assertTrue("codex's own build must not go through the loader", !script.contains("verb_exec_musl \"\$verb_bin\""))
    }
}
