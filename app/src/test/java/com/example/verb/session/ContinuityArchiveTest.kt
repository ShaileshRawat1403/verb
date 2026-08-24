package com.example.verb.session

import androidx.test.core.app.ApplicationProvider
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
