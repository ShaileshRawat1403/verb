package com.example.verb.project

import org.junit.Assert.assertFalse
import org.junit.Assert.assertEquals
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

    @Test
    fun `create returns a contained direct child`() {
        val filesDir = temporaryFolder.newFolder("create-files")
        val workspace = ProjectWorkspace(filesDir)

        val project = workspace.create("My New Project")

        assertTrue(project.directory.isDirectory)
        assertTrue(workspace.isContained(project.directory))
        assertTrue(project.id.startsWith("my-new-project-"))
        assertEquals(project, workspace.get(project.id))
    }

    /**
     * The list a person chooses from must show the name they typed. A full id reads as machine
     * output, and the suffix is only useful when two projects share a name.
     */
    @Test
    fun `a project reports the typed name and its disambiguating suffix separately`() {
        val project = VerbProject("mobile-kit-30603ae7", File("/projects/mobile-kit-30603ae7"))

        assertEquals("mobile-kit", project.displayName)
        assertEquals("30603ae7", project.shortId)
    }

    /** An id with no suffix is its own name, never an empty label. */
    @Test
    fun `a project with no suffix still reports a name`() {
        val project = VerbProject("scratch", File("/projects/scratch"))

        assertEquals("scratch", project.displayName)
        assertEquals("", project.shortId)
    }
}
