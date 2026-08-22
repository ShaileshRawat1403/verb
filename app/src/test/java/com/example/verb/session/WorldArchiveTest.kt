package com.example.verb.session

import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class WorldArchiveTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun home(): File = File(temporaryFolder.root, "home").apply { mkdirs() }

    private fun archive(name: String, modifiedAt: Long, body: String = "cipher"): File =
        File(home(), name).apply {
            writeText(body)
            setLastModified(modifiedAt)
        }

    @Test
    fun `no archive means nothing to offer, not an empty name`() {
        assertNull(WorldArchive.newestArchive(temporaryFolder.root))
    }

    @Test
    fun `only vbak files count -- a world is not every file in the home directory`() {
        File(home(), "notes.txt").writeText("not an archive")
        File(home(), ".env").writeText("SECRET=1")

        assertNull(WorldArchive.newestArchive(temporaryFolder.root))
    }

    @Test
    fun `the newest archive wins, so a fresh export is what gets saved`() {
        archive("old.vbak", modifiedAt = 1_000_000)
        archive("new.vbak", modifiedAt = 9_000_000)

        assertEquals("new.vbak", WorldArchive.newestArchive(temporaryFolder.root)?.name)
    }

    @Test
    fun `staging copies the chosen file to the fixed name verb import is told to read`() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val source = File(temporaryFolder.newFolder("downloads"), "chosen.vbak")
        source.writeText("encrypted-bytes")

        val outcome = WorldArchive.stageForImport(context, Uri.fromFile(source), temporaryFolder.root)

        assertEquals(
            WorldArchive.Outcome.Saved("imported-world.vbak"),
            outcome
        )
        val staged = File(home(), "imported-world.vbak")
        assertTrue(staged.isFile)
        assertEquals("encrypted-bytes", staged.readText())
    }

    @Test
    fun `staging never replaces the world itself -- only a file import will later read`() {
        // The failure this guards against is a one-tap restore: staging must leave the live
        // credentials untouched, because the person has not yet seen what the archive contains.
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val live = File(home(), ".env").apply { writeText("LIVE=1") }
        val source = File(temporaryFolder.newFolder("picked"), "world.vbak")
        source.writeText("encrypted-bytes")

        WorldArchive.stageForImport(context, Uri.fromFile(source), temporaryFolder.root)

        assertEquals("LIVE=1", live.readText())
    }

    @Test
    fun `an unreadable source is reported, not swallowed into a false success`() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val missing = Uri.fromFile(File(temporaryFolder.root, "does-not-exist.vbak"))

        val outcome = WorldArchive.stageForImport(context, missing, temporaryFolder.root)

        assertTrue(outcome is WorldArchive.Outcome.Failed)
    }
}
