package com.example.verb.session

import com.example.verb.terminal.FakeTerminalRuntimeAdapter
import com.example.verb.terminal.ShellIntegrationEvent
import kotlinx.coroutines.async
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class ClaudeAgentAdapterTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun setUpFilesystem(): Pair<File, File> {
        val filesDir = temporaryFolder.newFolder("files")
        val project = File(filesDir, "projects/alpha").apply { mkdirs() }
        return filesDir to project
    }

    private fun transcriptDir(filesDir: File, project: File): File =
        File(filesDir, "home/.claude/projects/${project.absolutePath.replace('/', '-')}")

    // --- canResume(): the three ugly cases from the contract's mapping table ---

    @Test
    fun `canResume is YES when a transcript matching the known resumeIdentity exists`() {
        val (filesDir, project) = setUpFilesystem()
        val dir = transcriptDir(filesDir, project).apply { mkdirs() }
        File(dir, "session-abc.jsonl").createNewFile()
        val adapter = ClaudeAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        val verdict = adapter.canResume(AgentRef("claude", resumeIdentity = "session-abc"))

        assertEquals(ResumeVerdict.YES, verdict)
    }

    @Test
    fun `canResume is YES from any transcript when no resumeIdentity is known yet`() {
        val (filesDir, project) = setUpFilesystem()
        val dir = transcriptDir(filesDir, project).apply { mkdirs() }
        File(dir, "session-xyz.jsonl").createNewFile()
        val adapter = ClaudeAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        val verdict = adapter.canResume(AgentRef("claude", resumeIdentity = null))

        assertEquals(ResumeVerdict.YES, verdict)
    }

    @Test
    fun `canResume is NO when the transcript directory exists but the transcript is missing`() {
        val (filesDir, project) = setUpFilesystem()
        transcriptDir(filesDir, project).mkdirs() // exists, empty
        val adapter = ClaudeAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        val verdict = adapter.canResume(AgentRef("claude", resumeIdentity = "never-happened"))

        assertEquals(ResumeVerdict.NO, verdict)
    }

    @Test
    fun `canResume is UNKNOWN, never NO, when there is no project to check`() {
        val (filesDir, _) = setUpFilesystem()
        val adapter = ClaudeAgentAdapter(filesDir, projectDirectory = null, FakeTerminalRuntimeAdapter(filesDir))

        val verdict = adapter.canResume(AgentRef("claude", resumeIdentity = "abc"))

        assertEquals(ResumeVerdict.UNKNOWN, verdict)
    }

    @Test
    fun `canResume is UNKNOWN, never NO, when the transcript directory has never been created`() {
        val (filesDir, project) = setUpFilesystem()
        // transcriptDir(filesDir, project) deliberately not created
        val adapter = ClaudeAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        val verdict = adapter.canResume(AgentRef("claude", resumeIdentity = "abc"))

        assertEquals(ResumeVerdict.UNKNOWN, verdict)
    }

    // --- resume(): a settled command-history record is the only failure signal ---

    @Test
    fun `resume succeeds when nothing new settles within the window`() = runTest {
        val (filesDir, project) = setUpFilesystem()
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        val adapter = ClaudeAgentAdapter(filesDir, project, fake, resumeSettleMs = 200, pollIntervalMs = 20)

        // Nothing is ever fed to fake.simulateShellIntegration -- standing in for Claude staying
        // interactive and never returning to the prompt.
        val binding = adapter.resume(AgentRef("claude", "session-abc"))

        assertNotNull("nothing settling within the window must read as still running", binding)
    }

    @Test
    fun `resume fails when a new command settles before the window closes`() = runTest {
        val (filesDir, project) = setUpFilesystem()
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        val adapter = ClaudeAgentAdapter(filesDir, project, fake, resumeSettleMs = 5_000, pollIntervalMs = 50)

        val resumeCall = async { adapter.resume(AgentRef("claude", "bad-id")) }
        runCurrent() // let resume() capture the pre-resume history and send the command first
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(exitCode = 1))
        advanceUntilIdle()

        assertNull("a settled record appearing at all -- any exit code -- must never read as success", resumeCall.await())
    }

    @Test
    fun `resume fails, not succeeds, even when the settled command exits cleanly (code 0)`() = runTest {
        // A quick clean exit still means Claude is not running -- resume did not produce a live
        // session, whatever the exit code says. The only signal that means success is nothing
        // settling at all.
        val (filesDir, project) = setUpFilesystem()
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        val adapter = ClaudeAgentAdapter(filesDir, project, fake, resumeSettleMs = 5_000, pollIntervalMs = 50)

        val resumeCall = async { adapter.resume(AgentRef("claude", "session-abc")) }
        runCurrent()
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(exitCode = 0))
        advanceUntilIdle()

        assertNull(resumeCall.await())
    }
}
