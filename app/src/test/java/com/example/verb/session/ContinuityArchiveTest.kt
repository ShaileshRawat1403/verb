package com.example.verb.session

import androidx.test.core.app.ApplicationProvider
import com.example.BuildConfig
import com.example.verb.project.VerbProject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.Instant

@RunWith(RobolectricTestRunner::class)
class ContinuityArchiveTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `export is allowlist-only and carries no path terminal text transcript or credential`() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val directory = temporaryFolder.newFolder("client-secret-project")
        val project = VerbProject("client-secret-project", directory)
        val session = VerbSession(
            id = "session-safe",
            projectId = project.id,
            runtime = "claude",
            createdAt = Instant.parse("2026-08-24T10:00:00Z"),
            lastSeenAt = Instant.parse("2026-08-24T10:01:00Z"),
            state = VerbSessionState.RECOVERABLE,
            lastKnownCwd = directory.absolutePath,
            lastObservedAt = Instant.parse("2026-08-24T10:00:30Z"),
            agent = AgentRef("claude", "conversation-safe")
        )

        val bytes = ContinuityArchive.buildForTest(context, project, listOf(session))
        val text = bytes.toString(Charsets.UTF_8)
        val summary = ContinuityArchive.validateForTest(bytes)

        assertEquals(1, summary.sessions)
        assertFalse(text.contains(directory.absolutePath))
        assertFalse(text.contains("commandText"))
        assertFalse(text.contains("terminalOutput"))
        assertFalse(text.contains("transcript"))
        assertFalse(text.contains("apiKey"))
        assertFalse(text.contains("processPresent"))
        assertFalse(text.contains("pid"))
        assertTrue(text.contains("\"recordedState\":\"RECOVERABLE\""))
    }

    /**
     * Provenance comes from the build, not from a literal maintained beside it.
     *
     * A hardcoded "0.1.0-beta.2" outlived the beta.3 release: the published APK reported beta.2,
     * and so did every archive it exported. The assertion is deliberately against
     * [BuildConfig.VERSION_NAME] rather than against a spelled-out version -- a test that named the
     * expected string would be a second literal to forget, which is the failure it exists to stop.
     */
    @Test
    fun `the exported origin record carries the build's own version`() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val directory = temporaryFolder.newFolder("provenance-project")
        val project = VerbProject("provenance-project", directory)

        val bytes = ContinuityArchive.buildForTest(context, project, emptyList())
        val text = bytes.toString(Charsets.UTF_8)

        assertTrue(
            "the origin record must state the version of the build that wrote it, got: " +
                text.lineSequence().first(),
            text.contains("\"verbVersion\":\"${BuildConfig.VERSION_NAME}\"")
        )
        // And nothing may still be carrying the version that shipped mislabelled.
        assertFalse(
            "a stale hardcoded version has come back",
            BuildConfig.VERSION_NAME != "0.1.0-beta.2" && text.contains("0.1.0-beta.2")
        )
    }

    @Test
    fun `tampering is rejected before an import can be applied`() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val directory = temporaryFolder.newFolder("project")
        val bytes = ContinuityArchive.buildForTest(
            context,
            VerbProject("project", directory),
            emptyList()
        )
        val tampered = bytes.copyOf().also { it[it.lastIndex] = 'x'.code.toByte() }

        assertThrows(IllegalArgumentException::class.java) {
            ContinuityArchive.validateForTest(tampered)
        }
    }

    @Test
    fun `unsafe resume identity is absent rather than transported`() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val directory = temporaryFolder.newFolder("project-unsafe")
        val project = VerbProject("project-unsafe", directory)
        val session = VerbSession(
            id = "session-safe",
            projectId = project.id,
            runtime = "claude",
            createdAt = Instant.EPOCH,
            lastSeenAt = Instant.EPOCH,
            state = VerbSessionState.INTERRUPTED,
            agent = AgentRef("claude", "x; touch injected")
        )

        val text = ContinuityArchive.buildForTest(context, project, listOf(session))
            .toString(Charsets.UTF_8)

        assertFalse(text.contains("touch injected"))
        assertTrue(text.contains("\"resumeIdentityRef\":null"))
    }
}
