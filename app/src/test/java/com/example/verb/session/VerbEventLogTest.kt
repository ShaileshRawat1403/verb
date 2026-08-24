package com.example.verb.session

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.Instant

@RunWith(RobolectricTestRunner::class)
class VerbEventLogTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `events are sequenced structural facts with no text carrying field`() {
        val filesDir = temporaryFolder.newFolder("files")
        val session = VerbSession(
            id = "session-1",
            projectId = "project",
            runtime = "claude",
            createdAt = Instant.EPOCH,
            lastSeenAt = Instant.EPOCH,
            state = VerbSessionState.LIVE,
            agent = AgentRef("claude")
        )
        val log = VerbEventLog(filesDir)

        log.append(session, "SESSION_STARTED")
        log.append(session.copy(state = VerbSessionState.RECOVERABLE), "RECOVERY_CHECKED", state = VerbSessionState.RECOVERABLE)

        val records = log.continuityRecords(session.id)
        val durable = java.io.File(filesDir, "verb/events/${session.id}.jsonl").readText()
        assertEquals(listOf(1L, 2L), records.map { it.getLong("seq") })
        assertEquals("RECOVERABLE", records.last().getString("state"))
        assertTrue(durable.contains("SESSION_STARTED"))
        listOf(
            "commandText", "output", "prompt", "transcript", "apiKey", "pid", "processPresent",
            "toolArguments", "toolResult"
        ).forEach { prohibited -> assertFalse("$prohibited leaked into $durable", durable.contains(prohibited)) }
    }
}
