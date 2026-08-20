package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class TerminalEnvironmentResolverTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `missing Verb bootstrap resolves to the limited Android shell`() {
        val filesDir = temporaryFolder.newFolder("files")

        val environment = TerminalEnvironmentResolver(filesDir, "/system/bin:/system/xbin").resolve()

        assertEquals(TerminalEnvironment.Kind.ANDROID_SYSTEM_SHELL, environment.kind)
        assertEquals("/system/bin/sh", environment.shellExecutable)
        assertEquals(filesDir, environment.workingDirectory)
        assertTrue(environment.variables.contains("PATH=/system/bin:/system/xbin"))
        assertFalse(environment.variables.any { it.contains("com.termux") })
    }

    @Test
    fun `complete bootstrap with proot resolves to the isolated Termux userland`() {
        val filesDir = temporaryFolder.newFolder("files")
        val login = File(filesDir, "usr/bin/login").apply {
            parentFile?.mkdirs()
            createNewFile()
        }
        val execShim = File(filesDir, "usr/lib/libtermux-exec.so").apply {
            parentFile?.mkdirs()
            createNewFile()
        }
        val proot = File(filesDir, "usr/bin/proot").apply {
            parentFile?.mkdirs()
            createNewFile()
        }
        File(filesDir, "home").mkdirs()
        File(filesDir, "usr/etc/resolv.conf").apply {
            parentFile?.mkdirs()
            writeText("nameserver 192.168.1.1\n")
        }
        assertTrue(login.setExecutable(true))
        assertTrue(proot.setExecutable(true))
        assertTrue(execShim.isFile)

        val environment = TerminalEnvironmentResolver(filesDir).resolve()
        val prefix = "${filesDir.absolutePath}/usr"

        assertEquals(TerminalEnvironment.Kind.VERB_LOCAL_USERLAND, environment.kind)
        assertEquals("$prefix/bin/proot", environment.shellExecutable)
        assertEquals("${filesDir.absolutePath}/home", environment.workingDirectory.absolutePath)
        assertEquals(prefix, environment.prefixDir?.absolutePath)

        assertTrue(environment.arguments.contains("-b"))
        assertTrue(environment.arguments.contains("${filesDir.absolutePath}:${VerbGuestPaths.FILES}"))
        assertTrue(environment.arguments.contains("${filesDir.absolutePath}/cache:${VerbGuestPaths.CACHE}"))
        assertTrue(environment.arguments.contains("-w"))
        assertTrue(environment.arguments.contains(VerbGuestPaths.HOME))
        assertTrue(environment.arguments.contains("${filesDir.absolutePath}/usr/etc/resolv.conf:/etc/resolv.conf"))
        assertTrue(environment.arguments.last().endsWith("usr/bin/login"))

        assertTrue(environment.arguments.contains("PATH=${AgentWrapperBootstrap.GUEST_BIN_DIR}:${VerbGuestPaths.HOME}/.local/bin:" +
                "${VerbGuestPaths.PREFIX}/bin:${VerbGuestPaths.PREFIX}/bin/applets"))
        assertTrue(environment.arguments.contains("LD_PRELOAD=${VerbGuestPaths.PREFIX}/lib/libtermux-exec-ld-preload.so"))
        assertTrue(environment.arguments.contains("CURL_CA_BUNDLE=${VerbGuestPaths.PREFIX}/etc/tls/cert.pem"))
        assertTrue(environment.arguments.contains("SSL_CERT_FILE=${VerbGuestPaths.PREFIX}/etc/tls/cert.pem"))
        assertTrue(environment.arguments.contains("HOME=${VerbGuestPaths.HOME}"))
        assertTrue(environment.variables.contains("TERMUX__ROOTFS=${filesDir.absolutePath}"))
        assertTrue(environment.variables.contains("TERMUX__PREFIX=$prefix"))
        assertTrue(environment.variables.contains("PROOT_TMP_DIR=${filesDir.absolutePath}/usr/tmp"))

        assertTrue(environment.arguments.contains("-b"))
        val appDir = File(filesDir.absolutePath).parent ?: filesDir.absolutePath
        assertTrue(environment.arguments.contains("$appDir:/data/data/com.aistudio.verb.app"))
    }

    @Test
    fun `guest HOME PREFIX and PATH are the literal Verb identity with no legacy path`() {
        val filesDir = temporaryFolder.newFolder("files")
        File(filesDir, "usr/bin/login").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(filesDir, "usr/bin/proot").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(filesDir, "usr/lib/libtermux-exec.so").apply { parentFile?.mkdirs(); createNewFile() }
        File(filesDir, "home").mkdirs()

        val environment = TerminalEnvironmentResolver(filesDir).resolve()

        // Literal expected values, not derived from VerbGuestPaths -- this must catch a regression
        // in the constant itself, not just confirm the resolver echoes whatever the constant says.
        assertTrue(environment.arguments.contains("HOME=/data/data/com.aistudio.verb.app/files/home"))
        assertTrue(environment.arguments.contains("PREFIX=/data/data/com.aistudio.verb.app/files/usr"))
        val pathAssignment = environment.arguments.first { it.startsWith("PATH=") }
        assertEquals(
            // Verb's own agent launcher directory comes first, ahead of the two directories other
            // people's installers write to. See AgentWrapperBootstrap.
            "PATH=/data/data/com.aistudio.verb.app/files/usr/libexec/verb/bin:" +
                "/data/data/com.aistudio.verb.app/files/home/.local/bin:" +
                "/data/data/com.aistudio.verb.app/files/usr/bin:" +
                "/data/data/com.aistudio.verb.app/files/usr/bin/applets",
            pathAssignment
        )
        assertFalse(pathAssignment.contains("/data/data/com.termux"))
        assertTrue(environment.arguments.contains("SHELL=/data/data/com.aistudio.verb.app/files/usr/bin/bash"))
    }

    @Test
    fun `hidden legacy compat bind is still present for official Termux repo packages`() {
        val filesDir = temporaryFolder.newFolder("files")
        File(filesDir, "usr/bin/login").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(filesDir, "usr/bin/proot").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(filesDir, "usr/lib/libtermux-exec.so").apply { parentFile?.mkdirs(); createNewFile() }
        File(filesDir, "home").mkdirs()

        val environment = TerminalEnvironmentResolver(filesDir).resolve()

        // Packages installed via apt from the official signed Termux repository (runtime/README.md,
        // "Package-management boundary") are unmodified upstream builds path-bound to com.termux.
        // Do not remove this bind without physical-device proof it is no longer needed.
        val appDir = File(filesDir.absolutePath).parent ?: filesDir.absolutePath
        assertTrue(environment.arguments.contains("$appDir:/data/data/com.termux"))
        assertTrue(environment.arguments.contains("${filesDir.absolutePath}:/data/data/com.termux/files"))
        assertTrue(environment.arguments.contains("${filesDir.absolutePath}/cache:/data/data/com.termux/cache"))
    }

    @Test
    fun `bootstrap without proot still falls back to the Android shell`() {
        val filesDir = temporaryFolder.newFolder("files")
        File(filesDir, "usr/bin/login").apply {
            parentFile?.mkdirs()
            createNewFile()
        }
        File(filesDir, "usr/lib/libtermux-exec.so").apply {
            parentFile?.mkdirs()
            createNewFile()
        }

        val environment = TerminalEnvironmentResolver(filesDir).resolve()

        assertEquals(TerminalEnvironment.Kind.ANDROID_SYSTEM_SHELL, environment.kind)
    }

    @Test
    fun `selected app project maps to its proot guest directory`() {
        val filesDir = temporaryFolder.newFolder("files")
        val project = File(filesDir, "projects/demo").apply { mkdirs() }
        File(filesDir, "usr/bin/login").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(filesDir, "usr/bin/proot").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(filesDir, "usr/lib/libtermux-exec.so").apply { parentFile?.mkdirs(); createNewFile() }

        val environment = TerminalEnvironmentResolver(filesDir, projectDirectory = project).resolve()

        assertEquals(project, environment.workingDirectory)
        assertTrue(environment.arguments.contains("${VerbGuestPaths.FILES}/projects/demo"))
    }

    @Test
    fun `outside project is ignored by the system shell fallback`() {
        val filesDir = temporaryFolder.newFolder("files")
        val outside = temporaryFolder.newFolder("outside")

        val environment = TerminalEnvironmentResolver(filesDir, projectDirectory = outside).resolve()

        assertEquals(filesDir, environment.workingDirectory)
    }

    @Test
    fun `resolveGuestCommand is null when the guest userland is not installed`() {
        val filesDir = temporaryFolder.newFolder("files")

        val environment = TerminalEnvironmentResolver(filesDir).resolveGuestCommand(listOf("git", "--version"))

        assertEquals(null, environment)
    }

    @Test
    fun `resolveGuestCommand shares the exact same mounts and env contract regardless of target shape`() {
        val filesDir = temporaryFolder.newFolder("files")
        File(filesDir, "usr/bin/login").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(filesDir, "usr/bin/proot").apply { parentFile?.mkdirs(); createNewFile(); setExecutable(true) }
        File(filesDir, "usr/lib/libtermux-exec.so").apply { parentFile?.mkdirs(); createNewFile() }
        File(filesDir, "home").mkdirs()
        val resolver = TerminalEnvironmentResolver(filesDir)
        val prefix = "${filesDir.absolutePath}/usr"

        // A native ELF binary (e.g. git), a shell script, and a `#!/usr/bin/env node` npm shim are
        // all just a bare command name here -- PATH resolution and shebang interpretation happen
        // inside the guest itself, exactly as they would for the interactive terminal.
        for (guestCommand in listOf(listOf("git", "--version"), listOf("some-script", "--version"), listOf("codex", "--version"))) {
            val environment = resolver.resolveGuestCommand(guestCommand)
            requireNotNull(environment)

            assertEquals(TerminalEnvironment.Kind.VERB_LOCAL_USERLAND, environment.kind)
            assertEquals("$prefix/bin/proot", environment.shellExecutable)
            // Same binds as the interactive session.
            assertTrue(environment.arguments.contains("${filesDir.absolutePath}:${VerbGuestPaths.FILES}"))
            assertTrue(environment.arguments.contains("${filesDir.absolutePath}/cache:${VerbGuestPaths.CACHE}"))
            // Same HOME/PREFIX/PATH/LD_LIBRARY_PATH/certs contract as the interactive session.
            assertTrue(environment.arguments.contains("HOME=${VerbGuestPaths.HOME}"))
            assertTrue(environment.arguments.contains("PREFIX=${VerbGuestPaths.PREFIX}"))
            assertTrue(environment.arguments.contains("PATH=${AgentWrapperBootstrap.GUEST_BIN_DIR}:${VerbGuestPaths.HOME}/.local/bin:" +
                "${VerbGuestPaths.PREFIX}/bin:${VerbGuestPaths.PREFIX}/bin/applets"))
            assertTrue(environment.arguments.contains("LD_LIBRARY_PATH=${VerbGuestPaths.PREFIX}/lib"))
            assertTrue(environment.arguments.contains("CURL_CA_BUNDLE=${VerbGuestPaths.PREFIX}/etc/tls/cert.pem"))
            assertTrue(environment.arguments.contains("SSL_CERT_FILE=${VerbGuestPaths.PREFIX}/etc/tls/cert.pem"))
            // Same working-directory resolution (HOME, no project selected).
            assertEquals(File(filesDir, "home"), environment.workingDirectory)
            // Trailing argv is the bare probe command -- not the interactive login shell.
            assertEquals(guestCommand, environment.arguments.takeLast(guestCommand.size).toList())
        }
    }
}
