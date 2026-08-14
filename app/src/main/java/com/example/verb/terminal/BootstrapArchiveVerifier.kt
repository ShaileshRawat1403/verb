package com.example.verb.terminal

import java.io.File
import java.io.InputStreamReader
import java.security.MessageDigest
import java.util.zip.ZipFile

/** Validates the review artifact before it is passed to the staged installer. */
object BootstrapArchiveVerifier {
    private const val expectedArchiveName = "bootstrap-aarch64.zip"
    private const val symlinkManifest = "SYMLINKS.txt"
    private const val loginPath = "bin/login"
    private const val execShimPath = "lib/libtermux-exec.so"

    fun verify(archive: File, checksumFile: File): Result<Unit> = runCatching {
        require(archive.isFile && archive.length() > 0L) { "Bootstrap ZIP is unavailable or empty." }
        require(checksumFile.isFile && checksumFile.length() > 0L) { "Bootstrap SHA-256 file is unavailable or empty." }
        val parts = checksumFile.useLines { lines ->
            lines.map(String::trim).firstOrNull(String::isNotEmpty)?.split(Regex("\\s+"))
        }
        require(parts != null && parts.size >= 2) { "Bootstrap SHA-256 file has an invalid format." }
        val expected = parts[0].lowercase()
        require(expected.length == 64 && expected.all { it in "0123456789abcdef" }) { "Bootstrap SHA-256 value is invalid." }
        require(parts[1].substringAfterLast('/') == expectedArchiveName) { "Checksum does not name bootstrap-aarch64.zip." }
        require(sha256(archive) == expected) { "Bootstrap SHA-256 verification failed." }
        ZipFile(archive).use { zip ->
            require(zip.getEntry(loginPath)?.isDirectory == false) {
                "Bootstrap ZIP is missing required entry: $loginPath"
            }
            val manifestEntry = zip.getEntry(symlinkManifest)
            require(manifestEntry?.isDirectory == false) {
                "Bootstrap ZIP is missing required entry: $symlinkManifest"
            }
            val entries = zip.entries().asSequence().filterNot { it.isDirectory }.map { it.name }.toSet()
            val hasShim = execShimPath in entries || manifestCreatesShim(zip, manifestEntry, entries)
            require(hasShim) {
                "Bootstrap ZIP is missing required runtime shim: $execShimPath"
            }
        }
    }

    private fun manifestCreatesShim(zip: ZipFile, manifest: java.util.zip.ZipEntry, entries: Set<String>): Boolean {
        val links = zip.getInputStream(manifest).use { input ->
            InputStreamReader(input).buffered().readLines()
        }
        return links.any { line ->
            val parts = line.split("←", limit = 2)
            if (parts.size != 2 || normalize(parts[1]) != execShimPath) return@any false
            val target = parts[0]
            val resolvedTarget = if (target.startsWith('/')) null else {
                normalize("${execShimPath.substringBeforeLast('/')}/$target")
            }
            resolvedTarget in entries
        }
    }

    private fun normalize(path: String): String = path.removePrefix("./")

    private fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
            var count = input.read(buffer)
            while (count >= 0) {
                if (count > 0) digest.update(buffer, 0, count)
                count = input.read(buffer)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }
}
