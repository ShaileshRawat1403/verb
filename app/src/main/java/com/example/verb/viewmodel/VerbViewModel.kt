package com.example.verb.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.example.verb.actions.ActionRegistry
import com.example.verb.intent.IntentEngine
import com.example.verb.model.ActionResult
import com.example.verb.model.SemanticEntity
import com.example.verb.semantic.SemanticEngine
import com.example.verb.terminal.TerminalRuntime
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

enum class VerbTab {
    ASK,
    SYSTEM,
    TERMINAL
}

class VerbViewModel(application: Application) : AndroidViewModel(application) {

    private val intentEngine = IntentEngine()
    private val actionRegistry = ActionRegistry(application.applicationContext)
    private val semanticEngine = SemanticEngine()

    val terminalRuntime = TerminalRuntime(application.applicationContext.filesDir)

    private val _activeTab = MutableStateFlow(VerbTab.ASK)
    val activeTab: StateFlow<VerbTab> = _activeTab.asStateFlow()

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
