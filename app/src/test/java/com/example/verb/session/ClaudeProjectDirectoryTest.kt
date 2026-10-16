package com.example.verb.session

import org.junit.Assert.assertEquals
import org.junit.Test

class ClaudeProjectDirectoryTest {
    @Test
    fun mapping_matches_the_installed_claude_layout_and_desktop() {
        assertEquals(
            "-tmp-Verb-Transfer-v1",
            ClaudeProjectDirectory.encode("/tmp/Verb_Transfer.v1")
        )
        assertEquals(
            "-Users-apple-My-Project-2-x",
            ClaudeProjectDirectory.encode("/Users/apple/My Project@2+x")
        )
        assertEquals(
            "-home--------x",
            ClaudeProjectDirectory.encode("/home/ü/项目/🚀x")
        )
        val longPath = "/${"a".repeat(250)}/b c"
        assertEquals(
            "-${"a".repeat(199)}-lv1bdn",
            ClaudeProjectDirectory.encode(longPath)
        )
    }
}
