package com.example.verb.terminal

import java.io.File
import java.io.FileOutputStream
import java.security.MessageDigest
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class BootstrapArchiveVerifierTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `accepts matching checksum and required runtime entries`() {
        val archive = createArchive("bin/login", "lib/libtermux-exec.so", "SYMLINKS.txt")
        val checksum = checksumFile(archive)

        assertTrue(BootstrapArchiveVerifier.verify(archive, checksum).isSuccess)
    }

    @Test
    fun `rejects checksum mismatch`() {
        val archive = createArchive("bin/login", "lib/libtermux-exec.so", "SYMLINKS.txt")
        val checksum = temporaryFolder.newFile("bootstrap-aarch64.zip.sha256")
        checksum.writeText("${"0".repeat(64)}  bootstrap-aarch64.zip\n")

        assertTrue(BootstrapArchiveVerifier.verify(archive, checksum).isFailure)
    }

    @Test
    fun `rejects archive with differently named shim`() {
        val archive = createArchive("bin/login", "lib/libtermux-exec-v2.so", "SYMLINKS.txt")
        val checksum = checksumFile(archive)

        assertTrue(BootstrapArchiveVerifier.verify(archive, checksum).isFailure)
    }

    @Test
    fun `accepts the exact shim created by the bootstrap symlink manifest`() {
        val archive = createArchive(
            "bin/login",
            "lib/libtermux-exec-ld-preload.so",
            "SYMLINKS.txt" to "libtermux-exec-ld-preload.so←./lib/libtermux-exec.so"
        )
        val checksum = checksumFile(archive)

        assertTrue(BootstrapArchiveVerifier.verify(archive, checksum).isSuccess)
    }

    private fun createArchive(vararg entries: Any): File {
        val archive = temporaryFolder.newFile("bootstrap-aarch64.zip")
        ZipOutputStream(FileOutputStream(archive)).use { output ->
            entries.forEach { item ->
                val (entry, contents) = when (item) {
                    is String -> item to "runtime"
                    is Pair<*, *> -> item.first as String to item.second as String
                    else -> error("Unsupported ZIP fixture entry")
                }
                output.putNextEntry(ZipEntry(entry))
                output.write(contents.toByteArray())
                output.closeEntry()
            }
        }
        return archive
    }

    private fun checksumFile(archive: File): File {
        val digest = MessageDigest.getInstance("SHA-256")
        archive.inputStream().use { input ->
            val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
            var count = input.read(buffer)
            while (count >= 0) {
                if (count > 0) digest.update(buffer, 0, count)
                count = input.read(buffer)
            }
        }
        val hash = digest.digest().joinToString("") { "%02x".format(it) }
        return temporaryFolder.newFile("bootstrap-aarch64.zip.sha256").also {
            it.writeText("$hash  bootstrap-aarch64.zip\n")
        }
    }
}
