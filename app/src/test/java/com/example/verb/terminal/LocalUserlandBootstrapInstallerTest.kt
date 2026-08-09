package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.io.FileOutputStream
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

class LocalUserlandBootstrapInstallerTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `installs a staged bootstrap with executable entry and symlink manifest`() {
        val filesDir = temporaryFolder.newFolder("files")
        val archive = createArchive(
            "bin/login" to "#!/bin/sh",
            "lib/libtermux-exec.so" to "shim",
            "SYMLINKS.txt" to "login←bin/sh\n"
        )
        val symlinks = mutableListOf<Pair<String, String>>()

        val result = LocalUserlandBootstrapInstaller(
            appFilesDir = filesDir,
            symlinkCreator = TerminalSymlinkCreator { target, link -> symlinks += target to link }
        ).install(archive)

        assertEquals(LocalUserlandBootstrapInstaller.Result.Installed, result)
        val login = File(filesDir, "usr/bin/login")
        assertTrue(login.isFile)
        assertTrue(login.canExecute())
        assertTrue(File(filesDir, "usr/lib/libtermux-exec.so").isFile)
        assertTrue(File(filesDir, "home").isDirectory)
        assertEquals(listOf("login" to "${filesDir.absolutePath}/.verb-runtime-prefix-staging/bin/sh"), symlinks)
        assertFalse(File(filesDir, ".verb-runtime-prefix-staging").exists())
    }

    @Test
    fun `rejects zip paths that escape runtime staging`() {
        val filesDir = temporaryFolder.newFolder("files")
        val archive = createArchive("../escape" to "not allowed")

        val result = LocalUserlandBootstrapInstaller(filesDir).install(archive)

        assertTrue(result is LocalUserlandBootstrapInstaller.Result.Failure)
        assertFalse(File(filesDir.parentFile, "escape").exists())
        assertFalse(File(filesDir, "usr").exists())
    }

    private fun createArchive(vararg files: Pair<String, String>): File {
        val archive = temporaryFolder.newFile("bootstrap.zip")
        ZipOutputStream(FileOutputStream(archive)).use { output ->
            files.forEach { (path, contents) ->
                output.putNextEntry(ZipEntry(path))
                output.write(contents.toByteArray())
                output.closeEntry()
            }
        }
        return archive
    }
}
