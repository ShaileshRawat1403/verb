package com.example.verb.mobile

import okhttp3.tls.HeldCertificate
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.net.SocketException
import java.security.KeyStore
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.concurrent.CopyOnWriteArrayList
import javax.net.ssl.KeyManagerFactory
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLServerSocket
import javax.net.ssl.SSLSocket

@RunWith(RobolectricTestRunner::class)
class DesktopBridgeClientTest {
    private class TestDesktop(certificateHost: String, private val replies: List<String>) : AutoCloseable {
        val certificate = HeldCertificate.Builder()
            .commonName("Verb test desktop")
            .addSubjectAlternativeName(certificateHost)
            .build()
        val pin = MessageDigest.getInstance("SHA-256").digest(certificate.certificate.encoded)
            .joinToString("") { "%02x".format(it) }
        private val keyStore = KeyStore.getInstance(KeyStore.getDefaultType()).apply {
            load(null)
            setKeyEntry("test", certificate.keyPair.private, charArrayOf(),
                arrayOf(certificate.certificate))
        }
        private val managers = KeyManagerFactory.getInstance(KeyManagerFactory.getDefaultAlgorithm()).apply {
            init(keyStore, charArrayOf())
        }
        private val context = SSLContext.getInstance("TLS").apply {
            init(managers.keyManagers, null, SecureRandom())
        }
        private val server = (context.serverSocketFactory.createServerSocket(0) as SSLServerSocket).apply {
            soTimeout = 5000
        }
        val port: Int = server.localPort
        val received = CopyOnWriteArrayList<JSONObject>()
        private val worker = Thread {
            try {
                repeat(maxOf(1, replies.size)) { index ->
                    (server.accept() as SSLSocket).use { socket ->
                        socket.soTimeout = 3000
                        val line = socket.inputStream.bufferedReader().readLine()
                        received.add(JSONObject(line))
                        replies.getOrNull(index)?.let { reply ->
                            socket.outputStream.write((reply + "\n").toByteArray())
                            socket.outputStream.flush()
                        }
                    }
                }
            } catch (_: Exception) {
                // Negative tests deliberately abort the handshake before a request is sent.
            }
        }.apply { isDaemon = true; start() }

        fun link(pin: String = this.pin): DesktopPairingLink = DesktopPairingLink(
            host = "127.0.0.1", port = port, pin = pin, code = "a".repeat(32))

        override fun close() {
            try { server.close() } catch (_: SocketException) { }
            worker.join(5000)
            assertTrue("test TLS server thread did not stop", !worker.isAlive)
        }
    }

    @Test fun pairsAndReadsTheLiveScreenOverPinnedTls() {
        TestDesktop("127.0.0.1", listOf(
            """{"version":1,"ok":true,"result":{"deviceToken":"${"b".repeat(32)}"}}""",
            """{"version":1,"ok":true,"result":{"sessionId":"chosen-session","revision":2,"bytes":[104,105],"controller":"desktop"}}"""
        )).use { desktop ->
            val client = DesktopBridgeClient(desktop.link())
            val token = client.pair()
            val screen = client.snapshot(token)
            assertEquals("b".repeat(32), token)
            assertEquals("chosen-session", screen.sessionId)
            assertEquals("hi", screen.text)
            assertEquals("desktop", screen.controller)
            assertEquals("pair", desktop.received[0].getString("op"))
            assertEquals("a".repeat(32), desktop.received[0].getString("secret"))
            assertEquals("snapshot", desktop.received[1].getString("op"))
            assertEquals(token, desktop.received[1].getString("secret"))
        }
    }

    @Test fun refusesWrongPinBeforeSendingThePairingCode() {
        TestDesktop("127.0.0.1", emptyList()).use { desktop ->
            assertThrows(Exception::class.java) {
                DesktopBridgeClient(desktop.link("0".repeat(64))).pair()
            }
            assertTrue(desktop.received.isEmpty())
        }
    }

    @Test fun refusesCertificateForAnotherAddressBeforeSendingThePairingCode() {
        TestDesktop("192.168.1.8", emptyList()).use { desktop ->
            assertThrows(Exception::class.java) { DesktopBridgeClient(desktop.link()).pair() }
            assertTrue(desktop.received.isEmpty())
        }
    }
}
