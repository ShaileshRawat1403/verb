package com.example.verb.privacy

import android.content.Context
import android.content.ContextWrapper
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class LegacyPrivateDataPurgeTest {
    @Test
    fun purge_removes_legacy_text_stores_and_is_idempotent() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        context.getSharedPreferences("verb_privacy_migrations", Context.MODE_PRIVATE).edit().clear().commit()
        context.getSharedPreferences("verb_agent_memory", Context.MODE_PRIVATE)
            .edit().putString("chat_history_v1", "planted prompt").commit()
        context.openOrCreateDatabase("verb_database", Context.MODE_PRIVATE, null).close()

        assertTrue(LegacyPrivateDataPurge.run(context))
        assertFalse(context.getDatabasePath("verb_database").exists())
        assertTrue(
            context.getSharedPreferences("verb_agent_memory", Context.MODE_PRIVATE).all.isEmpty()
        )
        assertFalse(LegacyPrivateDataPurge.run(context))
    }

    @Test
    fun purge_does_not_mark_complete_when_legacy_database_remains() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        context.getSharedPreferences("verb_privacy_migrations", Context.MODE_PRIVATE)
            .edit().clear().commit()
        context.openOrCreateDatabase("verb_database", Context.MODE_PRIVATE, null).close()
        val refusingContext =
            object : ContextWrapper(context) {
                override fun deleteDatabase(name: String): Boolean =
                    if (name == "verb_database") false else super.deleteDatabase(name)
            }

        assertFalse(LegacyPrivateDataPurge.run(refusingContext))
        assertTrue(context.getDatabasePath("verb_database").exists())
        assertFalse(
            context.getSharedPreferences("verb_privacy_migrations", Context.MODE_PRIVATE)
                .getBoolean("prohibited_text_stores_purged_v1", false)
        )

        assertTrue(LegacyPrivateDataPurge.run(context))
    }

    @Test
    fun purge_does_not_mark_complete_when_legacy_agent_memory_remains() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        context.getSharedPreferences("verb_privacy_migrations", Context.MODE_PRIVATE)
            .edit().clear().commit()
        context.getSharedPreferences("verb_agent_memory", Context.MODE_PRIVATE)
            .edit().putString("chat_history_v1", "planted prompt").commit()
        val refusingContext =
            object : ContextWrapper(context) {
                override fun deleteSharedPreferences(name: String): Boolean =
                    if (name == "verb_agent_memory") false else super.deleteSharedPreferences(name)
            }

        assertFalse(LegacyPrivateDataPurge.run(refusingContext))
        assertTrue(
            context.getSharedPreferences("verb_agent_memory", Context.MODE_PRIVATE)
                .contains("chat_history_v1")
        )
        assertFalse(
            context.getSharedPreferences("verb_privacy_migrations", Context.MODE_PRIVATE)
                .getBoolean("prohibited_text_stores_purged_v1", false)
        )

        assertTrue(LegacyPrivateDataPurge.run(context))
    }
}
