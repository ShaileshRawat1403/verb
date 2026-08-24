package com.example.verb.privacy

import org.junit.Assert.assertFalse
import org.junit.Test
import java.io.File

class DurableStoreBoundaryTest {
    @Test
    fun android_main_has_no_prohibited_text_store_capability() {
        val root = File("src/main")
        val prohibited = listOf(
            "recordTerminalOutput",
            "saveChatMessage",
            "terminal_outputs",
            "chat_messages",
            "command_history",
            "AgentMemoryStore"
        )
        val offenders = root.walkTopDown()
            .filter { it.isFile && it.extension in setOf("kt", "java", "xml") }
            .flatMap { file ->
                val text = file.readText()
                prohibited.asSequence().filter(text::contains).map { marker -> "${file.path}: $marker" }
            }
            .toList()

        assertFalse("Prohibited durable text stores remain:\n${offenders.joinToString("\n")}", offenders.isNotEmpty())
    }
}
