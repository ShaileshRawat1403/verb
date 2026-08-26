package com.example.verb.world

import java.io.File
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/**
 * The Working World archive is the documented recovery path, and it had a hole in it.
 *
 * `verb export` wrote archives that `verb import` then refused, because import rejects any tar
 * member that is not a regular file or a directory — a symlink in an archive is a path-traversal
 * waiting to happen, so that refusal is right. The export simply did not check that it satisfied
 * its own importer, and two sources of symlinks slipped through: `.codex/tmp/arg0` (missed by a
 * single dot — the exclusion said `.tmp`) and `node_modules/.bin` shims under `.config/opencode`.
 *
 * Anyone who had used Codex or installed OpenCode therefore held an unrestorable backup. Found by
 * running the round-trip on a physical device on 26 August; no unit test existed for this script at
 * all, which is why it shipped.
 */
class WorldArchiveInvariantTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private val script = File("src/main/assets/verb/world.sh")

    private fun run(vararg command: String): Pair<Int, String> {
        val process = ProcessBuilder(*command)
            .directory(temporaryFolder.root)
            .redirectErrorStream(true)
            .start()
        val output = process.inputStream.bufferedReader().readText()
        process.waitFor(60, TimeUnit.SECONDS)
        return process.exitValue() to output
    }

    /** Calls one function out of the script without running its command dispatch. */
    private fun assertRestorable(payload: File): Pair<Int, String> = run(
        "bash", "-c",
        "source '${script.absolutePath}' >/dev/null 2>&1; assert_payload_restorable '${payload.absolutePath}'"
    )

    private fun payloadContaining(build: (File) -> Unit): File {
        val source = temporaryFolder.newFolder("world")
        build(source)
        val payload = File(temporaryFolder.root, "payload.tgz")
        val (code, output) = run("tar", "-C", source.absolutePath, "-czf", payload.absolutePath, ".")
        assertEquals("tar failed: $output", 0, code)
        return payload
    }

    @Test
    fun `an archive of plain files and directories is restorable`() {
        assumeTrue(script.isFile)

        val payload = payloadContaining { root ->
            File(root, "nested").mkdirs()
            File(root, "nested/session.xml").writeText("<map/>")
        }

        val (code, output) = assertRestorable(payload)
        assertEquals("expected restorable, got: $output", 0, code)
    }

    /** The exact shape the device produced: an npm bin shim inside the exported tree. */
    @Test
    fun `an archive containing a symlink is refused before it can be written`() {
        assumeTrue(script.isFile)

        val payload = payloadContaining { root ->
            File(root, "node_modules/.bin").mkdirs()
            File(root, "real.js").writeText("//")
            java.nio.file.Files.createSymbolicLink(
                File(root, "node_modules/.bin/yaml").toPath(),
                File(root, "real.js").toPath()
            )
        }

        val (code, output) = assertRestorable(payload)
        assertEquals("a symlink must be refused", 1, code)
        assertTrue(output, output.contains("link or special file"))
        // The message has to say this is Verb's bug, not the user's -- they cannot fix an exclusion.
        assertTrue(output, output.contains("not something you did"))
    }

    /**
     * The two exclusions this defect turned on. Pinned by name because both are easy to lose in a
     * reformat, and losing either silently restores the broken behaviour.
     */
    @Test
    fun `the exclusions cover the two symlink sources found on the device`() {
        assumeTrue(script.isFile)
        val source = script.readText()

        assertTrue("`.codex/tmp` (no dot) must be excluded", source.contains("--exclude=*/.codex/tmp/*"))
        assertTrue("npm bin shims must be excluded", source.contains("--exclude=*/node_modules/.bin/*"))
    }

    /** Export must call the check. An invariant nothing invokes is a comment. */
    @Test
    fun `the export path asserts restorability before writing the archive`() {
        assumeTrue(script.isFile)
        val source = script.readText()

        val check = source.indexOf("if ! assert_payload_restorable")
        val encrypt = source.indexOf("openssl enc -aes-256-cbc")
        assertTrue("export must call assert_payload_restorable", check > 0)
        assertTrue("the check must run before the archive is encrypted", check < encrypt)
    }
}
