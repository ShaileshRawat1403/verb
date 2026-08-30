package com.example.verb.session

import com.example.verb.project.VerbProject
import com.example.verb.terminal.FakeTerminalRuntimeAdapter
import com.example.verb.terminal.ShellIntegrationEvent
import com.example.verb.terminal.TerminalRuntime
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

@OptIn(ExperimentalCoroutinesApi::class)
class MultiTerminalLifecycleIsolationTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @After
    fun tearDown() {
        VerbTerminalSessionHolder.resetForTests()
    }

    private fun setupTestEnvironment(): Pair<File, VerbProject> {
        val filesDir = temporaryFolder.newFolder("files-${System.nanoTime()}")
        val projectDir = File(filesDir, "projects/alpha").apply { mkdirs() }
        val project = VerbProject(id = "alpha", directory = projectDir)
        return filesDir to project
    }

    /**
     * Property 1: Session Isolation
     * Events and command completions in T2 must NEVER advance or settle lifecycle state in T1.
     */
    @Test
    fun `commands executing and finishing in T2 never settle agent running in T1`() = runTest {
        val (filesDir, project) = setupTestEnvironment()
        val fakeT1 = FakeTerminalRuntimeAdapter(filesDir)
        val fakeT2 = FakeTerminalRuntimeAdapter(filesDir)

        val t1Id = "t1"
        val t2Id = "t2"
        val runtimes = mapOf(t1Id to fakeT1, t2Id to fakeT2)

        val coordinator = ClaudeSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeProvider = { id -> runtimes[id] },
            coroutineScope = this
        )

        try {
            // Launch Claude in T1
            coordinator.launch(
                project = project,
                sessionId = t1Id,
                command = "claude",
                runtime = fakeT1
            )
            runCurrent()

            assertEquals(VerbSessionState.LIVE, coordinator.session.value?.state)
            assertNotNull(coordinator.session.value?.process)

            // Run and complete an unrelated command in T2
            fakeT2.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
            fakeT2.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(0))
            testScheduler.advanceTimeBy(1500)
            runCurrent()

            // Claude in T1 MUST still be LIVE and unaffected
            assertEquals(
                "Claude in T1 must remain LIVE despite T2 command completion",
                VerbSessionState.LIVE,
                coordinator.session.value?.state
            )
            assertNotNull(coordinator.session.value?.process)

            // Now complete the command in T1
            fakeT1.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
            fakeT1.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(0))
            testScheduler.advanceTimeBy(1500)
            runCurrent()
            advanceUntilIdle()

            // Claude in T1 transitions only after T1 finishes
            assertNull("Process binding in T1 must be released after T1 command finishes", coordinator.session.value?.process)
        } finally {
            coordinator.cancelWatch()
        }
    }

    /**
     * Property 2: UI Independence
     * Switching activeTerminalSessionId in the UI produces zero lifecycle transitions and zero event mutations.
     */
    @Test
    fun `switching active terminal produces zero lifecycle transitions`() = runTest {
        val (filesDir, project) = setupTestEnvironment()
        val fakeT1 = FakeTerminalRuntimeAdapter(filesDir)
        val fakeT2 = FakeTerminalRuntimeAdapter(filesDir)
        val fakeT3 = FakeTerminalRuntimeAdapter(filesDir)

        val t1Runtime = TerminalRuntime(workingDir = filesDir, useFakeForTesting = true)
        val t2Runtime = TerminalRuntime(workingDir = filesDir, useFakeForTesting = true)
        val t3Runtime = TerminalRuntime(workingDir = filesDir, useFakeForTesting = true)

        VerbTerminalSessionHolder.getOrCreateActive { t1Runtime }
        val t1Id = VerbTerminalSessionHolder.activeId.value!!
        val t2Id = VerbTerminalSessionHolder.open { t2Runtime }!!
        val t3Id = VerbTerminalSessionHolder.open { t3Runtime }!!

        val runtimes = mapOf(t1Id to fakeT1, t2Id to fakeT2, t3Id to fakeT3)

        val coordinator = ClaudeSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeProvider = { id -> runtimes[id] },
            coroutineScope = this
        )

        try {
            // Launch Claude in T1 while T1 is active
            VerbTerminalSessionHolder.activate(t1Id)
            val launched = coordinator.launch(
                project = project,
                sessionId = t1Id,
                command = "claude",
                runtime = fakeT1
            )
            assertTrue("Claude launch should succeed", launched)
            runCurrent()

            assertEquals(VerbSessionState.LIVE, coordinator.session.value?.state)
            assertNotNull(coordinator.session.value?.process)

            // Switch active UI selection to T2
            VerbTerminalSessionHolder.activate(t2Id)
            assertEquals("T2 must be the active UI terminal", t2Id, VerbTerminalSessionHolder.activeId.value)
            fakeT2.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
            fakeT2.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(0))
            testScheduler.advanceTimeBy(1000)
            runCurrent()

            // Invariant: Changing active selection to T2 produces zero lifecycle mutations in T1 coordinator
            assertEquals(VerbSessionState.LIVE, coordinator.session.value?.state)
            assertNotNull(coordinator.session.value?.process)

            // Switch active UI selection to T3
            VerbTerminalSessionHolder.activate(t3Id)
            assertEquals("T3 must be the active UI terminal", t3Id, VerbTerminalSessionHolder.activeId.value)
            fakeT3.simulateShellIntegration(ShellIntegrationEvent.CommandStart)
            fakeT3.simulateShellIntegration(ShellIntegrationEvent.CommandEnd(1))
            testScheduler.advanceTimeBy(1000)
            runCurrent()

            // Invariant: Changing active selection to T3 produces zero lifecycle mutations in T1 coordinator
            assertEquals(VerbSessionState.LIVE, coordinator.session.value?.state)
            assertNotNull(coordinator.session.value?.process)

            // Switch back to T1
            VerbTerminalSessionHolder.activate(t1Id)
            assertEquals("T1 must be active UI terminal again", t1Id, VerbTerminalSessionHolder.activeId.value)
            runCurrent()

            assertEquals(
                "Changing active UI selection across T1, T2, T3 must produce zero lifecycle transitions",
                VerbSessionState.LIVE,
                coordinator.session.value?.state
            )
            assertNotNull(coordinator.session.value?.process)
        } finally {
            coordinator.cancelWatch()
        }
    }

    /**
     * Property 3: Restoration Identity
     * After Activity/ViewModel recreation, every foreground agent restores bound to its exact concrete session.
     */
    @Test
    fun `multi-agent restoration binds each coordinator to its exact concrete session`() = runTest {
        val (filesDir, _) = setupTestEnvironment()

        val t1Runtime = TerminalRuntime(workingDir = filesDir, useFakeForTesting = true)
        val t2Runtime = TerminalRuntime(workingDir = filesDir, useFakeForTesting = true)

        VerbTerminalSessionHolder.getOrCreateActive { t1Runtime }
        val t1Id = VerbTerminalSessionHolder.activeId.value!!
        val t2Id = VerbTerminalSessionHolder.open { t2Runtime }!!

        // Claim T1 for Claude, T2 for Codex
        VerbTerminalSessionHolder.claimForeground(t1Id, CLAUDE_AGENT_TYPE, setOf("cmd-1"))
        VerbTerminalSessionHolder.claimForeground(t2Id, CODEX_AGENT_TYPE, setOf("cmd-2"))

        // Save persisted records for Claude and Codex
        val claudeStore = InMemoryVerbSessionStore()
        claudeStore.save(
            VerbSession(
                id = "claude-session-id",
                projectId = "p1",
                runtime = CLAUDE_AGENT_TYPE,
                createdAt = java.time.Instant.now(),
                lastSeenAt = java.time.Instant.now(),
                state = VerbSessionState.LIVE,
                agent = AgentRef(CLAUDE_AGENT_TYPE)
            )
        )

        val codexStore = InMemoryVerbSessionStore()
        codexStore.save(
            VerbSession(
                id = "codex-session-id",
                projectId = "p1",
                runtime = CODEX_AGENT_TYPE,
                createdAt = java.time.Instant.now(),
                lastSeenAt = java.time.Instant.now(),
                state = VerbSessionState.LIVE,
                agent = AgentRef(CODEX_AGENT_TYPE)
            )
        )

        // Make T2 active (to ensure restoration doesn't falsely bind everything to active terminal)
        VerbTerminalSessionHolder.activate(t2Id)

        // Recreate Coordinators
        val claudeCoordinator = ClaudeSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeProvider = { id -> VerbTerminalSessionHolder.runtimeOf(id) },
            coroutineScope = this,
            sessionStore = claudeStore,
            processBindingConfirmed = true
        )

        val codexCoordinator = CodexSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeProvider = { id -> VerbTerminalSessionHolder.runtimeOf(id) },
            coroutineScope = this,
            sessionStore = codexStore,
            processBindingConfirmed = true
        )

        // Assert Claude restored as LIVE bound to T1 (not active T2)
        assertEquals(VerbSessionState.LIVE, claudeCoordinator.session.value?.state)
        assertNotNull(claudeCoordinator.session.value?.process)
        assertEquals(t1Id, VerbTerminalSessionHolder.sessionIdForAgent(CLAUDE_AGENT_TYPE))

        // Assert Codex restored as LIVE bound to T2
        assertEquals(VerbSessionState.LIVE, codexCoordinator.session.value?.state)
        assertNotNull(codexCoordinator.session.value?.process)
        assertEquals(t2Id, VerbTerminalSessionHolder.sessionIdForAgent(CODEX_AGENT_TYPE))

        claudeCoordinator.cancelWatch()
        codexCoordinator.cancelWatch()
    }

    /**
     * Property 4: Ambiguity Refusal
     * If multiple sessions claim the same agent type, restoration refuses to guess and resolves recovery.
     */
    @Test
    fun `ambiguous foreground bindings refuse guessed restoration`() = runTest {
        val (filesDir, _) = setupTestEnvironment()
        val t1Runtime = TerminalRuntime(workingDir = filesDir, useFakeForTesting = true)
        val t2Runtime = TerminalRuntime(workingDir = filesDir, useFakeForTesting = true)

        VerbTerminalSessionHolder.getOrCreateActive { t1Runtime }
        val t1Id = VerbTerminalSessionHolder.activeId.value!!
        val t2Id = VerbTerminalSessionHolder.open { t2Runtime }!!

        // Invariant violation: both T1 and T2 claim Claude
        VerbTerminalSessionHolder.claimForeground(t1Id, CLAUDE_AGENT_TYPE, emptySet())
        VerbTerminalSessionHolder.claimForeground(t2Id, CLAUDE_AGENT_TYPE, emptySet())

        // Ambiguity refusal check
        assertNull("Ambiguous foreground bindings must refuse guess and return null", VerbTerminalSessionHolder.foregroundBindingForAgent(CLAUDE_AGENT_TYPE))

        val claudeStore = InMemoryVerbSessionStore()
        claudeStore.save(
            VerbSession(
                id = "claude-session-id",
                projectId = "p1",
                runtime = CLAUDE_AGENT_TYPE,
                createdAt = java.time.Instant.now(),
                lastSeenAt = java.time.Instant.now(),
                state = VerbSessionState.LIVE,
                agent = AgentRef(CLAUDE_AGENT_TYPE)
            )
        )

        val coordinator = ClaudeSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeProvider = { id -> VerbTerminalSessionHolder.runtimeOf(id) },
            coroutineScope = this,
            sessionStore = claudeStore,
            processBindingConfirmed = true
        )

        // Because ambiguous, binding must NOT be attached
        assertNull("Ambiguous binding must not restore as LIVE process binding", coordinator.session.value?.process)
    }

    /**
     * Property 5: Atomic Launch Ownership
     * Claim foreground before dispatch; if dispatch throws, release foreground atomically.
     */
    @Test
    fun `launch failure rolls back foreground claim atomically`() = runTest {
        val (filesDir, project) = setupTestEnvironment()
        val throwingRuntime = object : FakeTerminalRuntimeAdapter(filesDir) {
            override fun sendCommand(cmd: String) {
                throw IllegalStateException("PTY crashed")
            }
        }

        val runtime = TerminalRuntime(workingDir = filesDir, useFakeForTesting = true)
        VerbTerminalSessionHolder.getOrCreateActive { runtime }
        val activeId = VerbTerminalSessionHolder.activeId.value!!

        val coordinator = ClaudeSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeProvider = { activeId.takeIf { id -> id == activeId }?.let { throwingRuntime } },
            coroutineScope = this
        )

        val launched = coordinator.launch(
            project = project,
            sessionId = activeId,
            command = "claude",
            runtime = throwingRuntime
        )

        assertFalse("Launch must report failure when sendCommand fails", launched)
        assertNull("Foreground claim must be rolled back on dispatch failure", VerbTerminalSessionHolder.foregroundBindingOf(activeId))
        assertEquals(VerbSessionState.ENDED, coordinator.session.value?.state)
    }

    /**
     * Property 6: Concrete Dispatch Isolation
     * Even if active terminal changes in VerbTerminalSessionHolder, dispatch and lifecycle remain bound to the originating session.
     */
    @Test
    fun `dispatch and lifecycle remain strictly in originating session even if UI switches concurrently`() = runTest {
        val (filesDir, project) = setupTestEnvironment()
        val t1Runtime = FakeTerminalRuntimeAdapter(filesDir)
        val t2Runtime = FakeTerminalRuntimeAdapter(filesDir)

        val t1Id = "t1"
        val t2Id = "t2"
        val runtimes = mapOf(t1Id to t1Runtime, t2Id to t2Runtime)

        val coordinator = ClaudeSessionCoordinator(
            filesDir = filesDir,
            terminalRuntimeProvider = { id -> runtimes[id] },
            coroutineScope = this
        )

        try {
            // Target T1 for launch
            val launched = coordinator.launch(
                project = project,
                sessionId = t1Id,
                command = "claude",
                runtime = t1Runtime
            )
            assertTrue(launched)

            // Switch UI immediately to T2
            VerbTerminalSessionHolder.activate(t2Id)

            // Command was received on T1, NOT on T2
            assertTrue(t1Runtime.terminalOutput.value.contains("claude"))
            assertFalse(t2Runtime.terminalOutput.value.contains("claude"))
        } finally {
            coordinator.cancelWatch()
        }
    }
}
