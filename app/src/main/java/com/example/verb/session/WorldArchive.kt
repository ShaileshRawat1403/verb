package com.example.verb.session

import android.content.ContentValues
import android.content.Context
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import java.io.File

/**
 * Moving a world archive out of app-private storage, and back in.
 *
 * `verb export` writes the archive inside the app's own directory, which is exactly the storage an
 * uninstall destroys -- so an archive that stays there protects against nothing. This copies it to
 * the device's Downloads folder, which survives the app entirely, using MediaStore so no storage
 * permission is needed and nothing else on the device gains access to Verb's files.
 *
 * What is copied is already encrypted by `verb export`; this code never sees a credential, only an
 * opaque blob. That ordering is deliberate: the sensitive work happens once, in the shell script the
 * user invoked, rather than in every path that might move a file.
 */
object WorldArchive {

    /** The extension `verb export` writes, and the only thing this will move. */
    const val EXTENSION = "vbak"

    sealed interface Outcome {
        data class Saved(val displayName: String) : Outcome
        data class Failed(val reason: String) : Outcome
        data object NothingToSave : Outcome
    }

    /** The newest archive `verb export` has written, if any. */
    fun newestArchive(filesDir: File): File? =
        File(filesDir, "home").listFiles { file -> file.isFile && file.extension == EXTENSION }
            ?.maxByOrNull { it.lastModified() }

    /**
     * Copies [archive] into Downloads.
     *
     * On API 29+ this goes through MediaStore, which needs no permission and writes into the
     * collection the system owns. Below that there is no such API, and Verb says so rather than
     * asking for broad storage access it would then hold forever.
     */
    fun saveToDownloads(context: Context, archive: File): Outcome {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            return Outcome.Failed(
                "Saving to Downloads needs Android 10 or newer. Copy the file off the device with adb instead."
            )
        }

        return runCatching {
            val resolver = context.contentResolver
            val values = ContentValues().apply {
                put(MediaStore.Downloads.DISPLAY_NAME, archive.name)
                put(MediaStore.Downloads.MIME_TYPE, "application/octet-stream")
                put(MediaStore.Downloads.IS_PENDING, 1)
            }
            val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
                ?: return Outcome.Failed("Downloads is not available on this device.")

            resolver.openOutputStream(uri)?.use { output ->
                archive.inputStream().use { input -> input.copyTo(output) }
            } ?: return Outcome.Failed("Downloads could not be opened for writing.")

            values.clear()
            values.put(MediaStore.Downloads.IS_PENDING, 0)
            resolver.update(uri, values, null, null)

            Outcome.Saved("${Environment.DIRECTORY_DOWNLOADS}/${archive.name}")
        }.getOrElse { error ->
            Outcome.Failed(error.message ?: "The archive could not be saved.")
        }
    }

    /**
     * Copies a chosen archive back into the guest's home directory, where `verb import` can read it.
     *
     * Restoring is deliberately two steps: this moves the file, and `verb import` shows what is in
     * it and asks before replacing anything. A one-tap restore would be a one-tap way to overwrite a
     * working login with an older one.
     */
    fun stageForImport(context: Context, uri: android.net.Uri, filesDir: File): Outcome = runCatching {
        val home = File(filesDir, "home").apply { mkdirs() }
        val target = File(home, "imported-world.$EXTENSION")
        context.contentResolver.openInputStream(uri)?.use { input ->
            target.outputStream().use { output -> input.copyTo(output) }
        } ?: return Outcome.Failed("That file could not be opened.")
        Outcome.Saved(target.name)
    }.getOrElse { error ->
        Outcome.Failed(error.message ?: "The archive could not be staged.")
    }
}
