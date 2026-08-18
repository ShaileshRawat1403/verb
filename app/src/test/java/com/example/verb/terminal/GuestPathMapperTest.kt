package com.example.verb.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * The mapper is the only place a guest path is ever allowed to become a host [File]. These tests
 * pin both halves of that contract: the allowlisted binds map, and everything else -- unknown
 * roots, the legacy com.termux alias, traversal out of a bind, relative input -- resolves to null
 * rather than to some host path that happens to exist.
 */
class GuestPathMapperTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun filesDir(): File = temporaryFolder.newFolder("files")

    @Test
    fun `a verb guest project path maps to the corresponding host project directory`() {
        val filesDir = filesDir()
        val project = File(filesDir, "projects/demo").apply { mkdirs() }
        val mapper = GuestPathMapper.verbUserland(filesDir)

        val mapped = mapper.toHostPath("${VerbGuestPaths.FILES}/projects/demo")

        assertEquals(project.canonicalFile, mapped)
    }

    @Test
    fun `the verb guest files root itself maps to the host files directory`() {
        val filesDir = filesDir()

        val mapped = GuestPathMapper.verbUserland(filesDir).toHostPath(VerbGuestPaths.FILES)

        assertEquals(filesDir.canonicalFile, mapped)
    }

    @Test
    fun `the verb guest home path maps beneath the host files directory`() {
        val filesDir = filesDir()
        File(filesDir, "home").mkdirs()

        val mapped = GuestPathMapper.verbUserland(filesDir).toHostPath(VerbGuestPaths.HOME)

        assertEquals(File(filesDir, "home").canonicalFile, mapped)
    }

    /**
     * A guest path is mapped structurally, not by checking the host filesystem: the browser and the
     * diagnostics probe both handle a non-existent directory already, and refusing to map here
     * would make the result depend on install order rather than on the bind.
     */
    @Test
    fun `a guest path under an allowlisted bind maps even when the host directory does not exist yet`() {
        val filesDir = filesDir()

        val mapped = GuestPathMapper.verbUserland(filesDir).toHostPath("${VerbGuestPaths.FILES}/projects/not-created-yet")

        assertEquals(File(filesDir, "projects/not-created-yet").canonicalFile, mapped)
    }

    @Test
    fun `the agent workspace path maps to the selected project directory`() {
        val project = temporaryFolder.newFolder("selected-project")
        val mapper = GuestPathMapper.agentRuntime(project)

        assertEquals(project.canonicalFile, mapper.toHostPath("/workspace"))
        assertEquals(
            File(project, "src/main").canonicalFile,
            mapper.toHostPath("/workspace/src/main")
        )
    }

    @Test
    fun `a sibling of an allowlisted root is not treated as being inside it`() {
        val project = temporaryFolder.newFolder("selected-project")

        // "/workspaceX" shares a string prefix with "/workspace" but is a different path.
        assertNull(GuestPathMapper.agentRuntime(project).toHostPath("/workspaceX/src"))
    }

    @Test
    fun `traversal out of an allowlisted bind is rejected`() {
        val filesDir = filesDir()
        val mapper = GuestPathMapper.verbUserland(filesDir)

        assertNull(mapper.toHostPath("${VerbGuestPaths.FILES}/../../etc"))
        assertNull(mapper.toHostPath("${VerbGuestPaths.FILES}/projects/../../../root"))
    }

    @Test
    fun `traversal that stays inside the bind is allowed`() {
        val filesDir = filesDir()

        val mapped = GuestPathMapper.verbUserland(filesDir).toHostPath("${VerbGuestPaths.FILES}/projects/../home")

        assertEquals(File(filesDir, "home").canonicalFile, mapped)
    }

    @Test
    fun `a path under no allowlisted bind is rejected`() {
        val mapper = GuestPathMapper.verbUserland(filesDir())

        assertNull(mapper.toHostPath("/etc/passwd"))
        assertNull(mapper.toHostPath("/root"))
        assertNull(mapper.toHostPath("/workspace/src"))
    }

    /**
     * `/data/data/com.termux` is a hidden internal compatibility mount (see [VerbGuestPaths]) and
     * is never Verb's user-visible identity, so a cwd reported under it stays unmapped rather than
     * being silently rewritten into Verb's own storage.
     */
    @Test
    fun `the legacy com termux alias is not an allowlisted bind`() {
        val mapper = GuestPathMapper.verbUserland(filesDir())

        assertNull(mapper.toHostPath("/data/data/com.termux/files/home"))
        assertNull(mapper.toHostPath("/data/data/com.termux/files/usr/bin"))
    }

    @Test
    fun `a relative or blank path is rejected`() {
        val mapper = GuestPathMapper.verbUserland(filesDir())

        assertNull(mapper.toHostPath("projects/demo"))
        assertNull(mapper.toHostPath(""))
        assertNull(mapper.toHostPath("~/projects"))
    }

    @Test
    fun `the empty mapper maps nothing at all`() {
        assertNull(GuestPathMapper.NONE.toHostPath(VerbGuestPaths.HOME))
        assertNull(GuestPathMapper.NONE.toHostPath("/workspace"))
        assertNull(GuestPathMapper.NONE.toHostPath("/"))
    }
}
