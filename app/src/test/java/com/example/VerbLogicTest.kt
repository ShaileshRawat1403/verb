package com.example

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.example.verb.actions.ActionRegistry
import com.example.verb.intent.IntentEngine
import com.example.verb.model.ActionRisk
import com.example.verb.model.EntityType
import com.example.verb.semantic.SemanticEngine
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class VerbLogicTest {

    private lateinit var context: Context
    private lateinit var intentEngine: IntentEngine
    private lateinit var actionRegistry: ActionRegistry
    private lateinit var semanticEngine: SemanticEngine

    @Before
    fun setup() {
        context = ApplicationProvider.getApplicationContext()
        intentEngine = IntentEngine()
        actionRegistry = ActionRegistry(context)
        semanticEngine = SemanticEngine()
    }

    @Test
    fun `supported intent accepted`() {
        val intent = intentEngine.resolveIntent("show me my storage")
        assertEquals("storage.summary", intent.id)
        assertEquals(ActionRisk.READ_ONLY, intent.risk)
    }

    @Test
    fun `unsupported intent rejected`() {
        val intent = intentEngine.resolveIntent("xyz123 unrecognised string query")
        assertEquals("unsupported.intent", intent.id)
        assertFalse(actionRegistry.isActionSupported(intent.id))
    }

    @Test
    fun `action registry rejects unknown actions`() {
        val isSupported = actionRegistry.isActionSupported("nonexistent.action")
        assertFalse(isSupported)
    }

    @Test
    fun `read-only action classified correctly`() {
        val intent = intentEngine.resolveIntent("check memory")
        assertEquals("memory.summary", intent.id)
        assertEquals(ActionRisk.READ_ONLY, intent.risk)

        val result = actionRegistry.executeAction(intent)
        assertTrue(result.isSuccess)
        assertFalse(result.requiresConfirmation)
    }

    @Test
    fun `process stop requires confirmation and retains original intent parameters`() {
        val intent = intentEngine.resolveIntent("stop process 1234")
        assertEquals("process.stop", intent.id)
        assertEquals("1234", intent.parameters["pid"])
        assertEquals(ActionRisk.CONTROLLED_WRITE, intent.risk)

        val unconfirmedResult = actionRegistry.executeAction(intent, confirmed = false)
        assertTrue(unconfirmedResult.requiresConfirmation)
        assertNotNull(unconfirmedResult.originalIntent)
        assertEquals("1234", unconfirmedResult.originalIntent?.parameters?.get("pid"))

        // This unit test deliberately stops at the confirmation boundary. It must not
        // request a real Android process kill merely to prove confirmation is required.
    }

    @Test
    fun `destructive selected command is flagged and not executed`() {
        val text = "rm -rf dist"
        val entity = semanticEngine.analyzeText(text)

        assertEquals(EntityType.DESTRUCTIVE_COMMAND, entity.entityType)
        assertEquals(ActionRisk.DESTRUCTIVE, entity.risk)
        assertNotNull(entity.warningMessage)
        assertTrue(entity.breakdown.isNotEmpty())
    }

    @Test
    fun `port conflict entity detection`() {
        val text = "Error: EADDRINUSE :::3000"
        val entity = semanticEngine.analyzeText(text)

        assertEquals(EntityType.PORT_CONFLICT, entity.entityType)
        assertEquals(3000, entity.detectedPort)
        assertTrue(entity.suggestedActions.any { it.label.contains("3000") })
    }

    @Test
    fun `path entity detection`() {
        val text = "/sdcard/Download/document.pdf"
        val entity = semanticEngine.analyzeText(text)

        assertEquals(EntityType.FILE_PATH, entity.entityType)
        assertEquals(text, entity.detectedPath)
    }

    @Test
    fun `error message recognition`() {
        val text = "TypeError: Cannot read properties of undefined (reading 'length')"
        val entity = semanticEngine.analyzeText(text)

        assertEquals(EntityType.ERROR_MESSAGE, entity.entityType)
        assertTrue(entity.description.contains("unknown without more context", ignoreCase = true))
    }

    @Test
    fun `selection change listener captures exact range and passes selection to observer`() {
        val runtime = com.example.verb.terminal.TerminalRuntime(context.filesDir, useFakeForTesting = true)
        var capturedSelection = ""
        var capturedRange = androidx.compose.ui.text.TextRange.Zero

        val listener = com.example.verb.terminal.SelectionChangeListener { range, text ->
            capturedRange = range
            capturedSelection = text
        }

        runtime.addSelectionChangeListener(listener)
        runtime.notifySelectionChanged(androidx.compose.ui.text.TextRange(5, 12), "/storage/emulated/0")

        assertEquals(androidx.compose.ui.text.TextRange(5, 12), capturedRange)
        assertEquals("/storage/emulated/0", capturedSelection)

        val entity = semanticEngine.analyzeText(capturedSelection)
        assertEquals(EntityType.FILE_PATH, entity.entityType)
        assertTrue(entity.title.contains("Path"))

        runtime.destroy()
    }

    @Test
    fun `terminal runtime fake session initialization`() {
        val runtime = com.example.verb.terminal.TerminalRuntime(context.filesDir, useFakeForTesting = true)
        assertTrue(runtime.isSessionActive.value)
        assertTrue(runtime.terminalOutput.value.contains("Verb Terminal Session Active"))
        assertEquals(
            com.example.verb.terminal.TerminalContextCapability.SESSION_ONLY,
            runtime.terminalContextState.value.capability
        )
        assertEquals(
            com.example.verb.terminal.AlternateScreenState.UNKNOWN,
            runtime.terminalContextState.value.alternateScreenState
        )

        runtime.sendCommand("echo 'Verb TTY test'")
        runtime.clearBuffer()
        assertEquals("$ ", runtime.terminalOutput.value)

        runtime.destroy()
        assertFalse(runtime.isSessionActive.value)
        assertEquals(
            com.example.verb.terminal.TerminalContextCapability.UNAVAILABLE,
            runtime.terminalContextState.value.capability
        )
    }

    @Test
    fun `production termux runtime adapter reports truthful failure when native pty unavailable`() {
        // Without useFakeForTesting, production TermuxTerminalRuntimeAdapter is selected.
        // On JVM without native libtermux.so, it must report FAILED state truthfully without fake fallback.
        val runtime = com.example.verb.terminal.TerminalRuntime(context.filesDir, useFakeForTesting = false)
        assertEquals(com.example.verb.terminal.TerminalSessionState.FAILED, runtime.sessionState.value)
        assertFalse(runtime.isSessionActive.value)
        assertTrue(runtime.terminalOutput.value.contains("FAILED to start Termux PTY session"))
        assertEquals(
            com.example.verb.terminal.TerminalContextCapability.UNAVAILABLE,
            runtime.terminalContextState.value.capability
        )
        assertEquals(
            com.example.verb.terminal.AlternateScreenState.UNKNOWN,
            runtime.terminalContextState.value.alternateScreenState
        )
    }

    @Test
    fun `terminal runtime adapter session state transitions`() {
        val adapter: com.example.verb.terminal.TerminalRuntimeAdapter =
            com.example.verb.terminal.TerminalRuntime(context.filesDir, useFakeForTesting = true)

        assertEquals(com.example.verb.terminal.TerminalSessionState.RUNNING, adapter.sessionState.value)
        assertTrue(adapter.isSessionActive.value)

        adapter.sendControlKey("CTRL_C")
        adapter.sendText("echo test\n")

        adapter.destroy()
        assertEquals(com.example.verb.terminal.TerminalSessionState.EXITED, adapter.sessionState.value)
        assertFalse(adapter.isSessionActive.value)
    }

    @Test
    fun `port observation truthfulness`() {
        val intent = intentEngine.resolveIntent("what's using port 3000?")
        val result = actionRegistry.executeAction(intent)

        assertNotNull(result.observedOutput)
        assertNotNull(result.explanation)
        assertTrue(result.observedOutput?.contains("Socket bind check") == true)
        assertFalse(result.observedOutput?.contains("PID 19281") == true) // No fake PID
    }

    @Test
    fun `terminal explanations do not claim command execution`() {
        val result = actionRegistry.executeAction(
            com.example.verb.model.VerbIntent(
                id = "terminal.explain",
                name = "Explain Command",
                parameters = mapOf("command" to "echo hello")
            )
        )

        assertTrue(result.isSuccess)
        assertTrue(result.summary.contains("will not execute", ignoreCase = true))
        assertFalse(result.summary.contains("executes shell operation", ignoreCase = true))
    }

    @Test
    fun `terminal port intent resolution`() {
        val intent = intentEngine.resolveIntent("what's using port 3000?")
        assertEquals("network.port.inspect", intent.id)
        assertEquals("3000", intent.parameters["port"])
    }

    @Test
    fun `file list intent resolution`() {
        val intent = intentEngine.resolveIntent("show files in /sdcard")
        assertEquals("file.list", intent.id)
        assertEquals("/sdcard", intent.parameters["path"])
    }

    @Test
    fun `selected port conflict without a port fabricates nothing`() {
        val entity = semanticEngine.analyzeText("EADDRINUSE")
        assertEquals(EntityType.PORT_CONFLICT, entity.entityType)
        assertNull(entity.detectedPort)
        assertTrue(entity.suggestedActions.isEmpty())
    }

    @Test
    fun `valid and invalid selected ports are distinguished`() {
        assertEquals(EntityType.PORT, semanticEngine.analyzeText("port 8080").entityType)
        assertEquals(EntityType.GENERIC_TEXT, semanticEngine.analyzeText("port 70000").entityType)
    }

    @Test
    fun `network and credential selections retain their own classification`() {
        assertEquals(EntityType.URL, semanticEngine.analyzeText("https://example.com/a/b").entityType)
        assertEquals(EntityType.IP_ADDRESS, semanticEngine.analyzeText("192.168.1.1").entityType)

        val credential = semanticEngine.analyzeText("Authorization: Bearer xyz123")
        assertEquals(EntityType.SENSITIVE_TEXT, credential.entityType)
        assertTrue(credential.isSensitive)
        assertEquals("******** (Redacted)", credential.rawText)
    }

    @Test
    fun `selected PID and command patterns require exact recognition`() {
        assertEquals(EntityType.PID, semanticEngine.analyzeText("PID 18342").entityType)
        assertEquals(EntityType.GENERIC_TEXT, semanticEngine.analyzeText("PID 0").entityType)
        assertEquals(EntityType.COMMAND, semanticEngine.analyzeText("ls -la").entityType)
        assertEquals(EntityType.GENERIC_TEXT, semanticEngine.analyzeText("lsof").entityType)
    }

    @Test
    fun `selected commands are explained without execution claims`() {
        val entity = semanticEngine.analyzeText("echo hello")

        assertEquals(EntityType.COMMAND, entity.entityType)
        assertTrue(entity.description.contains("will not execute", ignoreCase = true))
        assertFalse(entity.description.contains("executes", ignoreCase = true))
    }

    @Test
    fun `registry risk policy overrides caller supplied risk`() {
        val stopIntent = com.example.verb.model.VerbIntent(
            id = "process.stop",
            name = "Stop Process",
            parameters = mapOf("pid" to "9999"),
            risk = ActionRisk.READ_ONLY
        )
        val stopResult = actionRegistry.executeAction(stopIntent, confirmed = false)
        assertTrue(stopResult.requiresConfirmation)
        assertEquals(ActionRisk.CONTROLLED_WRITE, stopResult.originalIntent?.risk)

        val storageIntent = com.example.verb.model.VerbIntent(
            id = "storage.summary",
            name = "Storage Summary",
            risk = ActionRisk.CONTROLLED_WRITE
        )
        val storageResult = actionRegistry.executeAction(storageIntent, confirmed = false)
        assertFalse(storageResult.requiresConfirmation)
        assertEquals(ActionRisk.READ_ONLY, storageResult.originalIntent?.risk)
    }

    @Test
    fun `missing port is not silently changed to 3000`() {
        val intent = intentEngine.resolveIntent("what is using this port?")
        assertEquals("unsupported.intent", intent.id)
    }

    @Test
    fun `invalid pid does not create a confirmation and does not invoke the stopper`() {
        val naturalLanguageIntent = intentEngine.resolveIntent("stop process")
        assertEquals("unsupported.intent", naturalLanguageIntent.id)

        var stopperInvoked = false
        val registry = ActionRegistry(
            context = context,
            processStopper = { stopperInvoked = true },
            currentProcessId = { 4242 }
        )

        val intent = com.example.verb.model.VerbIntent(
            id = "process.stop",
            name = "Stop Process",
            parameters = mapOf("pid" to "-1")
        )
        val result = registry.executeAction(intent, confirmed = false)
        assertFalse(result.requiresConfirmation)
        assertFalse(result.isSuccess)
        assertFalse(stopperInvoked)
    }

    @Test
    fun `self pid does not create a confirmation and does not invoke the stopper`() {
        var stopperInvoked = false
        val registry = ActionRegistry(
            context = context,
            processStopper = { stopperInvoked = true },
            currentProcessId = { 1234 }
        )
        val intent = com.example.verb.model.VerbIntent(
            id = "process.stop",
            name = "Stop Process",
            parameters = mapOf("pid" to "1234")
        )
        val result = registry.executeAction(intent, confirmed = false)
        assertFalse(result.requiresConfirmation)
        assertFalse(result.isSuccess)
        assertTrue(result.title.contains("Blocked"))
        assertFalse(stopperInvoked)
    }

    @Test
    fun `valid PID requires confirmation before invocation`() {
        var stopperInvoked = false
        val registry = ActionRegistry(
            context = context,
            processStopper = { stopperInvoked = true },
            currentProcessId = { 4242 }
        )
        val intent = com.example.verb.model.VerbIntent(
            id = "process.stop",
            name = "Stop Process",
            parameters = mapOf("pid" to "999999")
        )
        val result = registry.executeAction(intent, confirmed = false)
        assertTrue(result.requiresConfirmation)
        assertFalse(result.isSuccess)
        assertFalse(stopperInvoked)
    }

    @Test
    fun `process stop reports an unverified outcome when Android accepts the request`() {
        val registry = ActionRegistry(
            context = context,
            processStopper = { },
            currentProcessId = { 4242 }
        )
        val intent = com.example.verb.model.VerbIntent(
            id = "process.stop",
            name = "Stop Process",
            parameters = mapOf("pid" to "1234")
        )

        val result = registry.executeAction(intent, confirmed = true)

        assertFalse(result.isSuccess)
        assertTrue(result.summary.contains("Outcome unverified"))
        assertFalse(result.summary.contains("stopped", ignoreCase = true))
        assertFalse(result.summary.contains("terminated", ignoreCase = true))
    }

    @Test
    fun `process stop exception is reported as failure`() {
        val registry = ActionRegistry(
            context = context,
            processStopper = { throw SecurityException("permission denied") },
            currentProcessId = { 4242 }
        )
        val intent = com.example.verb.model.VerbIntent(
            id = "process.stop",
            name = "Stop Process",
            parameters = mapOf("pid" to "1234")
        )

        val result = registry.executeAction(intent, confirmed = true)

        assertFalse(result.isSuccess)
        assertEquals("Process Stop Failed", result.title)
        assertTrue(result.errorMessage?.contains("permission denied") == true)
    }

    @Test
    fun `invalid port is not substituted with 3000`() {
        val intent = com.example.verb.model.VerbIntent(
            id = "network.port.inspect",
            name = "Inspect Port",
            parameters = mapOf("port" to "70000")
        )

        val result = actionRegistry.executeAction(intent)

        assertFalse(result.isSuccess)
        assertTrue(result.summary.contains("70000"))
        assertFalse(result.summary.contains("3000"))
    }
}
