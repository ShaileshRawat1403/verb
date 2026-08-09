package com.example.verb.terminal

import android.system.Os
import java.io.BufferedReader
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.InputStreamReader
import java.util.zip.ZipInputStream

/** Creates a symbolic link while keeping Android-only file-system calls out of installer tests. */
fun interface TerminalSymlinkCreator {
    fun create(target: String, linkPath: String)
}

/**
 * Installs a reviewed Verb-specific bootstrap archive into the application's private files root.
 *
 * The archive format intentionally matches the Termux bootstrap format: entries are relative to
 * `$PREFIX` and `SYMLINKS.txt` contains `target←link` pairs. Installation is staged under the
 * app files directory and never replaces an existing userland. Callers must verify provenance and
 * SHA-256 before passing an archive to this class.
 */
class LocalUserlandBootstrapInstaller(
    private val appFilesDir: File,
    private val symlinkCreator: TerminalSymlinkCreator = TerminalSymlinkCreator { target, link ->
        Os.symlink(target, link)
    }
) {
    sealed interface Result {
        data object Installed : Result
        data object AlreadyInstalled : Result
        data class Failure(val message: String) : Result
    }

    fun install(archive: File): Result {
        val prefixDir = File(appFilesDir, "usr")
        val stagingDir = File(appFilesDir, ".verb-runtime-prefix-staging")

        if (prefixDir.exists()) return Result.AlreadyInstalled
        if (!archive.isFile || archive.length() == 0L) {
            return Result.Failure("Bootstrap archive is unavailable or empty.")
        }

        return try {
            if (stagingDir.exists() && !stagingDir.deleteRecursively()) {
                return Result.Failure("Could not clear an incomplete runtime installation.")
            }
            if (!stagingDir.mkdirs()) {
                return Result.Failure("Could not create a runtime staging directory.")
            }

            val symlinks = mutableListOf<Pair<String, String>>()
            ZipInputStream(FileInputStream(archive)).use { zipInput ->
                var entry = zipInput.nextEntry
                while (entry != null) {
                    when (entry.name) {
                        "SYMLINKS.txt" -> readSymlinks(zipInput, stagingDir, symlinks)
                        else -> extractEntry(zipInput, entry.name, entry.isDirectory, stagingDir)
                    }
                    zipInput.closeEntry()
                    entry = zipInput.nextEntry
                }
            }

            if (symlinks.isEmpty()) throw IllegalStateException("Bootstrap archive has no symlink manifest.")
            symlinks.forEach { (target, linkPath) -> symlinkCreator.create(target, linkPath) }

            val login = File(stagingDir, "bin/login")
            val execShim = File(stagingDir, "lib/libtermux-exec.so")
            if (!login.isFile || !execShim.isFile) {
                throw IllegalStateException("Bootstrap archive is missing required runtime files.")
            }

            if (!stagingDir.renameTo(prefixDir)) {
                throw IllegalStateException("Could not activate the staged terminal runtime.")
            }
            File(appFilesDir, "home").mkdirs()
            Result.Installed
        } catch (exception: Exception) {
            stagingDir.deleteRecursively()
            Result.Failure(exception.message ?: "Runtime installation failed.")
        }
    }

    private fun readSymlinks(
        zipInput: ZipInputStream,
        stagingDir: File,
        symlinks: MutableList<Pair<String, String>>
    ) {
        val reader = BufferedReader(InputStreamReader(zipInput))
        generateSequence { reader.readLine() }.forEach { line ->
            val parts = line.split("←", limit = 2)
            if (parts.size != 2 || parts[0].isBlank()) {
                throw IllegalArgumentException("Malformed bootstrap symlink manifest.")
            }
            val link = safeDestination(stagingDir, parts[1])
            link.parentFile?.mkdirs()
            symlinks += parts[0] to link.absolutePath
        }
    }

    private fun extractEntry(zipInput: ZipInputStream, name: String, isDirectory: Boolean, stagingDir: File) {
        val target = safeDestination(stagingDir, name)
        if (isDirectory) {
            if (!target.mkdirs() && !target.isDirectory) {
                throw IllegalStateException("Could not create runtime directory.")
            }
            return
        }

        target.parentFile?.let { parent ->
            if (!parent.mkdirs() && !parent.isDirectory) {
                throw IllegalStateException("Could not create runtime directory.")
            }
        }
        FileOutputStream(target).use { output -> zipInput.copyTo(output) }
        if (
            name.startsWith("bin/") ||
            name.startsWith("libexec/") ||
            name.startsWith("lib/apt/apt-helper") ||
            name.startsWith("lib/apt/methods/")
        ) {
            target.setExecutable(true, false)
        }
    }

    private fun safeDestination(stagingDir: File, relativePath: String): File {
        require(relativePath.isNotBlank() && !relativePath.startsWith('/') && !relativePath.contains('\u0000')) {
            "Invalid bootstrap archive path."
        }
        val destination = File(stagingDir, relativePath)
        val stagingPath = stagingDir.canonicalFile.path
        val destinationPath = destination.canonicalFile.path
        require(destinationPath.startsWith("$stagingPath${File.separator}")) {
            "Bootstrap archive path escapes its staging directory."
        }
        return destination
    }
}
