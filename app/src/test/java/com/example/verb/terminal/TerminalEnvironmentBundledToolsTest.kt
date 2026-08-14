package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class TerminalEnvironmentBundledToolsTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun writeElfHeader(file: File, machine: Int = 183) {
        val header = ByteArray(20)
        header[0] = 0x7F
        header[1] = 'E'.code.toByte()
        header[2] = 'L'.code.toByte()
        header[3] = 'F'.code.toByte()
        header[4] = 2
        header[18] = (machine and 0xFF).toByte()
        header[19] = ((machine shr 8) and 0xFF).toByte()
        file.writeBytes(header)
    }

    @Test
    fun `bundled valid tools prepend PATH and set CA bundle`() {
        val filesDir = temporaryFolder.newFolder("files")
        val binDir = File(filesDir, "bin").apply { mkdirs() }
        writeElfHeader(File(binDir, "busybox"))
        File(binDir, "cacert.pem").writeText("cert-bundle")

        val environment = TerminalEnvironmentResolver(
            appFilesDir = filesDir,
            systemPath = "/system/bin:/system/xbin",
            bundledBinDir = binDir
        ).resolve()

        assertEquals(TerminalEnvironment.Kind.ANDROID_SYSTEM_SHELL, environment.kind)
        assertTrue(environment.variables.contains("PATH=${binDir.absolutePath}:/system/bin:/system/xbin"))
        assertTrue(environment.variables.contains("CURL_CA_BUNDLE=${binDir.absolutePath}/cacert.pem"))
    }

    @Test
    fun `corrupt bundled tools do not alter the plain system environment`() {
        val filesDir = temporaryFolder.newFolder("files")
        val binDir = File(filesDir, "bin").apply { mkdirs() }
        File(binDir, "curl").writeText("Not Found")
        File(binDir, "busybox").writeText("garbage")

        val environment = TerminalEnvironmentResolver(
            appFilesDir = filesDir,
            systemPath = "/system/bin:/system/xbin",
            bundledBinDir = binDir
        ).resolve()

        assertTrue(environment.variables.contains("PATH=/system/bin:/system/xbin"))
        assertTrue(environment.variables.none { it.startsWith("CURL_CA_BUNDLE=") })
    }

    @Test
    fun `missing bundled dir leaves the plain system environment unchanged`() {
        val filesDir = temporaryFolder.newFolder("files")

        val environment = TerminalEnvironmentResolver(
            appFilesDir = filesDir,
            systemPath = "/system/bin:/system/xbin",
            bundledBinDir = File(filesDir, "does-not-exist")
        ).resolve()

        assertTrue(environment.variables.contains("PATH=/system/bin:/system/xbin"))
        assertTrue(environment.variables.none { it.startsWith("CURL_CA_BUNDLE=") })
    }
}
