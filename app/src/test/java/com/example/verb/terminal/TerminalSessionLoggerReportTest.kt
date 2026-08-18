package com.example.verb.terminal

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The diagnostics report is the copy/paste artefact a user hands to someone debugging their setup,
 * so it carries the same split the Diagnostics sheet shows: the launch directory and the shell's
 * own directory as two separate lines. Collapsing them into one "Working Directory" line is what
 * previously made a stale launch path read as the live one.
 *
 * Covered here rather than on-device because the report button sits in a bottom action row that is
 * clipped off-screen on the validation device (a pre-existing sheet layout issue, unrelated to the
 * working-directory model).
 */
class TerminalSessionLoggerReportTest {

    @Test
    fun `report lists launch and current directories as separate lines`() {
        val report = TerminalSessionLogger.exportDiagnosticReport(
            sessionState = TerminalSessionState.RUNNING,
            launchWorkingDir = "/data/user/0/com.aistudio.verb.app/files/projects/demo",
            currentWorkingDir = "/data/data/com.aistudio.verb.app/files/home",
            shellExecutable = "/data/user/0/com.aistudio.verb.app/files/usr/bin/proot"
        )

        assertTrue(
            report.contains("Launch Directory (device path): /data/user/0/com.aistudio.verb.app/files/projects/demo")
        )
        assertTrue(
            report.contains("Current Directory (terminal path): /data/data/com.aistudio.verb.app/files/home")
        )
    }

    @Test
    fun `an unknown current directory is reported as unknown, never as the launch directory`() {
        val launch = "/data/user/0/com.aistudio.verb.app/files/projects/demo"
        val report = TerminalSessionLogger.exportDiagnosticReport(
            sessionState = TerminalSessionState.EXITED,
            launchWorkingDir = launch,
            currentWorkingDir = null,
            shellExecutable = null
        )

        assertTrue(report.contains("Launch Directory (device path): $launch"))
        assertTrue(report.contains("Current Directory (terminal path): Unknown (shell integration unavailable)"))
        // The launch path must appear exactly once -- never substituted into the current line.
        assertTrue(report.split(launch).size - 1 == 1)
    }
}
