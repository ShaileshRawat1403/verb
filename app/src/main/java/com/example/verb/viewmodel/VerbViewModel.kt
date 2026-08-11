package com.example.verb.viewmodel

import android.app.Application
import android.net.Uri
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.example.verb.actions.ActionRegistry
import com.example.verb.ai.AiAssistantRequest
import com.example.verb.ai.AiAssistantService
import com.example.verb.ai.AiAssistantState
import com.example.verb.ai.AiProviderConfig
import com.example.verb.ai.AiProviderSettings
import com.example.verb.ai.AndroidKeystoreAiProviderSettingsStore
import com.example.verb.ai.DefaultAiProviderClientFactory
import com.example.verb.intent.IntentEngine
import com.example.verb.model.ActionResult
import com.example.verb.model.SemanticEntity
import com.example.verb.semantic.SemanticEngine
import com.example.verb.terminal.TerminalRuntime
import com.example.verb.terminal.RuntimeArtifactImporter
import com.example.verb.terminal.TerminalEnvironment
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

enum class VerbTab {
    ASK,
    ASSISTANT,
    SYSTEM,
    TERMINAL
}

class VerbViewModel(application: Application) : AndroidViewModel(application) {

    private val intentEngine = IntentEngine()
    private val actionRegistry = ActionRegistry(application.applicationContext)
    private val semanticEngine = SemanticEngine()
    private val aiProviderSettingsStore = AndroidKeystoreAiProviderSettingsStore(application.applicationContext)
    private val aiProviderClientFactory = DefaultAiProviderClientFactory()
    private val aiAssistantService = AiAssistantService(
        settingsStore = aiProviderSettingsStore,
        clientFactory = aiProviderClientFactory::invoke
    )

    val terminalRuntime = TerminalRuntime(application.applicationContext.filesDir)
    private val runtimeImporter = RuntimeArtifactImporter(
        contentResolver = application.contentResolver,
        appFilesDir = application.applicationContext.filesDir
    )
    private val _runtimeImportState = MutableStateFlow<RuntimeImportState>(RuntimeImportState.Idle)
    val runtimeImportState: StateFlow<RuntimeImportState> = _runtimeImportState.asStateFlow()
    val terminalEnvironment: StateFlow<TerminalEnvironment> = terminalRuntime.environmentState

    private val _activeTab = MutableStateFlow(VerbTab.ASK)
    val activeTab: StateFlow<VerbTab> = _activeTab.asStateFlow()

    private val _aiProviderSettings = MutableStateFlow(aiProviderSettingsStore.load())
    val aiProviderSettings: StateFlow<AiProviderSettings> = _aiProviderSettings.asStateFlow()

    private val _assistantInput = MutableStateFlow("")
    val assistantInput: StateFlow<String> = _assistantInput.asStateFlow()

    private val _assistantState = MutableStateFlow<AiAssistantState>(AiAssistantState.Idle)
    val assistantState: StateFlow<AiAssistantState> = _assistantState.asStateFlow()

    private val _queryInput = MutableStateFlow("")
    val queryInput: StateFlow<String> = _queryInput.asStateFlow()

    private val _currentActionResult = MutableStateFlow<ActionResult?>(null)
    val currentActionResult: StateFlow<ActionResult?> = _currentActionResult.asStateFlow()

    private val _isExecuting = MutableStateFlow(false)
    val isExecuting: StateFlow<Boolean> = _isExecuting.asStateFlow()

    private val _historyList = MutableStateFlow<List<ActionResult>>(emptyList())
    val historyList: StateFlow<List<ActionResult>> = _historyList.asStateFlow()

    private val _activeSemanticEntity = MutableStateFlow<SemanticEntity?>(null)
    val activeSemanticEntity: StateFlow<SemanticEntity?> = _activeSemanticEntity.asStateFlow()

    private val _confirmationPendingResult = MutableStateFlow<ActionResult?>(null)
    val confirmationPendingResult: StateFlow<ActionResult?> = _confirmationPendingResult.asStateFlow()

    init {
        // Execute initial default storage check on launch to present structured home state immediately
        submitQuery("show me my storage")
    }

    fun selectTab(tab: VerbTab) {
        _activeTab.value = tab
    }

    fun updateQueryInput(newInput: String) {
        _queryInput.value = newInput
    }

    fun updateAssistantInput(newInput: String) {
        _assistantInput.value = newInput
    }

    fun saveAiProviderSettings(config: AiProviderConfig, apiKey: String?): Result<Unit> = runCatching {
        aiProviderSettingsStore.save(config, apiKey)
        _aiProviderSettings.value = aiProviderSettingsStore.load()
    }

    fun clearAiProviderApiKey() {
        aiProviderSettingsStore.clearApiKey()
        _aiProviderSettings.value = aiProviderSettingsStore.load()
    }

    fun importRuntime(zipUri: Uri, checksumUri: Uri) {
        if (_runtimeImportState.value is RuntimeImportState.Importing) return
        _runtimeImportState.value = RuntimeImportState.Importing
        viewModelScope.launch(Dispatchers.IO) {
            val result = runtimeImporter.importArtifact(zipUri, checksumUri)
            _runtimeImportState.value = result.fold(
                onSuccess = {
                    terminalRuntime.restartSession()
                    RuntimeImportState.Success
                },
                onFailure = { RuntimeImportState.Failure(it.message ?: "Runtime import failed.") }
            )
        }
    }

    fun openAssistant() {
        _activeTab.value = VerbTab.ASSISTANT
    }

    fun submitAssistantPrompt(prompt: String) {
        if (prompt.isBlank() || _assistantState.value is AiAssistantState.Generating) return
        _assistantInput.value = prompt
        _assistantState.value = AiAssistantState.Generating
        viewModelScope.launch(Dispatchers.IO) {
            _assistantState.value = try {
                AiAssistantState.Answer(aiAssistantService.respond(AiAssistantRequest(prompt)))
            } catch (exception: Exception) {
                AiAssistantState.Failure(exception.message ?: "The assistant could not complete this request.")
            }
        }
    }

    fun submitIntent(intent: com.example.verb.model.VerbIntent) {
        _isExecuting.value = true
        _queryInput.value = intent.summary
        if (intent.id != "terminal.open") {
            _activeTab.value = VerbTab.ASK
        }
        viewModelScope.launch(Dispatchers.IO) {
            try {
                if (intent.id == "terminal.open") {
                    _activeTab.value = VerbTab.TERMINAL
                    return@launch
                }
                handleActionResult(actionRegistry.executeAction(intent, confirmed = false))
            } catch (e: Exception) {
                handleActionResult(unexpectedFailure(intent, e))
            } finally {
                _isExecuting.value = false
            }
        }
    }

    fun submitQuery(query: String) {
        if (query.isBlank()) return
        _isExecuting.value = true
        _queryInput.value = query

        viewModelScope.launch(Dispatchers.IO) {
            var intent: com.example.verb.model.VerbIntent? = null
            try {
                val resolvedIntent = intentEngine.resolveIntent(query)
                intent = resolvedIntent
                if (resolvedIntent.id == "terminal.open") {
                    _activeTab.value = VerbTab.TERMINAL
                    return@launch
                }
                handleActionResult(actionRegistry.executeAction(resolvedIntent, confirmed = false))
            } catch (e: Exception) {
                handleActionResult(unexpectedFailure(intent, e))
            } finally {
                _isExecuting.value = false
            }
        }
    }

    fun confirmPendingAction() {
        val pending = _confirmationPendingResult.value ?: return
        _confirmationPendingResult.value = null
        _isExecuting.value = true

        viewModelScope.launch(Dispatchers.IO) {
            val intent = pending.originalIntent
            try {
                if (intent == null) {
                    handleActionResult(unexpectedFailure(null, IllegalStateException("Missing confirmed intent.")))
                } else {
                    handleActionResult(actionRegistry.executeAction(intent, confirmed = true))
                }
            } catch (e: Exception) {
                handleActionResult(unexpectedFailure(intent, e))
            } finally {
                _isExecuting.value = false
            }
        }
    }

    fun dismissConfirmation() {
        _confirmationPendingResult.value = null
    }

    fun inspectSemanticText(text: String, contextText: String? = null) {
        if (text.isBlank()) return
        viewModelScope.launch(Dispatchers.Default) {
            val entity = semanticEngine.analyzeText(text, contextText)
            _activeSemanticEntity.value = entity
        }
    }

    fun closeSemanticLens() {
        _activeSemanticEntity.value = null
    }

    fun openTerminal() {
        _activeTab.value = VerbTab.TERMINAL
    }

    private fun handleActionResult(result: ActionResult) {
        if (result.requiresConfirmation) {
            _confirmationPendingResult.value = result
        } else {
            _currentActionResult.value = result
            _historyList.value = listOf(result) + _historyList.value.take(9)
        }
    }

    private fun unexpectedFailure(
        intent: com.example.verb.model.VerbIntent?,
        error: Exception
    ): ActionResult = ActionResult(
        intentId = intent?.id ?: "internal.error",
        title = "Action Failed",
        summary = "Verb could not complete this action.",
        isSuccess = false,
        errorMessage = error.localizedMessage ?: "Unexpected runtime error.",
        originalIntent = intent
    )

    override fun onCleared() {
        super.onCleared()
        terminalRuntime.destroy()
    }
}

sealed interface RuntimeImportState {
    data object Idle : RuntimeImportState
    data object Importing : RuntimeImportState
    data object Success : RuntimeImportState
    data class Failure(val message: String) : RuntimeImportState
}
