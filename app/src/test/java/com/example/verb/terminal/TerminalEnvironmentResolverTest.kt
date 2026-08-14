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
        assertTrue(environment.arguments.contains("${filesDir.absolutePath}:${TermuxGuestPaths.FILES}"))
        assertTrue(environment.arguments.contains("${filesDir.absolutePath}/cache:${TermuxGuestPaths.CACHE}"))
        assertTrue(environment.arguments.contains("-w"))
        assertTrue(environment.arguments.contains(TermuxGuestPaths.HOME))
        assertTrue(environment.arguments.contains("${filesDir.absolutePath}/usr/etc/resolv.conf:/etc/resolv.conf"))
        assertTrue(environment.arguments.last().endsWith("usr/bin/login"))

        assertTrue(environment.arguments.contains("PATH=${TermuxGuestPaths.HOME}/.local/bin:${TermuxGuestPaths.PREFIX}/bin:${TermuxGuestPaths.PREFIX}/bin/applets"))
        assertTrue(environment.arguments.contains("LD_PRELOAD=${TermuxGuestPaths.PREFIX}/lib/libtermux-exec-ld-preload.so"))
        assertTrue(environment.arguments.contains("CURL_CA_BUNDLE=${TermuxGuestPaths.PREFIX}/etc/tls/cert.pem"))
        assertTrue(environment.arguments.contains("SSL_CERT_FILE=${TermuxGuestPaths.PREFIX}/etc/tls/cert.pem"))
        assertTrue(environment.arguments.contains("HOME=${TermuxGuestPaths.HOME}"))
        assertTrue(environment.variables.contains("TERMUX__ROOTFS=${filesDir.absolutePath}"))
        assertTrue(environment.variables.contains("TERMUX__PREFIX=$prefix"))
        assertTrue(environment.variables.contains("PROOT_TMP_DIR=${filesDir.absolutePath}/usr/tmp"))
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
        assertTrue(environment.arguments.contains("${TermuxGuestPaths.FILES}/projects/demo"))
    }

    @Test
    fun `outside project is ignored by the system shell fallback`() {
        val filesDir = temporaryFolder.newFolder("files")
        val outside = temporaryFolder.newFolder("outside")

        val environment = TerminalEnvironmentResolver(filesDir, projectDirectory = outside).resolve()

        assertEquals(filesDir, environment.workingDirectory)
    }
}
