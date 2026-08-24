package com.example.verb.session

import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.time.Instant
import java.time.temporal.ChronoUnit

/**
 * Android structural execution memory. The API has no parameter capable of carrying command text,
 * PTY bytes, prompts, transcript content, credentials, or paths.
 */
class VerbEventLog(private val filesDir: File?) {
    @Synchronized
    fun append(
        session: VerbSession,
        type: String,
        exitCode: Int? = null,
        state: VerbSessionState? = null
    ) {
        val root = filesDir ?: return
        require(type in EVENT_TYPES) { "Unknown Verb event type" }
        require(session.id.matches(Regex("[A-Za-z0-9][A-Za-z0-9._:-]{0,127}"))) {
            "Unsafe session identity"
        }
        val file = File(root, "verb/events/${session.id}.jsonl")
        check(file.parentFile?.let { it.isDirectory || it.mkdirs() } == true)
        val seq = file.takeIf(File::isFile)?.useLines { lines ->
            lines.mapNotNull { line ->
                SEQ_PATTERN.find(line)?.groupValues?.get(1)?.toLongOrNull()?.takeIf { it > 0 }
            }.maxOrNull()
        }?.plus(1) ?: 1L
        val optional = buildString {
            exitCode?.let { append(",\"exitCode\":$it") }
            state?.let { append(",\"state\":\"${it.name}\"") }
        }
        val record = "{\"schemaVersion\":1," +
            "\"timestamp\":\"${Instant.now().truncatedTo(ChronoUnit.SECONDS)}\"," +
            "\"sessionId\":\"${session.id}\",\"seq\":$seq,\"type\":\"$type\"," +
            "\"source\":\"verb\"$optional}"
        FileOutputStream(file, true).use { output ->
            output.write((record + "\n").toByteArray(Charsets.UTF_8))
            output.fd.sync()
        }
    }

    fun continuityRecords(sessionId: String): List<JSONObject> {
        val root = filesDir ?: return emptyList()
        if (!sessionId.matches(Regex("[A-Za-z0-9][A-Za-z0-9._:-]{0,127}"))) return emptyList()
        val file = File(root, "verb/events/$sessionId.jsonl")
        if (!file.isFile) return emptyList()
        return file.useLines { lines ->
            lines.mapNotNull { line ->
                runCatching {
                    val source = JSONObject(line)
                    if (source.getString("type") !in EVENT_TYPES) return@runCatching null
                    JSONObject().apply {
                        put("recordType", "event")
                        put("sessionId", source.getString("sessionId"))
                        put("seq", source.getLong("seq"))
                        put("eventType", source.getString("type"))
                        put("recordedAt", source.getString("timestamp"))
                        put("exitCode", if (source.has("exitCode")) source.getInt("exitCode") else JSONObject.NULL)
                        put("commandId", JSONObject.NULL)
                        put("cwdRelative", JSONObject.NULL)
                        put("state", if (source.has("state")) source.getString("state") else JSONObject.NULL)
                        put("tool", JSONObject.NULL)
                        put("source", "verb")
                    }
                }.getOrNull()
            }.filterNotNull().toList()
        }
    }

    companion object {
        val Disabled = VerbEventLog(null)

        private val EVENT_TYPES = setOf(
            "SESSION_STARTED", "SESSION_STATE_CHANGED", "PROCESS_STARTED", "PROCESS_ENDED",
            "AGENT_STARTED", "AGENT_ENDED", "RECOVERY_CHECKED", "SESSION_ENDED"
        )
        private val SEQ_PATTERN = Regex("\\\"seq\\\":(\\d+)")
    }
}
