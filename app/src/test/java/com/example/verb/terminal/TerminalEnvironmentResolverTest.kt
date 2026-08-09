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
    fun `complete Verb bootstrap resolves to the isolated local userland`() {
        val filesDir = temporaryFolder.newFolder("files")
        val login = File(filesDir, "usr/bin/login").apply {
            parentFile?.mkdirs()
            createNewFile()
        }
        val execShim = File(filesDir, "usr/lib/libtermux-exec.so").apply {
            parentFile?.mkdirs()
            createNewFile()
        }
        File(filesDir, "home").mkdirs()
        assertTrue(login.setExecutable(true))
        assertTrue(execShim.isFile)

        val environment = TerminalEnvironmentResolver(filesDir).resolve()
        val prefix = "${filesDir.absolutePath}/usr"

        assertEquals(TerminalEnvironment.Kind.VERB_LOCAL_USERLAND, environment.kind)
        assertEquals("$prefix/bin/login", environment.shellExecutable)
        assertEquals("${filesDir.absolutePath}/home", environment.workingDirectory.absolutePath)
        assertEquals(prefix, environment.prefixDir?.absolutePath)
        assertTrue(environment.variables.contains("PATH=$prefix/bin"))
        assertTrue(environment.variables.contains("TERMUX__ROOTFS=${filesDir.absolutePath}"))
        assertTrue(environment.variables.contains("TERMUX__PREFIX=$prefix"))
    }
}
