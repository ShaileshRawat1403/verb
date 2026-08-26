package com.example.verb.session

import com.example.verb.terminal.FakeTerminalRuntimeAdapter
import com.example.verb.terminal.ShellIntegrationEvent
import kotlinx.coroutines.async
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

@OptIn(ExperimentalCoroutinesApi::class)
class CodexAgentAdapterTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun setUpFilesystem(): Pair<File, File> {
        val filesDir = temporaryFolder.newFolder("files")
        val project = File(filesDir, "projects/alpha").apply { mkdirs() }
        return filesDir to project
    }

    /**
     * A rollout as Codex writes it: a session-meta header line, then the conversation. [userTurns]
     * false is the case that matters most here -- Codex opened and never used, which produces a
     * file but nothing to recover.
     */
    private fun writeRollout(
        filesDir: File,
        cwd: String,
        sessionId: String,
        startedAt: String = "2026-08-21T10:00:00.000Z",
        userTurns: Boolean = true,
        day: String = "2026/08/21"
    ): File {
        val directory = File(filesDir, "home/.codex/sessions/$day").apply { mkdirs() }
        val file = File(directory, "rollout-$startedAt-$sessionId.jsonl")
        val header = """{"timestamp":"$startedAt","type":"session_meta","payload":""" +
            """{"id":"$sessionId","timestamp":"$startedAt","cwd":"$cwd","originator":"codex_cli_rs"}}"""
        val turn = """{"timestamp":"$startedAt","type":"response_item","payload":""" +
            """{"type":"message","role":"user","content":[{"type":"input_text","text":"hi"}]}}"""
        file.writeText(if (userTurns) "$header\n$turn\n" else "$header\n")
        return file
    }

    // --- canResume(): the three verdicts, and the difference between "opened" and "used" ---

    @Test
    fun `canResume is YES when a rollout for this project holds a real user turn`() {
        val (filesDir, project) = setUpFilesystem()
        writeRollout(filesDir, cwd = project.absolutePath, sessionId = "codex-session-1")
        val adapter = CodexAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.YES, adapter.canResume(AgentRef("codex")))
    }

    @Test
    fun `canResume is NO for a rollout that was only ever opened, never used`() {
        // Codex writes the rollout file at startup. Treating that as recovery evidence would make
        // Verb promise a conversation and then resume an empty one.
        val (filesDir, project) = setUpFilesystem()
        writeRollout(filesDir, cwd = project.absolutePath, sessionId = "codex-idle", userTurns = false)
        val adapter = CodexAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.NO, adapter.canResume(AgentRef("codex")))
        assertNull(adapter.resumeIdentity(AgentRef("codex")))
    }

    @Test
    fun `canResume is NO when the only user-role record is Codex's own environment context`() {
        // Read from a real codex-cli 0.149.0 rollout: Codex injects <environment_context> as a
        // user-role message. Counting it would report a session the user never actually used.
        val (filesDir, project) = setUpFilesystem()
        val directory = File(filesDir, "home/.codex/sessions/2026/08/21").apply { mkdirs() }
        val startedAt = "2026-08-21T13:08:16.859Z"
        File(directory, "rollout-$startedAt-codex-injected.jsonl").writeText(
            """{"timestamp":"$startedAt","ordinal":0,"type":"session_meta","payload":""" +
                """{"session_id":"codex-injected","id":"codex-injected","timestamp":"$startedAt",""" +
                """"cwd":"${project.absolutePath}","originator":"codex_cli_rs"}}""" + "\n" +
                """{"timestamp":"$startedAt","ordinal":5,"type":"response_item","payload":""" +
                """{"type":"message","role":"user","content":[{"type":"input_text",""" +
                """"text":"<environment_context>\n  <cwd>${project.absolutePath}</cwd>\n</environment_context>"}]}}""" + "\n"
        )
        val adapter = CodexAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.NO, adapter.canResume(AgentRef("codex")))
        assertNull(adapter.resumeIdentity(AgentRef("codex")))
    }

    @Test
    fun `canResume is YES for a rollout whose real turn follows Codex's injected context`() {
        val (filesDir, project) = setUpFilesystem()
        val directory = File(filesDir, "home/.codex/sessions/2026/08/21").apply { mkdirs() }
        val startedAt = "2026-08-21T13:08:16.859Z"
        File(directory, "rollout-$startedAt-codex-real.jsonl").writeText(
            """{"timestamp":"$startedAt","ordinal":0,"type":"session_meta","payload":""" +
                """{"session_id":"codex-real","id":"codex-real","timestamp":"$startedAt",""" +
                """"cwd":"${project.absolutePath}"}}""" + "\n" +
                """{"timestamp":"$startedAt","ordinal":5,"type":"response_item","payload":""" +
                """{"type":"message","role":"user","content":[{"type":"input_text",""" +
                """"text":"<environment_context>\n  <cwd>${project.absolutePath}</cwd>\n</environment_context>"}]}}""" + "\n" +
                """{"timestamp":"$startedAt","ordinal":8,"type":"response_item","payload":""" +
                """{"type":"message","role":"user","content":[{"type":"input_text","text":"say hi"}]}}""" + "\n"
        )
        val adapter = CodexAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.YES, adapter.canResume(AgentRef("codex")))
        assertEquals("codex-real", adapter.resumeIdentity(AgentRef("codex")))
    }

    @Test
    fun `canResume is NO when the sessions tree is readable but holds nothing for this project`() {
        val (filesDir, project) = setUpFilesystem()
        writeRollout(filesDir, cwd = "/somewhere/else", sessionId = "other-project")
        val adapter = CodexAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.NO, adapter.canResume(AgentRef("codex")))
    }

    @Test
    fun `canResume is NO when the known resumeIdentity is not among this project's rollouts`() {
        val (filesDir, project) = setUpFilesystem()
        writeRollout(filesDir, cwd = project.absolutePath, sessionId = "codex-session-1")
        val adapter = CodexAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.NO, adapter.canResume(AgentRef("codex", resumeIdentity = "never-happened")))
    }

    @Test
    fun `canResume is UNKNOWN, never NO, when there is no project to check`() {
        val (filesDir, _) = setUpFilesystem()
        val adapter = CodexAgentAdapter(filesDir, projectDirectory = null, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.UNKNOWN, adapter.canResume(AgentRef("codex", resumeIdentity = "abc")))
    }

    @Test
    fun `canResume is UNKNOWN, never NO, when Codex has never written a sessions tree`() {
        val (filesDir, project) = setUpFilesystem()
        // ~/.codex/sessions deliberately absent: no evidence either way.
        val adapter = CodexAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.UNKNOWN, adapter.canResume(AgentRef("codex", resumeIdentity = "abc")))
    }

    @Test
    fun `a rollout recorded under the other Android path alias still matches the project`() {
        val filesDir = temporaryFolder.newFolder("data", "user", "0", "com.aistudio.verb.app", "files")
        val project = File(filesDir, "projects/alpha").apply { mkdirs() }
        writeRollout(
            filesDir,
            cwd = "/data/data/com.aistudio.verb.app/files/projects/alpha",
            sessionId = "codex-alias"
        )
        val adapter = CodexAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals(ResumeVerdict.YES, adapter.canResume(AgentRef("codex")))
        assertEquals("codex-alias", adapter.resumeIdentity(AgentRef("codex")))
    }

    // --- resumeIdentity(): Codex's own conversation id, never the filename ---

    @Test
    fun `resumeIdentity returns the newest used conversation for this project`() {
        val (filesDir, project) = setUpFilesystem()
        writeRollout(
            filesDir,
            cwd = project.absolutePath,
            sessionId = "codex-older",
            startedAt = "2026-08-20T09:00:00.000Z",
            day = "2026/08/20"
        )
        writeRollout(
            filesDir,
            cwd = project.absolutePath,
            sessionId = "codex-newer",
            startedAt = "2026-08-21T09:00:00.000Z"
        )
        val adapter = CodexAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals("codex-newer", adapter.resumeIdentity(AgentRef("codex")))
    }

    @Test
    fun `resumeIdentity skips a newer rollout that was never used`() {
        val (filesDir, project) = setUpFilesystem()
        writeRollout(
            filesDir,
            cwd = project.absolutePath,
            sessionId = "codex-real",
            startedAt = "2026-08-20T09:00:00.000Z",
            day = "2026/08/20"
        )
        writeRollout(
            filesDir,
            cwd = project.absolutePath,
            sessionId = "codex-idle",
            startedAt = "2026-08-21T09:00:00.000Z",
            userTurns = false
        )
        val adapter = CodexAgentAdapter(filesDir, project, FakeTerminalRuntimeAdapter(filesDir))

        assertEquals("codex-real", adapter.resumeIdentity(AgentRef("codex")))
        assertEquals(ResumeVerdict.YES, adapter.canResume(AgentRef("codex")))
    }

    // --- resume(): a settled command-history record is the only failure signal ---

    @Test
    fun `resume runs codex resume with the conversation id and succeeds when nothing settles`() = runTest {
        val (filesDir, project) = setUpFilesystem()
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        val adapter = CodexAgentAdapter(filesDir, project, fake, resumeSettleMs = 200, pollIntervalMs = 20)

        val binding = adapter.resume(AgentRef("codex", "codex-session-1"))

        assertNotNull("nothing settling within the window must read as still running", binding)
        assertTrue(
            "resume must name the conversation, not open Codex's interactive picker",
            fake.terminalOutput.value.contains("codex --disable apps resume codex-session-1")
        )
    }

    @Test
    fun `resume falls back to --last, never the bare picker, when no id is known`() = runTest {
        val (filesDir, project) = setUpFilesystem()
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        val adapter = CodexAgentAdapter(filesDir, project, fake, resumeSettleMs = 200, pollIntervalMs = 20)

        adapter.resume(AgentRef("codex", resumeIdentity = null))

        assertTrue(fake.terminalOutput.value.contains("codex --disable apps resume --last"))
    }

    @Test
    fun `resume fails when a new command settles before the window closes`() = runTest {
        val (filesDir, project) = setUpFilesystem()
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        val adapter = CodexAgentAdapter(filesDir, project, fake, resumeSettleMs = 5_000, pollIntervalMs = 50)

        val resumeCall = async { adapter.resume(AgentRef("codex", "bad-id")) }
        runCurrent()
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(exitCode = 1))
        advanceUntilIdle()

        assertNull("a settled record appearing at all -- any exit code -- must never read as success", resumeCall.await())
    }

    @Test
    fun `resume fails, not succeeds, even when the settled command exits cleanly (code 0)`() = runTest {
        val (filesDir, project) = setUpFilesystem()
        val fake = FakeTerminalRuntimeAdapter(filesDir)
        val adapter = CodexAgentAdapter(filesDir, project, fake, resumeSettleMs = 5_000, pollIntervalMs = 50)

        val resumeCall = async { adapter.resume(AgentRef("codex", "codex-session-1")) }
        runCurrent()
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
        fake.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(exitCode = 0))
        advanceUntilIdle()

        assertNull(resumeCall.await())
    }
}
