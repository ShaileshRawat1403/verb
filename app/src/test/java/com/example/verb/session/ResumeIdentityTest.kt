package com.example.verb.session

import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ResumeIdentityTest {
    @Test
    fun safe_agent_references_are_accepted() {
        listOf("abc", "01a03261-1a4b-74d0-a892-2ee8de0c839f", "session.1:fork_2")
            .forEach { assertTrue(it, ResumeIdentity.isValid(it)) }
    }

    @Test
    fun shell_syntax_flags_and_unbounded_values_are_rejected() {
        listOf("", "-flag", "; touch owned", "a|b", "`id`", "$(id)", "line\nbreak", "a b")
            .forEach {
                assertFalse(it, ResumeIdentity.isValid(it))
                assertNull(ResumeIdentity.validOrNull(it))
            }
        assertFalse(ResumeIdentity.isValid("a".repeat(129)))
    }
}
