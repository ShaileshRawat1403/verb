package com.example.verb.privacy

import android.content.Context
import java.io.File

/**
 * Deletes stores from pre-beta builds that retained prohibited command, terminal, and chat text.
 *
 * There is deliberately no data migration: none of these contents is allowed in Verb's durable
 * model. The marker makes the sweep one-shot; every operation is still idempotent so an interrupted
 * launch can safely try again before setting it.
 */
object LegacyPrivateDataPurge {
    private const val MARKER_PREFERENCES = "verb_privacy_migrations"
    private const val MARKER = "prohibited_text_stores_purged_v1"
    private const val LEGACY_DATABASE = "verb_database"
    private const val LEGACY_AGENT_MEMORY = "verb_agent_memory"

    fun run(context: Context): Boolean {
        val marker = context.getSharedPreferences(MARKER_PREFERENCES, Context.MODE_PRIVATE)
        if (marker.getBoolean(MARKER, false)) return false

        context.deleteDatabase(LEGACY_DATABASE)
        context.deleteSharedPreferences(LEGACY_AGENT_MEMORY)

        if (!databaseFilesAreAbsent(context) || !agentMemoryIsAbsent(context)) return false

        return marker.edit().putBoolean(MARKER, true).commit()
    }

    private fun databaseFilesAreAbsent(context: Context): Boolean {
        val database = context.getDatabasePath(LEGACY_DATABASE)
        return listOf(
            database,
            File("${database.path}-journal"),
            File("${database.path}-shm"),
            File("${database.path}-wal"),
        ).none(File::exists)
    }

    private fun agentMemoryIsAbsent(context: Context): Boolean {
        val preferences =
            context.getSharedPreferences(LEGACY_AGENT_MEMORY, Context.MODE_PRIVATE)
        val preferencesDirectory = File(context.applicationInfo.dataDir, "shared_prefs")
        val preferenceFile = File(preferencesDirectory, "$LEGACY_AGENT_MEMORY.xml")
        val backupFile = File(preferencesDirectory, "$LEGACY_AGENT_MEMORY.xml.bak")
        return preferences.all.isEmpty() && !preferenceFile.exists() && !backupFile.exists()
    }
}
