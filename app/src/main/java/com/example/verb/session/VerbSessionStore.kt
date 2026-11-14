package com.example.verb.session

import android.annotation.SuppressLint
import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.time.Instant

/**
 * Durable storage for the product-level [VerbSession] record.
 *
 * This store intentionally has no process handle field. [VerbSession.process] is reconstructed by
 * the host that owns the PTY; a persisted LIVE value is only historical evidence and must be
 * reconciled before it is shown as current. The record also never contains terminal bytes,
 * commands, prompts, or credentials.
 */
interface VerbSessionStore {
    fun load(): VerbSession?
    fun loadAll(): List<VerbSession> = listOfNotNull(load())
    fun save(session: VerbSession)
    fun clear()
}

/** Small in-memory implementation for coordinator tests and host adapters that own their store. */
class InMemoryVerbSessionStore(initial: VerbSession? = null) : VerbSessionStore {
    private val values = linkedMapOf<String, VerbSession>().apply {
        initial?.let { put(it.id, it.copy(process = null)) }
    }

    override fun load(): VerbSession? = loadAll().lastOrNull()
    override fun loadAll(): List<VerbSession> = values.values.toList()

    override fun save(session: VerbSession) {
        values[session.id] = session.copy(process = null)
    }

    override fun clear() {
        values.clear()
    }
}

/**
 * Android process-death durable implementation backed by app-private SharedPreferences.
 *
 * One store per agent, named by [preferencesName]. Each record is keyed by product session ID so
 * parallel terminals using the same agent cannot erase one another. Version-one single records are
 * read and migrated on the first successful save.
 */
class SharedPreferencesVerbSessionStore(
    context: Context,
    preferencesName: String = PREFERENCES_NAME
) : VerbSessionStore {
    private val preferences = context.applicationContext.getSharedPreferences(
        preferencesName,
        Context.MODE_PRIVATE
    )

    override fun load(): VerbSession? = loadAll().lastOrNull()

    override fun loadAll(): List<VerbSession> {
        if (preferences.getInt(KEY_SCHEMA_VERSION, -1) == MULTI_SCHEMA_VERSION) {
            val json = preferences.getString(KEY_SESSIONS_JSON, null) ?: return emptyList()
            return runCatching {
                val array = JSONArray(json)
                (0 until array.length()).map { decode(array.getJSONObject(it)) }
            }.getOrElse { throw IllegalStateException("Damaged Verb session registry", it) }
        }
        return listOfNotNull(loadLegacy())
    }

    private fun loadLegacy(): VerbSession? {
        if (preferences.getInt(KEY_SCHEMA_VERSION, -1) != SCHEMA_VERSION) return null
        val id = preferences.getString(KEY_SESSION_ID, null) ?: return null
        val state = preferences.getString(KEY_STATE, null)
            ?.let { runCatching { VerbSessionState.valueOf(it) }.getOrNull() }
            ?: return null
        val createdAt = preferences.getLong(KEY_CREATED_AT, INVALID_INSTANT)
        val lastSeenAt = preferences.getLong(KEY_LAST_SEEN_AT, INVALID_INSTANT)
        if (createdAt == INVALID_INSTANT || lastSeenAt == INVALID_INSTANT) return null

        val agentType = preferences.getString(KEY_AGENT_TYPE, null)
        return VerbSession(
            id = id,
            projectId = preferences.getNullableString(KEY_PROJECT_ID),
            runtime = preferences.getNullableString(KEY_RUNTIME_ID),
            createdAt = Instant.ofEpochMilli(createdAt),
            lastSeenAt = Instant.ofEpochMilli(lastSeenAt),
            state = state,
            lastKnownCwd = preferences.getNullableString(KEY_LAST_KNOWN_CWD),
            lastObservedAt = preferences.getLong(KEY_LAST_OBSERVED_AT, INVALID_INSTANT)
                .takeUnless { it == INVALID_INSTANT }
                ?.let(Instant::ofEpochMilli),
            process = null,
            agent = agentType?.let {
                AgentRef(
                    agentType = it,
                    resumeIdentity = ResumeIdentity.validOrNull(
                        preferences.getNullableString(KEY_RESUME_IDENTITY)
                    )
                )
            }
        )
    }

    @SuppressLint("ApplySharedPref")
    @Synchronized
    override fun save(session: VerbSession) {
        // commit() is deliberate: this metadata is the recovery anchor if Android kills the app
        // immediately after the launch or state transition.
        val records = loadAll().associateByTo(linkedMapOf()) { it.id }
        records[session.id] = session.copy(process = null)
        val json = JSONArray().apply { records.values.forEach { put(encode(it)) } }.toString()
        check(preferences.edit().clear()
            .putInt(KEY_SCHEMA_VERSION, MULTI_SCHEMA_VERSION)
            .putString(KEY_SESSIONS_JSON, json)
            .commit()) { "Verb session metadata was not committed" }
    }

    @SuppressLint("ApplySharedPref")
    override fun clear() {
        check(preferences.edit().clear().commit()) { "Verb session metadata could not be cleared" }
    }

    private fun encode(session: VerbSession): JSONObject = JSONObject().apply {
        put("id", session.id)
        put("projectId", session.projectId ?: JSONObject.NULL)
        put("runtime", session.runtime ?: JSONObject.NULL)
        put("createdAt", session.createdAt.toEpochMilli())
        put("lastSeenAt", session.lastSeenAt.toEpochMilli())
        put("state", session.state.name)
        put("lastKnownCwd", session.lastKnownCwd ?: JSONObject.NULL)
        put("lastObservedAt", session.lastObservedAt?.toEpochMilli() ?: JSONObject.NULL)
        put("agentType", session.agent?.agentType ?: JSONObject.NULL)
        put("resumeIdentity", ResumeIdentity.validOrNull(session.agent?.resumeIdentity) ?: JSONObject.NULL)
    }

    private fun decode(record: JSONObject): VerbSession = VerbSession(
        id = record.getString("id"),
        projectId = record.optString("projectId").takeIf { record.has("projectId") && !record.isNull("projectId") },
        runtime = record.optString("runtime").takeIf { record.has("runtime") && !record.isNull("runtime") },
        createdAt = Instant.ofEpochMilli(record.getLong("createdAt")),
        lastSeenAt = Instant.ofEpochMilli(record.getLong("lastSeenAt")),
        state = VerbSessionState.valueOf(record.getString("state")),
        lastKnownCwd = record.optString("lastKnownCwd").takeIf { record.has("lastKnownCwd") && !record.isNull("lastKnownCwd") },
        lastObservedAt = record.optLong("lastObservedAt").takeIf { !record.isNull("lastObservedAt") }?.let(Instant::ofEpochMilli),
        process = null,
        agent = record.optString("agentType").takeIf { record.has("agentType") && !record.isNull("agentType") }
            ?.let {
                val identity = record.optString("resumeIdentity")
                    .takeIf { record.has("resumeIdentity") && !record.isNull("resumeIdentity") }
                AgentRef(it, ResumeIdentity.validOrNull(identity))
            }
    )

    private fun android.content.SharedPreferences.getNullableString(key: String): String? =
        if (contains(key)) getString(key, null) else null

    private fun android.content.SharedPreferences.Editor.putNullableString(
        key: String,
        value: String?
    ): android.content.SharedPreferences.Editor =
        if (value == null) remove(key) else putString(key, value)

    private fun android.content.SharedPreferences.Editor.putNullableLong(
        key: String,
        value: Long?
    ): android.content.SharedPreferences.Editor =
        if (value == null) remove(key) else putLong(key, value)

    companion object {
        /** Claude's store name, kept as the default for the records that predate per-agent stores. */
        const val PREFERENCES_NAME = "verb_session"

        /** Codex's store. A separate file, so no agent's recovery evidence can clobber another's. */
        const val CODEX_PREFERENCES_NAME = "verb_session_codex"

        /** OpenCode's store, same reasoning. */
        const val OPENCODE_PREFERENCES_NAME = "verb_session_opencode"

        private const val SCHEMA_VERSION = 1
        private const val MULTI_SCHEMA_VERSION = 2
        private const val KEY_SESSIONS_JSON = "sessionsJson"
        private const val INVALID_INSTANT = Long.MIN_VALUE
        private const val KEY_SCHEMA_VERSION = "schemaVersion"
        private const val KEY_SESSION_ID = "sessionId"
        private const val KEY_PROJECT_ID = "projectId"
        private const val KEY_RUNTIME_ID = "runtimeId"
        private const val KEY_LAST_KNOWN_CWD = "lastKnownCwd"
        private const val KEY_LAST_OBSERVED_AT = "lastObservedAt"
        private const val KEY_CREATED_AT = "createdAt"
        private const val KEY_LAST_SEEN_AT = "lastSeenAt"
        private const val KEY_STATE = "state"
        private const val KEY_AGENT_TYPE = "agentType"
        private const val KEY_RESUME_IDENTITY = "resumeIdentity"
    }
}
