package com.example.verb.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class DesktopPairingLinkTest {
    private val valid = "verb://pair#host=192.168.1.8&port=42000&pin=${"a".repeat(64)}&code=${"b".repeat(32)}"

    @Test fun parsesAnExactPairingLink() {
        val link = DesktopPairingLink.parse(valid)
        assertEquals("192.168.1.8", link.host)
        assertEquals(42000, link.port)
    }

    @Test fun rejectsChangedOriginAndAmbiguousOrInvalidParameters() {
        for (bad in listOf(
            valid.replace("verb://pair", "http://pair"),
            "$valid&code=${"c".repeat(32)}",
            valid.replace("192.168.1.8", "example.com"),
            valid.replace("192.168.1.8", "999.168.1.8"),
            valid.replace("port=42000", "port=0"),
            valid.replace("pin=${"a".repeat(64)}", "pin=abcd")
        )) assertThrows(Exception::class.java) { DesktopPairingLink.parse(bad) }
    }
}
