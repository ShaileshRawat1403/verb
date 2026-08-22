package com.example.verb.terminal

import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.rules.TemporaryFolder
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * Verb's own command in the guest. The script's behaviour is exercised by running it; these tests
 * cover the part Kotlin owns -- that it is installed, kept current, executable, and not deleted by
 * the wrapper prune that owns the same directory.
 */
@RunWith(RobolectricTestRunner::class)
class VerbCliBootstrapTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private val context: android.content.Context = ApplicationProvider.getApplicationContext()

    private fun binDir(filesDir: File) =
        File(filesDir, AgentWrapperBootstrap.RELATIVE_BIN_DIR)

    @Test
    fun `installs an executable verb command`() {
        val filesDir = temporaryFolder.newFolder("files")

        assertTrue(VerbCliBootstrap.install(context, filesDir))

        val command = File(binDir(filesDir), VerbCliBootstrap.COMMAND)
        assertTrue(command.isFile)
        assertTrue(command.canExecute())
        val script = command.readText()
        assertTrue("the command should be the world script", script.contains("verb export"))
        assertTrue("import must preview before it replaces", script.contains("--apply"))
    }

    @Test
    fun `the agent wrapper prune leaves verb alone`() {
        // Both write to the same Verb-owned directory, and the prune deletes anything it does not
        // recognise -- which used to include Verb's own command.
        val filesDir = temporaryFolder.newFolder("files")
        VerbCliBootstrap.install(context, filesDir)

        AgentWrapperBootstrap.install(filesDir)

        assertTrue(File(binDir(filesDir), VerbCliBootstrap.COMMAND).isFile)
    }

    @Test
    fun `a stale command is replaced rather than left in place`() {
        val filesDir = temporaryFolder.newFolder("files")
        VerbCliBootstrap.install(context, filesDir)
        val command = File(binDir(filesDir), VerbCliBootstrap.COMMAND)
        val current = command.readText()
        command.writeText("#!/bin/sh\necho an older version\n")

        VerbCliBootstrap.install(context, filesDir)

        assertEquals(current, command.readText())
    }
}
