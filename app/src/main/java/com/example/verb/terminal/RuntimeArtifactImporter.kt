package com.example.verb.terminal

import android.content.ContentResolver
import android.net.Uri
import java.io.File

/** Copies user-selected documents into private storage, verifies them, then installs the ZIP. */
class RuntimeArtifactImporter(
    private val contentResolver: ContentResolver,
    private val appFilesDir: File,
    private val installer: LocalUserlandBootstrapInstaller = LocalUserlandBootstrapInstaller(appFilesDir)
) {
    fun importArtifact(zipUri: Uri, checksumUri: Uri): Result<Unit> = runCatching {
        val stagingDir = File(appFilesDir, ".verb-runtime-import")
        require(stagingDir.mkdirs() || stagingDir.isDirectory) { "Could not create temporary runtime import storage." }
        val archive = File(stagingDir, "bootstrap-aarch64.zip")
        val checksum = File(stagingDir, "bootstrap-aarch64.zip.sha256")
        try {
            copyDocument(zipUri, archive)
            copyDocument(checksumUri, checksum)
            BootstrapArchiveVerifier.verify(archive, checksum).getOrThrow()
            when (val result = installer.install(archive)) {
                is LocalUserlandBootstrapInstaller.Result.Installed -> Unit
                is LocalUserlandBootstrapInstaller.Result.AlreadyInstalled -> error("A Verb runtime is already installed.")
                is LocalUserlandBootstrapInstaller.Result.Failure -> error(result.message)
            }
        } finally {
            stagingDir.deleteRecursively()
        }
    }

    private fun copyDocument(uri: Uri, destination: File) {
        val input = contentResolver.openInputStream(uri) ?: error("Could not read the selected runtime document.")
        input.use { source -> destination.outputStream().use { target -> source.copyTo(target) } }
    }
}
