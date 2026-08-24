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
    }
}
