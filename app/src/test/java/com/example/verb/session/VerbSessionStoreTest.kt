package com.example.verb.session

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.time.Instant

@RunWith(RobolectricTestRunner::class)
class VerbSessionStoreTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    @Before
    fun clearStore() {
        listOf(
            SharedPreferencesVerbSessionStore.PREFERENCES_NAME,
            SharedPreferencesVerbSessionStore.CODEX_PREFERENCES_NAME
        ).forEach { name ->
            context.getSharedPreferences(name, Context.MODE_PRIVATE).edit().clear().commit()
        }
    }

    @Test
    fun `each agent gets its own store, so one agent's launch cannot erase another's recovery record`() {
        val claudeStore = SharedPreferencesVerbSessionStore(context)
        val codexStore = SharedPreferencesVerbSessionStore(
            context,
            SharedPreferencesVerbSessionStore.CODEX_PREFERENCES_NAME
        )
        fun session(id: String, agentType: String) = VerbSession(
            id = id,
            projectId = "alpha",
            runtime = agentType,
            createdAt = Instant.ofEpochMilli(10),
            lastSeenAt = Instant.ofEpochMilli(20),
            state = VerbSessionState.RECOVERABLE,
            lastKnownCwd = "/projects/alpha",
            agent = AgentRef(agentType, "$agentType-conversation")
        )

        claudeStore.save(session("claude-session", "claude"))
        codexStore.save(session("codex-session", "codex"))

        assertEquals("claude-session", claudeStore.load()!!.id)
        assertEquals("codex-session", codexStore.load()!!.id)
        assertEquals("codex-conversation", codexStore.load()!!.agent!!.resumeIdentity)

        codexStore.clear()

        assertNull(codexStore.load())
        assertEquals("clearing one agent's record must not touch another's", "claude-session", claudeStore.load()!!.id)
    }

    @Test
    fun `round trip keeps canonical fields and drops process binding`() {
        val store = SharedPreferencesVerbSessionStore(context)
        val session = VerbSession(
            id = "session-1",
            projectId = "alpha",
            runtime = "claude",
            createdAt = Instant.ofEpochMilli(10),
            lastSeenAt = Instant.ofEpochMilli(20),
            state = VerbSessionState.LIVE,
            lastKnownCwd = "/projects/alpha",
            lastObservedAt = Instant.ofEpochMilli(15),
            process = object : ProcessBinding {},
            agent = AgentRef("claude", "resume-1")
        )

        store.save(session)

        val restored = store.load()!!
        assertEquals(session.copy(process = null), restored)
        assertNull(restored.process)
    }

    @Test
    fun `durable preferences contain no pid or terminal fields`() {
        val store = SharedPreferencesVerbSessionStore(context)
        store.save(
            VerbSession(
                id = "session-1",
                projectId = "alpha",
                runtime = "claude",
                createdAt = Instant.EPOCH,
                lastSeenAt = Instant.EPOCH,
                state = VerbSessionState.INTERRUPTED,
                agent = AgentRef("claude")
            )
        )

        val keys = context.getSharedPreferences("verb_session", Context.MODE_PRIVATE).all.keys
        assertFalse(keys.any { it.contains("pid", ignoreCase = true) })
        assertFalse(keys.any { it.contains("process", ignoreCase = true) })
        assertFalse(keys.any { it.contains("terminal", ignoreCase = true) })
        assertFalse(keys.any { it.contains("input", ignoreCase = true) })
        assertFalse(keys.any { it.contains("output", ignoreCase = true) })
    }
}
