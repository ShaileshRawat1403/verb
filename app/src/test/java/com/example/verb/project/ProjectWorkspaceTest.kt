package com.example.verb.project

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class ProjectWorkspaceTest {
    @get:Rule val temporaryFolder = TemporaryFolder()

    @Test
    fun `only direct children of the app owned projects root are contained`() {
        val filesDir = temporaryFolder.newFolder("files")
        val workspace = ProjectWorkspace(filesDir)
        val project = File(workspace.root, "safe-project").apply { mkdir() }
        val nested = File(project, "nested").apply { mkdir() }

        assertTrue(workspace.isContained(project))
        assertFalse(workspace.isContained(nested))
        assertFalse(workspace.isContained(File(filesDir, "outside")))
    }
}
