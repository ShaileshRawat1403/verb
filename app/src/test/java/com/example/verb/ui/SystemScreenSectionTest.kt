package com.example.verb.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import com.example.verb.terminal.TerminalEnvironment
import java.io.File
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SystemScreenSectionTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private val environment = TerminalEnvironment(
        kind = TerminalEnvironment.Kind.ANDROID_SYSTEM_SHELL,
        shellExecutable = "/system/bin/sh",
        arguments = emptyArray(),
        workingDirectory = File("/tmp"),
        variables = emptyArray(),
        rootfsDir = File("/tmp")
    )

    @Test
    fun `a named continuity task brings the continuity section into view`() {
        composeTestRule.setContent {
            SystemScreen(
                isTerminalSessionActive = false,
                terminalEnvironment = environment,
                initialSection = SystemSection.CONTINUITY
            )
        }

        composeTestRule.waitForIdle()
        composeTestRule.onNodeWithTag("system_section_continuity").assertIsDisplayed()
    }
}
