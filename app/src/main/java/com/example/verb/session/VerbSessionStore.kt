package com.example.verb.session

import android.annotation.SuppressLint
import android.content.Context
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
    fun save(session: VerbSession)
    fun clear()
}

/** Small in-memory implementation for coordinator tests and host adapters that own their store. */
class InMemoryVerbSessionStore(initial: VerbSession? = null) : VerbSessionStore {
    private var value: VerbSession? = initial

    override fun load(): VerbSession? = value

    override fun save(session: VerbSession) {
        value = session.copy(process = null)
    }

    override fun clear() {
        value = null
    }
}

/**
 * Android process-death durable implementation backed by app-private SharedPreferences.
 *
 * One store per agent, named by [preferencesName]: each agent tracks its own session, and sharing
 * one record would make launching Codex silently destroy Claude's recovery evidence. The default
 * name is Claude's, unchanged, so records written before Codex existed still load.
 */
class SharedPreferencesVerbSessionStore(
    context: Context,
    preferencesName: String = PREFERENCES_NAME
) : VerbSessionStore {
    private val preferences = context.applicationContext.getSharedPreferences(
        preferencesName,
        Context.MODE_PRIVATE
    )

    override fun load(): VerbSession? {
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
    override fun save(session: VerbSession) {
        // commit() is deliberate: this metadata is the recovery anchor if Android kills the app
        // immediately after the launch or state transition.
        preferences.edit()
            .clear()
            .putInt(KEY_SCHEMA_VERSION, SCHEMA_VERSION)
            .putString(KEY_SESSION_ID, session.id)
            .putNullableString(KEY_PROJECT_ID, session.projectId)
            .putNullableString(KEY_RUNTIME_ID, session.runtime)
            .putNullableString(KEY_LAST_KNOWN_CWD, session.lastKnownCwd)
            .putNullableLong(KEY_LAST_OBSERVED_AT, session.lastObservedAt?.toEpochMilli())
            .putLong(KEY_CREATED_AT, session.createdAt.toEpochMilli())
            .putLong(KEY_LAST_SEEN_AT, session.lastSeenAt.toEpochMilli())
            .putString(KEY_STATE, session.state.name)
            .putNullableString(KEY_AGENT_TYPE, session.agent?.agentType)
            .putNullableString(
                KEY_RESUME_IDENTITY,
                ResumeIdentity.validOrNull(session.agent?.resumeIdentity)
            )
            // commit(), not apply(), and lint's ApplySharedPref advice is wrong here. apply()
            // writes in the background; this record exists to survive the process being killed,
            // which is the exact moment a background write is lost. A session record that did not
            // reach disk is a session Verb cannot prove it ever had.
            .commit()
    }

    @SuppressLint("ApplySharedPref")
    override fun clear() {
        preferences.edit().clear().commit()
    }

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
