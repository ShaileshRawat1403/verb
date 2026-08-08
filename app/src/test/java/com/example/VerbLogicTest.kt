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

        val confirmedResult = actionRegistry.executeAction(unconfirmedResult.originalIntent!!, confirmed = true)
        assertFalse(confirmedResult.requiresConfirmation)
        assertEquals("1234", confirmedResult.originalIntent?.parameters?.get("pid"))
    }

    @Test
    fun `storage size entity detection`() {
        val entity = semanticEngine.analyzeText("9.7G")
        assertEquals("Storage Size (9.7 G)", entity.title)
        assertTrue(entity.suggestedActions.isNotEmpty())
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
        assertTrue(entity.description.contains("null reference", ignoreCase = true) || entity.description.contains("undefined", ignoreCase = true))
    }

    @Test
    fun `terminal command template resolution`() {
        val intent = intentEngine.resolveIntent("what's using port 3000?")
        assertEquals("network.port.inspect", intent.id)
        assertEquals("3000", intent.parameters["port"])
        assertTrue(intent.commandTemplate?.contains("3000") == true)
    }

    @Test
    fun `file list intent command template`() {
        val intent = intentEngine.resolveIntent("show files in /sdcard")
        assertEquals("file.list", intent.id)
        assertEquals("ls -la /sdcard", intent.commandTemplate)
    }
}
