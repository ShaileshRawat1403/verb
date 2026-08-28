package com.example.verb.session

import android.content.ContentValues
import android.content.Context
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import com.example.BuildConfig
import com.example.verb.project.VerbProject
import org.json.JSONObject
import java.io.File
import java.security.MessageDigest
import java.time.Instant
import java.time.temporal.ChronoUnit
import java.util.UUID

/** Manual, evidence-only continuity. Imported records never enter a local [VerbSessionStore]. */
object ContinuityArchive {
    const val EXTENSION = "vcont"
    private const val VERSION = 1
    private const val KIND = "verb.continuity"
    private const val MAX_BYTES = 5L * 1024L * 1024L
    private const val MAX_LINE_BYTES = 16 * 1024
    private const val MAX_PROJECTS = 1_000
    private const val MAX_SESSIONS = 1_000
    private const val MAX_EVENTS = 10_000
    private const val STAGED_NAME = "continuity-staged.vcont"

    data class Summary(
        val hostId: String,
        val hostKind: String,
        val exportedAt: String,
        val projects: Int,
        val sessions: Int,
        val events: Int
    ) {
        fun display(): String =
            "$sessions session${if (sessions == 1) "" else "s"} and $events events " +
                "recorded on another $hostKind host (${hostId.take(8)}) at $exportedAt"
    }

    sealed interface Outcome {
        data class Saved(val displayName: String, val summary: Summary) : Outcome
        data class Previewed(val summary: Summary) : Outcome
        data class Imported(val summary: Summary, val replay: Boolean) : Outcome
        data class Failed(val reason: String) : Outcome
    }

    private data class Parsed(
        val bytes: ByteArray,
        val checksum: String,
        val summary: Summary,
        val sessionIds: Set<String>
    )

    fun exportToDownloads(
        context: Context,
        project: VerbProject,
        sessions: Collection<VerbSession>
    ): Outcome {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            return Outcome.Failed("Saving continuity evidence needs Android 10 or newer.")
        }
        return runCatching {
            val bytes = buildEnvelope(context, project, sessions.filter { it.projectId == project.id })
            val name = "verb-continuity-${Instant.now().epochSecond}.$EXTENSION"
            val resolver = context.contentResolver
            val values = ContentValues().apply {
                put(MediaStore.Downloads.DISPLAY_NAME, name)
                put(MediaStore.Downloads.MIME_TYPE, "application/x-verb-continuity")
                put(MediaStore.Downloads.IS_PENDING, 1)
            }
            val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
                ?: return Outcome.Failed("Downloads is not available on this device.")
            resolver.openOutputStream(uri)?.use { it.write(bytes) }
                ?: return Outcome.Failed("Downloads could not be opened for writing.")
            values.clear()
            values.put(MediaStore.Downloads.IS_PENDING, 0)
            resolver.update(uri, values, null, null)
            val parsed = parse(bytes)
            Outcome.Saved("${Environment.DIRECTORY_DOWNLOADS}/$name", parsed.summary)
        }.getOrElse { error -> Outcome.Failed(error.message ?: "Continuity export failed.") }
    }

    /** Validates and stages a user-selected file. No imported evidence changes at preview time. */
    fun previewImport(context: Context, uri: android.net.Uri): Outcome = runCatching {
        val bytes = context.contentResolver.openInputStream(uri)?.use { input ->
            input.readBytes(MAX_BYTES + 1)
        } ?: return Outcome.Failed("That file could not be opened.")
        val parsed = parse(bytes)
        val directory = File(context.filesDir, "verb").apply { mkdirs() }
        atomicWrite(File(directory, STAGED_NAME), parsed.bytes)
        Outcome.Previewed(parsed.summary)
    }.getOrElse { error -> Outcome.Failed(error.message ?: "Continuity preview failed.") }

    /** Applies the already-validated preview into a separate, read-only imported namespace. */
    fun applyPreview(context: Context): Outcome = runCatching {
        val staged = File(context.filesDir, "verb/$STAGED_NAME")
        if (!staged.isFile) return Outcome.Failed("No continuity preview is staged.")
        val parsed = parse(staged.readBytes())
        val destination = File(
            context.filesDir,
            "verb/imported/${parsed.summary.hostId}/${parsed.checksum}.$EXTENSION"
        )
        if (destination.exists()) {
            staged.delete()
            return Outcome.Imported(parsed.summary, replay = true)
        }
        atomicWrite(destination, parsed.bytes)
        staged.delete()
        Outcome.Imported(parsed.summary, replay = false)
    }.getOrElse { error -> Outcome.Failed(error.message ?: "Continuity import failed.") }

    fun importedSessionCount(filesDir: File): Int =
        File(filesDir, "verb/imported").walkTopDown()
            .filter { it.isFile && it.extension == EXTENSION }
            .mapNotNull { runCatching { parse(it.readBytes()) }.getOrNull() }
            .flatMap { parsed -> parsed.sessionIds.asSequence().map { parsed.summary.hostId to it } }
            .toSet()
            .size

    internal fun buildForTest(
        context: Context,
        project: VerbProject,
        sessions: Collection<VerbSession>
    ): ByteArray = buildEnvelope(context, project, sessions)

    internal fun validateForTest(bytes: ByteArray): Summary = parse(bytes).summary

    private fun buildEnvelope(
        context: Context,
        project: VerbProject,
        sessions: Collection<VerbSession>
    ): ByteArray {
        val projectKey = projectKey(project.directory)
        val payload = buildString {
            appendLine(JSONObject().apply {
                put("recordType", "origin")
                put("hostId", hostId(context))
                put("hostKind", "android")
                // The build's own version, never a literal maintained beside it. A hardcoded
                // "0.1.0-beta.2" survived the beta.3 release and stamped every exported archive
                // with a version that had already shipped -- provenance is the one field in this
                // record that must not be able to drift from the artifact that wrote it.
                put("verbVersion", BuildConfig.VERSION_NAME)
                put("exportedAt", timestamp(Instant.now()))
            })
            appendLine(JSONObject().apply {
                put("recordType", "project")
                put("projectKey", projectKey)
                put("label", project.id.take(128))
            })
            sessions.forEach { session ->
                appendLine(JSONObject().apply {
                    put("recordType", "session")
                    put("sessionId", session.id)
                    put("projectKey", projectKey)
                    put("runtimeId", session.runtime ?: JSONObject.NULL)
                    put("agentType", session.agent?.agentType ?: JSONObject.NULL)
                    put(
                        "resumeIdentityRef",
                        ResumeIdentity.validOrNull(session.agent?.resumeIdentity) ?: JSONObject.NULL
                    )
                    put("createdAt", timestamp(session.createdAt))
                    put("lastSeenAt", timestamp(session.lastSeenAt))
                    put("lastObservedAt", session.lastObservedAt?.let(::timestamp) ?: JSONObject.NULL)
                    put("cwdRelative", relativePath(project.directory, session.lastKnownCwd) ?: JSONObject.NULL)
                    put("recordedState", session.state.name)
                    put("recordedStateAt", timestamp(session.lastSeenAt))
                })
            }
            val events = VerbEventLog(context.filesDir)
            sessions.forEach { session ->
                events.continuityRecords(session.id).forEach(::appendLine)
            }
        }.toByteArray(Charsets.UTF_8)
        val header = JSONObject().apply {
            put("recordType", "header")
            put("envelopeVersion", VERSION)
            put("kind", KIND)
            put("payloadSha256", sha256(payload))
        }.toString().toByteArray(Charsets.UTF_8)
        return header + byteArrayOf('\n'.code.toByte()) + payload
    }

    private fun parse(bytes: ByteArray): Parsed {
        require(bytes.size.toLong() <= MAX_BYTES) { "Continuity file exceeds the 5 MiB limit." }
        val newline = bytes.indexOf('\n'.code.toByte())
        require(newline in 1..MAX_LINE_BYTES) { "Continuity header is incomplete." }
        val header = JSONObject(bytes.copyOfRange(0, newline).toString(Charsets.UTF_8))
        requireKeys(header, HEADER_KEYS)
        require(header.getString("recordType") == "header") { "Invalid continuity header." }
        require(header.getInt("envelopeVersion") == VERSION && header.getString("kind") == KIND) {
            "Unsupported continuity envelope; expected $KIND version $VERSION."
        }
        val payload = bytes.copyOfRange(newline + 1, bytes.size)
        val checksum = header.getString("payloadSha256")
        require(checksum.matches(Regex("[0-9a-f]{64}")) && sha256(payload) == checksum) {
            "Continuity checksum does not match; nothing was imported."
        }

        var origin: JSONObject? = null
        var projects = 0
        var sessions = 0
        var events = 0
        val projectKeys = mutableSetOf<String>()
        val sessionIds = mutableSetOf<String>()
        val eventIds = mutableSetOf<Pair<String, Long>>()
        val payloadText = payload.toString(Charsets.UTF_8)
        require(payloadText.endsWith('\n')) { "Continuity payload is not canonical JSON Lines." }
        val records = payloadText.dropLast(1).split('\n')
        require(records.none(String::isEmpty)) { "Continuity payload contains a blank record." }
        val lastEventSequence = mutableMapOf<String, Long>()
        records.forEachIndexed { index, line ->
            require(line.toByteArray().size <= MAX_LINE_BYTES) { "Continuity record is too large." }
            val record = JSONObject(line)
            when (record.optString("recordType")) {
                "origin" -> {
                    require(index == 0 && origin == null) { "Origin must be the first payload record." }
                    requireKeys(record, ORIGIN_KEYS)
                    require(record.getString("hostId").matches(Regex("[0-9a-fA-F]{32}"))) {
                        "Invalid continuity host identity."
                    }
                    require(record.getString("hostKind") in setOf("android", "desktop")) {
                        "Invalid continuity host kind."
                    }
                    requireDisplay(record.getString("verbVersion"), 64)
                    requireTimestamp(record.getString("exportedAt"))
                    origin = record
                }
                "project" -> {
                    requireKeys(record, PROJECT_KEYS)
                    require(validProjectKey(record.getString("projectKey"))) { "Invalid continuity project key." }
                    requireDisplay(record.getString("label"), 128)
                    require(projectKeys.add(record.getString("projectKey"))) {
                        "Duplicate continuity project identity."
                    }
                    projects++
                    require(projects <= MAX_PROJECTS) { "Too many continuity projects." }
                }
                "session" -> {
                    requireKeys(record, SESSION_KEYS)
                    validateSession(record)
                    require(record.getString("projectKey") in projectKeys) {
                        "Continuity session refers to an unknown project."
                    }
                    require(sessionIds.add(record.getString("sessionId"))) {
                        "Duplicate continuity session identity."
                    }
                    sessions++
                    require(sessions <= MAX_SESSIONS) { "Too many continuity sessions." }
                }
                "event" -> {
                    requireKeys(record, EVENT_KEYS)
                    validateEvent(record)
                    val sessionId = record.getString("sessionId")
                    val sequence = record.getLong("seq")
                    val eventId = sessionId to sequence
                    require(sessionId in sessionIds && eventIds.add(eventId)) {
                        "Continuity event has an unknown or conflicting identity."
                    }
                    require(sequence > (lastEventSequence[sessionId] ?: 0L)) {
                        "Continuity event sequence is not increasing."
                    }
                    lastEventSequence[sessionId] = sequence
                    events++
                    require(events <= MAX_EVENTS) { "Too many continuity events." }
                }
                else -> error("Unknown continuity record type.")
            }
        }
        val originRecord = requireNotNull(origin) { "Continuity payload has no origin." }
        val summary = Summary(
            hostId = originRecord.getString("hostId").lowercase(),
            hostKind = originRecord.getString("hostKind"),
            exportedAt = originRecord.getString("exportedAt").also(::requireTimestamp),
            projects = projects,
            sessions = sessions,
            events = events
        )
        return Parsed(bytes, checksum, summary, sessionIds)
    }

    private fun validateSession(record: JSONObject) {
        requireOpaque(record.getString("sessionId"), 128)
        require(validProjectKey(record.getString("projectKey")))
        nullableString(record, "runtimeId")?.let { requireOpaque(it, 64) }
        nullableString(record, "agentType")?.let { requireOpaque(it, 64) }
        nullableString(record, "resumeIdentityRef")?.let {
            require(ResumeIdentity.validOrNull(it) != null) { "Invalid resume identity reference." }
        }
        requireTimestamp(record.getString("createdAt"))
        requireTimestamp(record.getString("lastSeenAt"))
        nullableString(record, "lastObservedAt")?.let(::requireTimestamp)
        nullableString(record, "cwdRelative")?.let { require(validRelative(it)) }
        require(record.getString("recordedState") in STATE_VALUES) { "Invalid recorded state." }
        requireTimestamp(record.getString("recordedStateAt"))
    }

    private fun validateEvent(record: JSONObject) {
        requireOpaque(record.getString("sessionId"), 128)
        require(record.getLong("seq") > 0) { "Invalid event sequence." }
        require(record.getString("eventType") in EVENT_TYPES) { "Unknown continuity event." }
        requireTimestamp(record.getString("recordedAt"))
        if (!record.isNull("exitCode")) record.getInt("exitCode")
        nullableString(record, "commandId")?.let { requireOpaque(it, 128) }
        nullableString(record, "cwdRelative")?.let { require(validRelative(it)) }
        nullableString(record, "state")?.let {
            require(it in STATE_VALUES) { "Invalid continuity event state." }
        }
        nullableString(record, "tool")?.let { require(it.length <= 128 && it.none(Char::isISOControl)) }
        require(record.getString("source") in setOf("shell", "agentRecord", "verb")) {
            "Invalid event source."
        }
    }

    private fun requireKeys(record: JSONObject, allowed: Set<String>) {
        val keys = record.keys().asSequence().toSet()
        require(keys == allowed) { "Unexpected or missing continuity field." }
    }

    private fun nullableString(record: JSONObject, key: String): String? =
        if (record.isNull(key)) null else record.getString(key)

    private fun requireOpaque(value: String, max: Int) {
        require(value.length in 1..max && value.matches(Regex("[A-Za-z0-9][A-Za-z0-9._:-]*"))) {
            "Invalid opaque continuity identifier."
        }
    }

    private fun requireDisplay(value: String, max: Int) {
        require(value.length in 1..max && value.none(Char::isISOControl)) {
            "Invalid continuity display value."
        }
    }

    private fun requireTimestamp(value: String) {
        require(runCatching { Instant.parse(value) }.isSuccess) { "Invalid continuity timestamp." }
    }

    private fun validProjectKey(value: String): Boolean =
        value == "unresolved" ||
            (value.startsWith("git:") && value.length <= 512 && ".." !in value && value.none(Char::isISOControl))

    private fun validRelative(value: String): Boolean =
        value.length <= 512 && !value.startsWith('/') && '\u0000' !in value &&
            value.split('/').none { it == ".." }

    private fun projectKey(project: File): String {
        val config = File(project, ".git/config").takeIf(File::isFile)?.readText() ?: return "unresolved"
        val remote = Regex(
            "(?ms)^\\s*\\[remote\\s+\"origin\"]\\s*$.*?^\\s*url\\s*=\\s*(\\S+)"
        ).find(config)?.groupValues?.get(1) ?: return "unresolved"
        return normalizeRemote(remote)?.let { "git:$it" } ?: "unresolved"
    }

    private fun normalizeRemote(value: String): String? {
        if (value.isBlank() || value.any(Char::isISOControl)) return null
        val withoutScheme = value.substringAfter("://", value)
        val withoutUser = withoutScheme.substringAfterLast('@')
        var normalized = if ("://" !in value && ':' in withoutUser) {
            withoutUser.replaceFirst(':', '/')
        } else withoutUser
        normalized = normalized.substringBefore('?').substringBefore('#').trim('/').removeSuffix(".git")
        val host = normalized.substringBefore('/').substringBefore(':').lowercase()
        val path = normalized.substringAfter('/', "")
        return "$host/$path".takeIf { host.isNotBlank() && path.isNotBlank() && ".." !in it }
    }

    private fun relativePath(project: File, cwd: String?): String? {
        if (cwd == null) return null
        val root = project.canonicalFile.path
        val path = File(cwd).canonicalFile.path
        if (path == root) return ""
        val rootPrefix = root.trimEnd(File.separatorChar) + File.separatorChar
        if (!path.startsWith(rootPrefix)) return null
        return path.removePrefix(rootPrefix).replace(File.separatorChar, '/').takeIf(::validRelative)
    }

    private fun hostId(context: Context): String {
        val preferences = context.getSharedPreferences("verb_continuity", Context.MODE_PRIVATE)
        preferences.getString("host_id", null)?.takeIf { it.matches(Regex("[0-9a-f]{32}")) }?.let { return it }
        val created = UUID.randomUUID().toString().replace("-", "")
        check(preferences.edit().putString("host_id", created).commit()) { "Could not persist host identity." }
        return created
    }

    private fun timestamp(value: Instant): String = value.truncatedTo(ChronoUnit.SECONDS).toString()

    private fun sha256(bytes: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

    private fun atomicWrite(destination: File, bytes: ByteArray) {
        val parent = requireNotNull(destination.parentFile)
        check(parent.isDirectory || parent.mkdirs()) { "Could not create continuity storage." }
        val temporary = File(parent, ".${destination.name}.tmp")
        temporary.outputStream().use { output ->
            output.write(bytes)
            output.fd.sync()
        }
        check(temporary.renameTo(destination)) { "Could not commit continuity evidence." }
    }

    private fun java.io.InputStream.readBytes(limit: Long): ByteArray {
        val output = java.io.ByteArrayOutputStream()
        val buffer = ByteArray(8 * 1024)
        var total = 0L
        while (true) {
            val count = read(buffer)
            if (count < 0) break
            total += count
            require(total <= limit) { "Continuity file exceeds the 5 MiB limit." }
            output.write(buffer, 0, count)
        }
        return output.toByteArray()
    }

    private val HEADER_KEYS = setOf("recordType", "envelopeVersion", "kind", "payloadSha256")
    private val ORIGIN_KEYS = setOf("recordType", "hostId", "hostKind", "verbVersion", "exportedAt")
    private val PROJECT_KEYS = setOf("recordType", "projectKey", "label")
    private val SESSION_KEYS = setOf(
        "recordType", "sessionId", "projectKey", "runtimeId", "agentType", "resumeIdentityRef",
        "createdAt", "lastSeenAt", "lastObservedAt", "cwdRelative", "recordedState", "recordedStateAt"
    )
    private val EVENT_KEYS = setOf(
        "recordType", "sessionId", "seq", "eventType", "recordedAt", "exitCode", "commandId",
        "cwdRelative", "state", "tool", "source"
    )
    private val STATE_VALUES = setOf("LIVE", "INTERRUPTED", "RECOVERABLE", "ENDED")
    private val EVENT_TYPES = setOf(
        "SESSION_STARTED", "SESSION_STATE_CHANGED", "PROCESS_STARTED", "PROCESS_ENDED",
        "AGENT_STARTED", "AGENT_ENDED", "COMMAND_STARTED", "COMMAND_FINISHED", "CWD_CHANGED",
        "RUNTIME_CHANGED", "RECOVERY_CHECKED", "SESSION_ENDED", "AGENT_TURN_STARTED",
        "AGENT_TOOL_CALLED", "AGENT_TOOL_SUCCEEDED", "AGENT_TOOL_FAILED", "AGENT_TURN_FINISHED"
    )
}
