package com.example.verb.mobile

import com.example.verb.model.ChatMessage
import com.example.verb.model.ChatSender
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedInputStream
import java.net.Socket
import java.net.URI
import java.net.URLDecoder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLSocket
import javax.net.ssl.X509TrustManager

/** A one-use offer. The certificate pin and pairing code travel together, outside the TLS channel. */
data class DesktopPairingLink(val host: String, val port: Int, val pin: String, val code: String) {
    companion object {
        fun parse(text: String): DesktopPairingLink {
            val uri = URI(text.trim())
            require(uri.scheme == "verb" && uri.host == "pair" && uri.path.isNullOrEmpty()) {
                "Paste a Verb desktop pairing link."
            }
            val parameters = uri.rawFragment?.split('&')?.associate { part ->
                val pair = part.split('=', limit = 2)
                require(pair.size == 2) { "Incomplete pairing link." }
                URLDecoder.decode(pair[0], "UTF-8") to URLDecoder.decode(pair[1], "UTF-8")
            } ?: error("Incomplete pairing link.")
            require(parameters.size == uri.rawFragment.split('&').size &&
                parameters.keys == setOf("host", "port", "pin", "code")) {
                "Invalid pairing link."
            }
            val host = parameters.getValue("host")
            require(host.matches(Regex("(?:[0-9]{1,3}\\.){3}[0-9]{1,3}")) &&
                host.split('.').all { part -> part.toIntOrNull()?.let { it in 0..255 } == true }) {
                "Invalid desktop address."
            }
            val port = parameters.getValue("port").toIntOrNull()
            require(port != null && port in 1..65535) { "Invalid desktop port." }
            val pin = parameters.getValue("pin")
            val code = parameters.getValue("code")
            require(pin.matches(Regex("[0-9a-f]{64}")) && code.matches(Regex("[0-9a-f]{32}"))) {
                "Invalid pairing code or certificate pin."
            }
            return DesktopPairingLink(host, port, pin, code)
        }
    }
}

data class DesktopScreen(
    val sessionId: String,
    val revision: Long,
    val text: String,
    val controller: String,
    val agent: String? = null,
    val agentState: String = "waiting",
    val canSendPrompt: Boolean = true,
    val messages: List<ChatMessage> = emptyList()
)

/** Bounded JSON frames over a reusable pinned TLS connection, with one-shot server compatibility. */
class DesktopBridgeClient(private val link: DesktopPairingLink) {
    private val trustManager = object : X509TrustManager {
        override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
        override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) {
            throw CertificateException("Verb Mobile does not trust client certificates.")
        }
        override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {
            if (chain.size != 1) throw CertificateException("Unexpected desktop certificate chain.")
            chain[0].checkValidity()
            val hostMatches = chain[0].subjectAlternativeNames?.any { entry ->
                entry.size >= 2 && entry[0] == 7 && entry[1] == link.host
            } == true
            if (!hostMatches) throw CertificateException("Desktop address does not match its certificate.")
            val actual = MessageDigest.getInstance("SHA-256").digest(chain[0].encoded)
                .joinToString("") { "%02x".format(it) }
            if (!MessageDigest.isEqual(actual.toByteArray(StandardCharsets.US_ASCII),
                link.pin.toByteArray(StandardCharsets.US_ASCII)))
                throw CertificateException("Desktop identity changed. Create a new pairing link.")
        }
    }

    private val context = SSLContext.getInstance("TLS").apply {
        init(null, arrayOf(trustManager), null)
    }

    private var connection: SSLSocket? = null
    private var input: BufferedInputStream? = null
    private var lastReplyAt = 0L

    @Synchronized
    fun close() {
        runCatching { connection?.close() }
        connection = null
        input = null
    }

    private fun connect(): SSLSocket {
        // The host expires idle connections after three seconds. Start fresh before that boundary.
        if (System.nanoTime() - lastReplyAt >= 2_000_000_000L) close()
        connection?.let { return it }
        val base = Socket()
        try {
            base.connect(java.net.InetSocketAddress(link.host, link.port), 3000)
            base.soTimeout = 3000
            val socket = context.socketFactory.createSocket(base, link.host, link.port, true) as SSLSocket
            connection = socket
            socket.soTimeout = 3000
            socket.sslParameters = socket.sslParameters.apply { endpointIdentificationAlgorithm = "HTTPS" }
            socket.startHandshake()
            input = BufferedInputStream(socket.inputStream)
            return socket
        } catch (error: Exception) {
            close()
            runCatching { base.close() }
            throw error
        }
    }

    @Synchronized
    private fun request(op: String, secret: String, bytes: ByteArray? = null): JSONObject {
        val payload = JSONObject().put("version", 1).put("op", op).put("secret", secret)
        if (bytes != null) {
            require(bytes.size <= 4096) { "Input is too long." }
            payload.put("bytes", JSONArray().apply { bytes.forEach { put(it.toInt() and 255) } })
        }
        val raw = (payload.toString() + "\n").toByteArray(StandardCharsets.UTF_8)
        require(raw.size <= 32 * 1024) { "Request is too long." }
        try {
            val socket = connect()
            socket.outputStream.write(raw)
            socket.outputStream.flush()
            val response = java.io.ByteArrayOutputStream()
            val reader = checkNotNull(input)
            while (response.size() <= 2 * 1024 * 1024) {
                val next = reader.read()
                require(next >= 0) { "Desktop closed the connection." }
                if (next == 10) break
                response.write(next)
            }
            require(response.size() <= 2 * 1024 * 1024) { "Desktop reply is too large." }
            val envelope = JSONObject(response.toString("UTF-8"))
            require(envelope.optInt("version") == 1) { "Unsupported desktop protocol." }
            require(envelope.optBoolean("ok")) { envelope.optString("error", "Desktop rejected this request.") }
            val result = envelope.getJSONObject("result")
            lastReplyAt = System.nanoTime()
            if (!envelope.optBoolean("keepAlive") || op == "disconnect") close()
            return result
        } catch (error: Exception) {
            close()
            // Never replay input/take/pair: a lost reply does not prove the host did not apply it.
            throw error
        }
    }

    fun pair(): String = request("pair", link.code).getString("deviceToken")
    fun reconnect(token: String) { request("reconnect", token) }
    fun disconnect(token: String) { request("disconnect", token) }
    fun take(token: String) { request("take", token) }
    fun input(token: String, bytes: ByteArray) { request("input", token, bytes) }
    fun snapshot(token: String): DesktopScreen {
        val result = request("snapshot", token)
        val array = result.optJSONArray("bytes")
        val bytes = array?.let { ByteArray(it.length()) { index -> it.getInt(index).toByte() } }
        val agent = result.optString("agent").takeIf { it.isNotEmpty() && it != "null" }
        val agentState = result.optString("agentState", "waiting")
        val canSendPrompt = result.optBoolean("canSendPrompt", true)
        val msgs = mutableListOf<ChatMessage>()
        val msgArray = result.optJSONArray("messages")
        if (msgArray != null) {
            for (i in 0 until msgArray.length()) {
                val item = msgArray.optJSONObject(i) ?: continue
                val sender = if (item.optString("sender") == "user") ChatSender.USER else ChatSender.AGENT
                msgs.add(
                    ChatMessage(
                        id = item.optString("id", java.util.UUID.randomUUID().toString()),
                        sender = sender,
                        text = item.optString("text", ""),
                        timestamp = item.optLong("timestamp", 0L)
                    )
                )
            }
        }
        return DesktopScreen(
            sessionId = result.getString("sessionId"),
            revision = result.getLong("revision"),
            text = bytes?.toString(StandardCharsets.UTF_8) ?: "Screen unavailable.",
            controller = result.getString("controller"),
            agent = agent,
            agentState = agentState,
            canSendPrompt = canSendPrompt,
            messages = msgs
        )
    }
}
