package com.example.verb.viewmodel

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ProfileInstallProtocolTest {
    private val marker = "__VERB_PROFILE_JAVASCRIPT_12345__"

    @Test
    fun `typed command never contains the full completion marker`() {
        val command = ProfileInstallProtocol.command("pkg install -y nodejs-lts npm", marker)

        assertFalse(command.contains(marker))
        assertTrue(command.contains("verb_marker="))
        assertTrue(command.contains("profile_status"))
    }

    @Test
    fun `parser waits for numeric output and reads the newest complete record`() {
        assertNull(ProfileInstallProtocol.exitCode("install still running", marker))
        assertNull(ProfileInstallProtocol.exitCode("$marker:not-yet-a-code", marker))
        assertEquals(1, ProfileInstallProtocol.exitCode("$marker:0\n$marker:1\n", marker))
        assertEquals(0, ProfileInstallProtocol.exitCode("output\r\n$marker:0\r\n~ ${'$'}", marker))
    }
}
