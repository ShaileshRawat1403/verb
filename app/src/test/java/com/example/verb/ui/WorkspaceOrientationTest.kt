package com.example.verb.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.example.verb.project.VerbProject
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * "Where was I?" -- answered on screen rather than remembered by the user.
 *
 * The header chip drops the project name the moment a second terminal claims the row's width, and
 * it never named the terminal at all. These tests pin the two things that replaced it: a line that
 * always says both, and one surface that lists projects and terminals together.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class WorkspaceOrientationTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private val projects = listOf(
        VerbProject("mobile-kit-30603ae7", File("/projects/mobile-kit-30603ae7")),
        VerbProject("verb-docs-a1b2c3d4", File("/projects/verb-docs-a1b2c3d4"))
    )

    @Test
    fun `context bar names the project and the terminal the header cannot`() {
        composeTestRule.setContent {
            WorkspaceContextBar(
                projectLabel = projects[0].displayName,
                terminalLabel = "Terminal 2",
                occupant = "Codex",
                onClick = {}
            )
        }

        composeTestRule.onNodeWithTag("workspace_context_bar").assertIsDisplayed()
        composeTestRule.onNodeWithText("mobile-kit").assertIsDisplayed()
        composeTestRule.onNodeWithText("Terminal 2  ·  Codex running").assertIsDisplayed()
    }

    /**
     * A project with no terminal and no name must still say so. A blank line reads as a bug, and
     * a workspace that shows nothing is the state a person is most likely to be lost in.
     */
    @Test
    fun `context bar states absence rather than rendering blank`() {
        composeTestRule.setContent {
            WorkspaceContextBar(
                projectLabel = null,
                terminalLabel = null,
                occupant = null,
                onClick = {}
            )
        }

        composeTestRule.onNodeWithText("No project").assertIsDisplayed()
        composeTestRule.onNodeWithText("No terminal open").assertIsDisplayed()
    }

    @Test
    fun `workspace sheet lists terminals and projects together`() {
        composeTestRule.setContent {
            WorkspaceSheetContent(
                projects = projects,
                selectedProject = projects[0],
                terminalSessionIds = listOf("terminal-1", "terminal-2"),
                activeTerminalSessionId = "terminal-1",
                agentInTerminal = { if (it == "terminal-2") "Codex" else null },
                canOpenMoreTerminals = true,
                onCreateProject = { true },
                onSelectProject = {},
                onSwitchTerminal = {},
                onOpenTerminal = {},
                onCloseTerminal = {}
            )
        }

        composeTestRule.onNodeWithTag("workspace_terminal_terminal-1").assertExists()
        composeTestRule.onNodeWithTag("workspace_terminal_terminal-2").assertExists()
        composeTestRule.onNodeWithText("Codex is running here").assertExists()
        // The typed name, not the generated id: recognising your own project is the whole point.
        composeTestRule.onNodeWithText("mobile-kit").assertExists()
        composeTestRule.onNodeWithText("verb-docs").assertExists()
    }

    @Test
    fun `switching a terminal from the workspace reports the session that was chosen`() {
        var switched: String? = null
        composeTestRule.setContent {
            WorkspaceSheetContent(
                projects = projects,
                selectedProject = projects[0],
                terminalSessionIds = listOf("terminal-1", "terminal-2"),
                activeTerminalSessionId = "terminal-1",
                agentInTerminal = { null },
                canOpenMoreTerminals = true,
                onCreateProject = { true },
                onSelectProject = {},
                onSwitchTerminal = { switched = it },
                onOpenTerminal = {},
                onCloseTerminal = {}
            )
        }

        composeTestRule.onNodeWithTag("workspace_terminal_terminal-2").performClick()

        assertEquals("terminal-2", switched)
    }

    /**
     * The project already in front is not a switch target. Selecting it would restart a session
     * for no reason, which on a phone means losing whatever was running in it.
     */
    @Test
    fun `the current project is not selectable`() {
        var selected: String? = null
        composeTestRule.setContent {
            WorkspaceSheetContent(
                projects = projects,
                selectedProject = projects[0],
                terminalSessionIds = listOf("terminal-1"),
                activeTerminalSessionId = "terminal-1",
                agentInTerminal = { null },
                canOpenMoreTerminals = true,
                onCreateProject = { true },
                onSelectProject = { selected = it },
                onSwitchTerminal = {},
                onOpenTerminal = {},
                onCloseTerminal = {}
            )
        }

        composeTestRule.onNodeWithTag("workspace_project_mobile-kit-30603ae7").performClick()

        assertEquals(null, selected)
    }
}
