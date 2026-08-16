package com.example.verb.terminal

import android.system.Os
import java.io.File
import java.io.IOException
import java.io.InputStream
import java.security.MessageDigest
import java.util.zip.GZIPInputStream

/**
 * Installs a CI-produced Linux agent rootfs without ever replacing the active version in place.
 * The archive is verified before extraction, extracted into a version-specific staging directory,
 * checked for its declared command paths, and activated by an atomic manifest rename.
 */
class AgentRuntimeInstaller(
    private val filesDir: File,
    private val paths: AgentRuntimePaths = AgentRuntimePaths(filesDir)
) {
    data class InstalledRuntime(
        val manifest: AgentRuntimeManifest,
        val rootfs: File
    )

    fun install(
        archive: File,
        checksumFile: File,
        manifestFile: File
    ): Result<InstalledRuntime> = runCatching {
        val manifest = AgentRuntimeManifest.fromFile(manifestFile).getOrThrow()
        require(archive.isFile && archive.length() > 0L) { "Agent runtime archive is unavailable or empty." }
        val digest = sha256(archive)
        require(digest.equals(manifest.rootfsSha256, ignoreCase = true)) {
            "Agent runtime archive does not match the manifest digest."
        }
        require(checksumMatches(checksumFile, archive, digest)) {
            "Agent runtime SHA-256 verification failed."
        }

        require(paths.root.mkdirs() || paths.root.isDirectory) { "Could not create agent runtime storage." }
        require(paths.versions.mkdirs() || paths.versions.isDirectory) { "Could not create agent runtime versions." }
        val staging = File(paths.root, ".staging-${manifest.runtimeVersion}-${System.nanoTime()}")
        val stagedRootfs = File(staging, "rootfs")
        try {
            require(stagedRootfs.mkdirs() || stagedRootfs.isDirectory) {
                "Could not create agent runtime staging directory."
            }
            TarGzipExtractor.extract(archive, stagedRootfs)
            validateRootfs(stagedRootfs, manifest)

            val versionDir = paths.version(manifest.runtimeVersion)
            if (versionDir.exists()) versionDir.deleteRecursively()
            require(staging.renameTo(versionDir)) { "Could not activate staged agent runtime." }

            val activeTemp = File(paths.root, "active.properties.tmp")
            activeTemp.writeText(manifest.toPropertiesText())
            require(activeTemp.renameTo(paths.activeManifest)) { "Could not activate agent runtime manifest." }
            InstalledRuntime(manifest, paths.versionRootfs(manifest.runtimeVersion))
        } finally {
            if (staging.exists()) staging.deleteRecursively()
        }
    }

    fun active(): InstalledRuntime? {
        val manifest = paths.activeManifest.takeIf(File::isFile)?.let {
            AgentRuntimeManifest.fromFile(it).getOrNull()
        } ?: return null
        val rootfs = paths.versionRootfs(manifest.runtimeVersion)
        return manifest.takeIf { rootfs.isDirectory }?.let { InstalledRuntime(it, rootfs) }
    }

    private fun validateRootfs(rootfs: File, manifest: AgentRuntimeManifest) {
        manifest.requiredCommands.forEach { absolutePath ->
            val command = File(rootfs, absolutePath.removePrefix("/"))
            require(command.isFile && command.canExecute()) {
                "Agent runtime is missing executable ${absolutePath}."
            }
        }
    }

    private fun checksumMatches(checksumFile: File, archive: File, digest: String): Boolean {
        require(checksumFile.isFile && checksumFile.length() > 0L) { "Agent runtime checksum is unavailable." }
        val parts = checksumFile.useLines { lines ->
            lines.map(String::trim).firstOrNull(String::isNotEmpty)?.split(Regex("\\s+"))
        } ?: return false
        val expectedName = parts.getOrNull(1)?.substringAfterLast('/')
        return parts.firstOrNull()?.equals(digest, ignoreCase = true) == true &&
            (expectedName == null || expectedName == archive.name)
    }

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

/** Minimal tar reader for Docker-exported rootfs archives; rejects path traversal. */
private object TarGzipExtractor {
    private const val BLOCK_SIZE = 512
    private const val MAX_ENTRY_SIZE = 512L * 1024L * 1024L

    fun extract(archive: File, destination: File) {
        GZIPInputStream(archive.inputStream(), 64 * 1024).use { input ->
            val header = ByteArray(BLOCK_SIZE)
            while (true) {
                readFully(input, header)
                if (header.all { it == 0.toByte() }) return
                val name = tarString(header, 0, 100)
                val prefix = tarString(header, 345, 155)
                val path = if (prefix.isBlank()) name else "$prefix/$name"
                require(path.isNotBlank() && !path.startsWith('/') && !path.split('/').contains("..")) {
                    "Agent runtime archive contains an unsafe path."
                }
                val size = tarOctal(header, 124, 12)
                require(size in 0..MAX_ENTRY_SIZE) { "Agent runtime archive entry is too large." }
                val mode = tarOctal(header, 100, 8)
                val type = header[156].toInt().toChar()
                val linkName = tarString(header, 157, 100)
                val target = File(destination, path)
                when (type) {
                    '5' -> require(target.mkdirs() || target.isDirectory) { "Could not create $path." }
                    '2' -> createSymlink(target, linkName)
                    '1' -> createHardlink(target, linkName, destination)
                    '\u0000', '0' -> extractFile(input, target, size, mode)
                    else -> skip(input, size)
                }
                val padding = (BLOCK_SIZE - (size % BLOCK_SIZE)) % BLOCK_SIZE
                skip(input, padding)
            }
        }
    }

    private fun extractFile(input: InputStream, target: File, size: Long, mode: Long) {
        require(target.parentFile?.let { it.mkdirs() || it.isDirectory } != false) {
            "Could not create ${target.parent}."
        }
        target.outputStream().use { output ->
            var remaining = size
            val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
            while (remaining > 0) {
                val count = input.read(buffer, 0, minOf(buffer.size.toLong(), remaining).toInt())
                require(count > 0) { "Unexpected end of agent runtime archive." }
                output.write(buffer, 0, count)
                remaining -= count
            }
        }
        target.setReadable(true, false)
        target.setWritable(true, true)
        if (mode and 73L != 0L) target.setExecutable(true, false)
    }

    private fun createSymlink(target: File, linkName: String) {
        require(linkName.isNotEmpty() && !linkName.split('/').contains("..")) { "Unsafe symlink in agent runtime." }
        require(target.parentFile?.let { it.mkdirs() || it.isDirectory } != false) {
            "Could not create ${target.parent}."
        }
        if (target.exists() || target.isSymbolicLinkCompat()) target.delete()
        Os.symlink(linkName, target.absolutePath)
    }

    private fun createHardlink(target: File, linkName: String, destination: File) {
        require(!linkName.startsWith('/') && !linkName.split('/').contains("..")) { "Unsafe hardlink in agent runtime." }
        val source = File(destination, linkName)
        require(source.isFile) { "Hardlink target is missing: $linkName." }
        require(target.parentFile?.let { it.mkdirs() || it.isDirectory } != false) {
            "Could not create ${target.parent}."
        }
        Os.link(source.absolutePath, target.absolutePath)
    }

    private fun skip(input: InputStream, count: Long) {
        var remaining = count
        while (remaining > 0) {
            val skipped = input.skip(remaining)
            require(skipped > 0) { "Unexpected end of agent runtime archive." }
            remaining -= skipped
        }
    }

    private fun readFully(input: InputStream, buffer: ByteArray) {
        var offset = 0
        while (offset < buffer.size) {
            val count = input.read(buffer, offset, buffer.size - offset)
            if (count < 0) {
                if (offset == 0) throw IOException("Unexpected end of agent runtime archive.")
                throw IOException("Truncated agent runtime tar header.")
            }
            offset += count
        }
    }

    private fun tarString(header: ByteArray, offset: Int, length: Int): String =
        header.copyOfRange(offset, offset + length).takeWhile { it != 0.toByte() }
            .toByteArray().toString(Charsets.UTF_8).trim()

    private fun tarOctal(header: ByteArray, offset: Int, length: Int): Long =
        tarString(header, offset, length).trim().ifEmpty { "0" }.toLong(8)

    private fun File.isSymbolicLinkCompat(): Boolean = runCatching {
        val stat = android.system.Os.lstat(absolutePath)
        android.system.OsConstants.S_ISLNK(stat.st_mode)
    }.getOrDefault(false)
}
