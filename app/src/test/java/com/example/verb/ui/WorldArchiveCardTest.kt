package com.example.verb.ui

import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class WorldArchiveCardTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private fun show(
        archiveName: String? = null,
        message: String? = null,
        onSave: () -> Unit = {},
        onPick: () -> Unit = {}
    ) {
        composeTestRule.setContent {
            WorldArchiveCard(
                archiveName = archiveName,
                message = message,
                onSaveToDownloads = onSave,
                onPickArchive = onPick
            )
        }
    }

    @Test
    fun `with no archive, saving is not offered as something that would work`() {
        // The button stays visible so the path is discoverable, but disabled: enabling it would
        // promise a save Verb has nothing to perform.
        show(archiveName = null)

        composeTestRule.onNodeWithTag("world_archive_state")
            .assertExists()
        composeTestRule.onNodeWithText("No archive yet — run the command above first.")
            .assertExists()
        composeTestRule.onNodeWithTag("btn_world_save").assertIsNotEnabled()
    }

    @Test
    fun `an archive on disk names the file rather than claiming it is backed up`() {
        show(archiveName = "world.vbak")

        composeTestRule.onNodeWithText("Ready to save: world.vbak").assertExists()
        composeTestRule.onNodeWithTag("btn_world_save").assertIsEnabled()
    }

    @Test
    fun `bringing an archive in is always available -- it is how a fresh install recovers`() {
        // A device that has just been reinstalled has no archive of its own; if this button
        // followed the same enablement as saving, recovery would be locked behind the thing
        // recovery is meant to restore.
        var picked = 0
        show(archiveName = null, onPick = { picked++ })

        composeTestRule.onNodeWithTag("btn_world_restore").assertIsEnabled().performClick()

        assertEquals(1, picked)
    }

    @Test
    fun `save reports its outcome through the message, not through the button`() {
        var saved = 0
        show(archiveName = "world.vbak", message = "Saved to Download/world.vbak.", onSave = { saved++ })

        composeTestRule.onNodeWithTag("btn_world_save").performClick()
        composeTestRule.onNodeWithTag("world_archive_message").assertExists()
        composeTestRule.onNodeWithText("Saved to Download/world.vbak.").assertExists()

        assertEquals(1, saved)
    }
}
