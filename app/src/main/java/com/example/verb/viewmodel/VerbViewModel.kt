package com.example.verb.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.example.BuildConfig
import com.example.verb.actions.ActionRegistry
import com.example.verb.ai.AiAssistantRequest
import com.example.verb.ai.AiAssistantService
import com.example.verb.ai.AiAssistantState
import com.example.verb.ai.AiProviderConfig
import com.example.verb.ai.AiProviderSettings
import com.example.verb.ai.AndroidKeystoreAiProviderSettingsStore
import com.example.verb.ai.DefaultAiProviderClientFactory
import com.example.verb.db.CommandHistoryEntity
import com.example.verb.db.VerbRepository
import com.example.verb.intent.IntentEngine
import com.example.verb.model.ActionResult
import com.example.verb.model.ChatMessage
import com.example.verb.model.ChatSender
import com.example.verb.model.SemanticEntity
import com.example.verb.project.ProjectRepository
import com.example.verb.project.VerbProject
import com.example.verb.semantic.SecretGuard
import com.example.verb.semantic.SemanticEngine
import com.example.verb.terminal.BundledToolBootstrap
import com.example.verb.terminal.AgentArtifactState
import com.example.verb.terminal.AgentCompatibilityState
import com.example.verb.terminal.AgentRuntimeCompatibilityProbe
import com.example.verb.terminal.AgentRuntimeInstaller
import com.example.verb.terminal.AgentRuntimeStatus
import com.example.verb.terminal.AgentRuntimeManifest
import com.example.verb.terminal.LogCategory
import com.example.verb.terminal.RuntimeCapabilityDetector
import com.example.verb.terminal.RuntimeProfile
import com.example.verb.terminal.RuntimeProfileId
import com.example.verb.terminal.RuntimeProfileReport
import com.example.verb.terminal.RuntimeProfiles
import com.example.verb.terminal.TerminalAiHelper
import com.example.verb.terminal.TerminalRuntime
import com.example.verb.terminal.TerminalSessionLogger
import com.example.verb.terminal.TermuxBootstrapInstaller
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import java.io.File

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
    private val repository = VerbRepository.getInstance(application.applicationContext)
    private val projectRepository = ProjectRepository(application.applicationContext)
    private val runtimeCapabilityDetector = RuntimeCapabilityDetector(application.filesDir)
    private val agentRuntimeInstaller = AgentRuntimeInstaller(application.filesDir)

    /**
     * Installs bundled CLI tools (busybox/curl/jq + CA bundle) with ELF validation. The terminal
     * runtime only picks the bundled bin directory up when at least one binary validated; corrupt
     * assets are skipped rather than ever being offered to the shell.
     */
    private val bundledTools = if (BuildConfig.FULL_CLI) {
        BundledToolBootstrap.install(application.applicationContext)
    } else {
        BundledToolBootstrap.Result(binDir = null, installed = emptyList(), skipped = emptyList())
    }
    private val bundledBinDir: File? = bundledTools.binDir?.takeIf { bundledTools.isReady }

    val terminalRuntime = TerminalRuntime(
        workingDir = application.applicationContext.filesDir,
        bundledBinDir = bundledBinDir,
        initialProjectDirectory = projectRepository.selected()?.directory
    )

    private val _projects = MutableStateFlow(projectRepository.list())
    val projects: StateFlow<List<VerbProject>> = _projects.asStateFlow()
    private val _selectedProject = MutableStateFlow(projectRepository.selected())
    val selectedProject: StateFlow<VerbProject?> = _selectedProject.asStateFlow()

    private val _terminalBootstrapState =
        MutableStateFlow<TermuxBootstrapInstaller.State>(TermuxBootstrapInstaller.State.NotStarted)
    val terminalBootstrapState: StateFlow<TermuxBootstrapInstaller.State> = _terminalBootstrapState.asStateFlow()

    private val _runtimeProfileReports = MutableStateFlow(runtimeReports())
    val runtimeProfileReports: StateFlow<List<RuntimeProfileReport>> = _runtimeProfileReports.asStateFlow()

    private val _runtimeInstallingProfile = MutableStateFlow<RuntimeProfileId?>(null)
    val runtimeInstallingProfile: StateFlow<RuntimeProfileId?> = _runtimeInstallingProfile.asStateFlow()

    private val _runtimeInstallMessage = MutableStateFlow<String?>(null)
    val runtimeInstallMessage: StateFlow<String?> = _runtimeInstallMessage.asStateFlow()

    private val agentRuntimeProbe = AgentRuntimeCompatibilityProbe(application.filesDir)

    /**
     * Artifact presence and executability, kept as separate facts. Verb used to offer a launch
     * button on the strength of files existing on disk; on a device where the runtime cannot
     * execute, that produced a session that died instantly with no explanation.
     */
    private val _agentRuntimeStatus = MutableStateFlow(
        agentRuntimeInstaller.active().let { installed ->
            AgentRuntimeStatus(
                artifact = if (installed != null) AgentArtifactState.INSTALLED else AgentArtifactState.NOT_INSTALLED,
                compatibility = AgentCompatibilityState.NOT_CHECKED,
                runtime = installed
            )
        }
    )
    val agentRuntimeStatus: StateFlow<AgentRuntimeStatus> = _agentRuntimeStatus.asStateFlow()


    private val _agentRuntimeImporting = MutableStateFlow(false)
    val agentRuntimeImporting: StateFlow<Boolean> = _agentRuntimeImporting.asStateFlow()

    private val _agentRuntimeMessage = MutableStateFlow<String?>(null)
    val agentRuntimeMessage: StateFlow<String?> = _agentRuntimeMessage.asStateFlow()

    // The terminal is the primary product surface, so the app lands on it at launch rather than
    // the query/assistant tabs where raw shell commands would appear to do nothing.
    private val _activeTab = MutableStateFlow(VerbTab.TERMINAL)
    val activeTab: StateFlow<VerbTab> = _activeTab.asStateFlow()

    // Tabs visited before the current one, newest first. Back gestures retrace this order until it
    // empties (the terminal root), at which point back exits the app.
    private val _tabBackStack = MutableStateFlow<List<VerbTab>>(emptyList())
    val tabBackStack: StateFlow<List<VerbTab>> = _tabBackStack.asStateFlow()

    // Reliable keyboard visibility driven by the Activity's window insets (edge-to-edge), not the
    // Compose-side isImeVisible read which can report a stale true on this configuration.
    private val _isKeyboardVisible = MutableStateFlow(false)
    val isKeyboardVisible: StateFlow<Boolean> = _isKeyboardVisible.asStateFlow()

    fun setKeyboardVisible(visible: Boolean) {
        if (_isKeyboardVisible.value != visible) {
            _isKeyboardVisible.value = visible
        }
    }

    private companion object {
        const val MAX_TAB_BACK_STACK = 8
        const val PROFILE_INSTALL_TIMEOUT_MS = 15 * 60 * 1000L
    }

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

    private val _persistedCommandHistory = MutableStateFlow<List<CommandHistoryEntity>>(emptyList())
    val persistedCommandHistory: StateFlow<List<CommandHistoryEntity>> = _persistedCommandHistory.asStateFlow()

    private val _terminalAiExplanation = MutableStateFlow<String?>(null)
    val terminalAiExplanation: StateFlow<String?> = _terminalAiExplanation.asStateFlow()

    private val _isTerminalAiExplaining = MutableStateFlow(false)
    val isTerminalAiExplaining: StateFlow<Boolean> = _isTerminalAiExplaining.asStateFlow()

    init {
        TerminalSessionLogger.info(
            LogCategory.DIAGNOSTIC,
            "Bundled tools: installed=${bundledTools.installed}, skipped=${bundledTools.skipped.size}"
        )

        installTermuxBootstrap()

        // An artifact found on disk at startup is unverified until proven otherwise, so check it
        // once here rather than presenting it as launchable.
        checkAgentRuntimeCompatibility()

        // Mirror persisted command history into UI state as it changes
        viewModelScope.launch(Dispatchers.IO) {
            repository.commandHistory.collect { _persistedCommandHistory.value = it }
        }
    }

    private fun runtimeReports(): List<RuntimeProfileReport> =
        RuntimeProfiles.all.map(runtimeCapabilityDetector::inspect)

    private fun refreshRuntimeProfiles() {
        _runtimeProfileReports.value = runtimeReports()
    }

    /**
     * Ensures the full Verb CLI userland is present, downloading and installing it on first
     * launch. Once it is ready the terminal runtime is re-resolved so the live PTY restarts under
     * proot with the Verb environment.
     */
    private fun installTermuxBootstrap() {
        val context = getApplication<Application>()
        if (!BuildConfig.FULL_CLI) {
            _terminalBootstrapState.value = TermuxBootstrapInstaller.State.NotStarted
            return
        }
        // Best-effort app-local copy of the linker config so proot can bind /linkerconfig without
        // hitting the Android 12+ EACCES on the real directory. Runs every launch, not just on
        // install, so already-provisioned devices also get the fix.
        TermuxBootstrapInstaller.ensureGuestLinkerConfig(context.filesDir)
        TermuxBootstrapInstaller.ensureSecureAptSources(context.filesDir)
        // Same "every launch, not just install" requirement as the two calls above: this used to
        // only run inside TermuxBootstrapInstaller.install(), so it silently never executed once a
        // device already had a bootstrap (the common case after the first launch).
        TermuxBootstrapInstaller.ensureGuestShellStartupCurrent(context.filesDir)
        if (TermuxBootstrapInstaller.isInstalled(context)) {
            _terminalBootstrapState.value = TermuxBootstrapInstaller.State.Ready
            TermuxBootstrapInstaller.ensureGuestDns(context)
            terminalRuntime.refreshEnvironment()
            refreshRuntimeProfiles()
            return
        }
        viewModelScope.launch(Dispatchers.IO) {
            TermuxBootstrapInstaller.install(context) { state ->
                _terminalBootstrapState.value = state
                if (state is TermuxBootstrapInstaller.State.Ready) {
                    TermuxBootstrapInstaller.ensureGuestDns(context)
                    viewModelScope.launch(Dispatchers.Main.immediate) {
                        terminalRuntime.refreshEnvironment()
                        refreshRuntimeProfiles()
                    }
                }
            }
        }
    }

    fun retryTermuxBootstrap() {
        installTermuxBootstrap()

        // An artifact found on disk at startup is unverified until proven otherwise, so check it
        // once here rather than presenting it as launchable.
        checkAgentRuntimeCompatibility()
    }

    /** Imports a verified CI artifact into a versioned, rollback-safe Agent Runtime slot. */
    fun importAgentRuntime(archiveUri: android.net.Uri, checksumUri: android.net.Uri, manifestUri: android.net.Uri) {
        if (_agentRuntimeImporting.value) return
        _agentRuntimeImporting.value = true
        _agentRuntimeMessage.value = "Verifying and installing Agent Runtime…"
        val context = getApplication<Application>()
        viewModelScope.launch(Dispatchers.IO) {
            val result = runCatching {
                val staging = File(context.cacheDir, "agent-runtime-import")
                staging.deleteRecursively()
                require(staging.mkdirs()) { "Could not create Agent Runtime staging storage." }
                val archive = File(staging, "agent-runtime-rootfs.tar.gz")
                val checksum = File(staging, "agent-runtime-rootfs.tar.gz.sha256")
                val manifest = File(staging, "agent-runtime-manifest.txt")
                copyDocument(context.contentResolver, archiveUri, archive)
                copyDocument(context.contentResolver, checksumUri, checksum)
                copyDocument(context.contentResolver, manifestUri, manifest)
                agentRuntimeInstaller.install(archive, checksum, manifest).getOrThrow()
            }
            withContext(Dispatchers.Main.immediate) {
                _agentRuntimeImporting.value = false
                result.onSuccess { installed ->
                    // "installed", never "ready": the artifact verified and extracted, which says
                    // nothing about whether it can execute. Readiness is claimed only by the probe.
                    _agentRuntimeStatus.value = AgentRuntimeStatus(
                        artifact = AgentArtifactState.INSTALLED,
                        compatibility = AgentCompatibilityState.NOT_CHECKED,
                        runtime = installed
                    )
                    _agentRuntimeMessage.value = "Agent Runtime ${installed.manifest.runtimeVersion} installed."
                    checkAgentRuntimeCompatibility()
                }.onFailure { error ->
                    _agentRuntimeMessage.value = "Agent Runtime import failed: ${error.message ?: "invalid artifact"}"
                }
            }
        }
    }

    /**
     * Runs the bounded compatibility probe for the installed runtime. Called once when an existing
     * artifact is found at startup, once after a successful import, and on explicit retry -- never
     * on a timer and never in a loop: [AgentRuntimeStatus.canCheck] is false while one is in flight,
     * so a repeated trigger is a no-op rather than a second probe.
     */
    fun checkAgentRuntimeCompatibility() {
        val status = _agentRuntimeStatus.value
        val runtime = status.runtime ?: return
        if (!status.canCheck) return

        _agentRuntimeStatus.value = status.copy(compatibility = AgentCompatibilityState.CHECKING)
        viewModelScope.launch(Dispatchers.IO) {
            val result = agentRuntimeProbe.check(runtime)
            withContext(Dispatchers.Main.immediate) {
                _agentRuntimeStatus.value = _agentRuntimeStatus.value.copy(compatibility = result)
                _agentRuntimeMessage.value = agentRuntimeMessageFor(result, runtime.manifest.runtimeVersion)
            }
        }
    }

    /**
     * User-facing wording. It never names a specific Android policy: the device evidence showed the
     * runtime cannot execute here, but which restriction is fatal was never uniquely identified, and
     * claiming one would be a guess presented as a fact.
     */
    private fun agentRuntimeMessageFor(state: AgentCompatibilityState, version: String): String = when (state) {
        AgentCompatibilityState.COMPATIBLE -> "Agent Runtime $version is ready."
        AgentCompatibilityState.INCOMPATIBLE ->
            "Installed, but incompatible on this device. This Linux runtime cannot execute inside " +
                "this Android app sandbox. The normal Verb terminal is unaffected."
        AgentCompatibilityState.CHECK_TIMED_OUT ->
            "Compatibility check timed out. The Agent Runtime is installed but unverified."
        AgentCompatibilityState.CHECK_FAILED ->
            "Compatibility check could not run. The Agent Runtime is installed but unverified."
        AgentCompatibilityState.CHECKING -> "Checking Agent Runtime compatibility…"
        AgentCompatibilityState.NOT_CHECKED -> "Agent Runtime $version installed."
    }

    /**
     * Refuses unless the runtime has been proven to execute. This is the programmatic guard behind
     * the disabled button: reaching the runtime by any other path must not bypass the check.
     */
    fun openAgentRuntime() {
        val status = _agentRuntimeStatus.value
        val runtime = status.runtime
        if (runtime == null) {
            _agentRuntimeMessage.value = "Import an Agent Runtime artifact first."
            return
        }
        if (!status.canOpen) {
            _agentRuntimeMessage.value = agentRuntimeMessageFor(status.compatibility, runtime.manifest.runtimeVersion)
            return
        }
        runCatching { terminalRuntime.activateAgentRuntime(runtime) }
            .onSuccess { selectTab(VerbTab.TERMINAL) }
            .onFailure { _agentRuntimeMessage.value = it.message ?: "Could not open Agent Runtime." }
    }

    fun returnToVerbRuntime() {
        terminalRuntime.deactivateAgentRuntime()
        _agentRuntimeMessage.value = "Returned to the Verb CLI userland."
    }

    private fun copyDocument(resolver: android.content.ContentResolver, uri: android.net.Uri, destination: File) {
        resolver.openInputStream(uri)?.use { input ->
            destination.outputStream().use { output -> input.copyTo(output) }
        } ?: error("Could not read selected Agent Runtime document.")
    }

    fun installRuntimeProfile(profileId: RuntimeProfileId) {
        if (_runtimeInstallingProfile.value != null) return
        if (!BuildConfig.FULL_CLI) {
            _runtimeInstallMessage.value =
                "Package profiles require the Full CLI distribution; the Play build uses Android's system shell."
            return
        }
        val profile = RuntimeProfiles.forId(profileId)
        val report = runtimeCapabilityDetector.inspect(profile)
        if (report.isReady) {
            _runtimeInstallMessage.value = "${profile.displayName} is already ready."
            refreshRuntimeProfiles()
            return
        }
        if (report.isUnsatisfiable) {
            _runtimeInstallMessage.value = unsatisfiableMessage(profile, report)
            return
        }
        if (_terminalBootstrapState.value !is TermuxBootstrapInstaller.State.Ready) {
            _runtimeInstallMessage.value = "Finish the core runtime setup before adding profiles."
            return
        }

        // Resolve the dependency graph instead of handing it back to the user. Asking for Codex CLI
        // means asking for whatever Codex needs, and Verb already knows what that is.
        val plan = RuntimeProfiles.installPlan(profileId) { runtimeCapabilityDetector.inspect(RuntimeProfiles.forId(it)).isReady }
        if (plan.isEmpty()) {
            _runtimeInstallMessage.value = "${profile.displayName} is already ready."
            refreshRuntimeProfiles()
            return
        }
        // Refuse the whole plan rather than installing part of it: a prerequisite that can never be
        // satisfied would otherwise leave the user with a half-provisioned runtime and no signal.
        val blocked = plan.firstOrNull { runtimeCapabilityDetector.inspect(it).isUnsatisfiable }
        if (blocked != null) {
            _runtimeInstallMessage.value =
                unsatisfiableMessage(blocked, runtimeCapabilityDetector.inspect(blocked)) +
                    if (blocked.id != profileId) " ${profile.displayName} depends on it." else ""
            return
        }

        _runtimeInstallingProfile.value = profileId
        viewModelScope.launch(Dispatchers.IO) {
            var failure: String? = null
            for ((index, step) in plan.withIndex()) {
                val progress = if (plan.size > 1) " (${index + 1} of ${plan.size})" else ""
                withContext(Dispatchers.Main.immediate) {
                    _runtimeInstallMessage.value = "Installing ${step.displayName}$progress..."
                }
                val outcome = installOneProfile(step)
                if (outcome != null) {
                    failure = outcome
                    break
                }
            }
            withContext(Dispatchers.Main.immediate) {
                refreshRuntimeProfiles()
                _runtimeInstallingProfile.value = null
                _runtimeInstallMessage.value = failure ?: if (plan.size > 1) {
                    "${profile.displayName} installed, with ${plan.size - 1} prerequisite" +
                        (if (plan.size > 2) "s." else ".")
                } else {
                    "${profile.displayName} installed."
                }
            }
        }
    }

    /**
     * Runs one profile's install command in the real terminal and waits for it to finish.
     * Returns null on success, or a user-facing failure message.
     *
     * Completion is still detected by the transcript marker plus a capability re-probe, unchanged
     * from before: the marker proves the command's own exit status, and the re-probe covers a
     * snapshot the throttled transcript may have coalesced away.
     */
    private suspend fun installOneProfile(profile: RuntimeProfile): String? {
        val marker = "__VERB_PROFILE_${profile.id.name}_${System.currentTimeMillis()}__"
        val command = "${profile.installCommand}; profile_status=${'$'}?; printf '\\n$marker:%s\\n' \"${'$'}profile_status\""
        withContext(Dispatchers.Main.immediate) {
            terminalRuntime.sendCommand(command)
            recordTerminalCommand(command)
        }
        val completed: String? = withTimeoutOrNull(PROFILE_INSTALL_TIMEOUT_MS) {
            var result: String? = null
            while (result == null) {
                val output = terminalRuntime.terminalOutput.value
                if (output.contains(marker)) {
                    result = output
                    continue
                }
                if (runtimeCapabilityDetector.inspect(profile).isReady) {
                    result = "profile-ready"
                    continue
                }
                delay(500)
            }
            result
        }
        return when {
            completed == null -> "${profile.displayName} timed out; inspect the terminal output."
            completed == "profile-ready" -> null
            completed.substringAfter("$marker:").trim().startsWith("0") -> null
            else -> "${profile.displayName} failed; inspect the terminal output."
        }
    }

    /**
     * Says plainly that nothing the user can do will help, and why. Naming the required version
     * matters: "Incompatible: python" reads like a mistake the user made, when in fact the package
     * repository simply does not ship a version this profile can use.
     */
    private fun unsatisfiableMessage(profile: RuntimeProfile, report: RuntimeProfileReport): String {
        val detail = profile.requirements
            .filter { it.command in report.incompatibleCommands && it.maxVersionExclusive != null }
            .joinToString { "${it.command} below ${it.maxVersionExclusive}" }
            .ifEmpty { report.incompatibleCommands.joinToString() }
        return "${profile.displayName} cannot run on this device: it needs $detail, and the package " +
            "repository does not provide a compatible version. No install will resolve this."
    }

    fun selectTab(tab: VerbTab) {
        val current = _activeTab.value
        if (current == tab) return
        // Push the outgoing tab so the system back gesture retraces tab hops in order.
        val stack = _tabBackStack.value
        if (stack.lastOrNull() != current && stack.size < MAX_TAB_BACK_STACK) {
            _tabBackStack.value = stack + current
        }
        _activeTab.value = tab
    }

    /**
     * Pops the tab history. Returns false when already at the root tab so the caller can decide
     * whether the app should exit.
     */
    fun navigateBack(): Boolean {
        val stack = _tabBackStack.value
        if (stack.isEmpty()) return false
        _tabBackStack.value = stack.dropLast(1)
        _activeTab.value = stack.last()
        return true
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

    fun openAssistant() {
        selectTab(VerbTab.ASSISTANT)
    }

    fun submitAssistantPrompt(prompt: String) {
        if (prompt.isBlank() || _assistantState.value is AiAssistantState.Generating) return
        _assistantInput.value = prompt
        _assistantState.value = AiAssistantState.Generating
        viewModelScope.launch(Dispatchers.IO) {
            _assistantState.value = try {
                val response = aiAssistantService.respond(AiAssistantRequest(prompt))
                runCatching {
                    repository.saveChatMessage(ChatMessage(sender = ChatSender.USER, text = prompt))
                    repository.saveChatMessage(ChatMessage(sender = ChatSender.AGENT, text = response.text))
                }
                AiAssistantState.Answer(response)
            } catch (exception: Exception) {
                AiAssistantState.Failure(exception.message ?: "The assistant could not complete this request.")
            }
        }
    }

    fun submitIntent(intent: com.example.verb.model.VerbIntent) {
        _isExecuting.value = true
        _queryInput.value = intent.summary
        if (intent.id != "terminal.open") {
            selectTab(VerbTab.ASK)
        }
        viewModelScope.launch(Dispatchers.IO) {
            try {
                if (intent.id == "terminal.open") {
                    selectTab(VerbTab.TERMINAL)
                    return@launch
                }
                handleActionResult(actionRegistry.executeAction(intent, confirmed = false), query = intent.summary)
            } catch (e: Exception) {
                handleActionResult(unexpectedFailure(intent, e), query = intent.summary)
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
                    selectTab(VerbTab.TERMINAL)
                    return@launch
                }
                handleActionResult(actionRegistry.executeAction(resolvedIntent, confirmed = false), query = query)
            } catch (e: Exception) {
                handleActionResult(unexpectedFailure(intent, e), query = query)
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
                    handleActionResult(unexpectedFailure(null, IllegalStateException("Missing confirmed intent.")), query = pending.title)
                } else {
                    handleActionResult(actionRegistry.executeAction(intent, confirmed = true), query = intent.summary)
                }
            } catch (e: Exception) {
                handleActionResult(unexpectedFailure(intent, e), query = intent?.summary)
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
        selectTab(VerbTab.TERMINAL)
    }

    fun createProject(name: String) {
        runCatching { projectRepository.create(name) }.getOrNull()?.let { project ->
            _projects.value = projectRepository.list()
            _selectedProject.value = project
            terminalRuntime.selectProject(project.directory)
        }
    }

    fun selectProject(id: String) {
        projectRepository.select(id)?.let { project ->
            if (_selectedProject.value?.id == project.id) return
            _selectedProject.value = project
            terminalRuntime.selectProject(project.directory)
        }
    }

    /**
     * Sends a command to the real PTY and records a bounded transcript snapshot to Room. The
     * command text is written to the TTY only; it is never parsed or executed by the AI layer.
     */
    fun sendTerminalCommand(cmd: String) {
        terminalRuntime.sendCommand(cmd)
        recordTerminalCommand(cmd)
    }

    /**
     * Records a command + bounded transcript snapshot to Room without writing to the PTY. The
     * mobile keyboard's live-echo typing path sends characters to the shell as they are typed and
     * only submits a trailing newline through [sendTerminalCommand], so it calls this directly
     * with the real typed text to keep command history meaningful instead of blank.
     */
    fun recordTerminalCommand(cmd: String) {
        if (cmd.isBlank()) return
        TerminalSessionLogger.info(LogCategory.IO, "Terminal command submitted (${cmd.length} chars)")
        viewModelScope.launch(Dispatchers.IO) {
            runCatching {
                // Redacted through SecretGuard before it ever reaches disk, and capped well below
                // the 50k in-memory buffer: this snapshot is written on every command send, so an
                // unbounded copy would grow the local database roughly with (commands x buffer
                // size) over a session.
                repository.recordTerminalOutput(
                    command = SecretGuard.redactKnownSensitiveText(cmd),
                    output = SecretGuard.redactKnownSensitiveText(terminalRuntime.terminalOutput.value).takeLast(4_000),
                    // The shell's own directory at submission time, as a guest path, or null when
                    // it is unknown. The launch directory is deliberately NOT substituted here: a
                    // stored row claiming a directory the command did not run in is worse than a
                    // row that admits it does not know. The column is already nullable.
                    workingDir = terminalRuntime.currentWorkingDirectory.value?.guestPath
                )
            }
        }
    }

    /**
     * Asks the user-configured provider to explain the recent terminal output. The transcript is
     * redacted through [com.example.verb.semantic.SecretGuard] before leaving the device.
     */
    fun explainTerminalOutput() {
        if (_isTerminalAiExplaining.value) return
        val output = terminalRuntime.terminalOutput.value
        if (output.isBlank()) return

        _isTerminalAiExplaining.value = true
        viewModelScope.launch(Dispatchers.IO) {
            _terminalAiExplanation.value = try {
                TerminalAiHelper.analyze(
                    service = aiAssistantService,
                    output = output,
                    workingDir = terminalRuntime.currentWorkingDirectory.value?.guestPath,
                    sessionState = terminalRuntime.sessionState.value
                )
            } catch (e: Exception) {
                e.message ?: "AI analysis failed."
            }
            _isTerminalAiExplaining.value = false
        }
    }

    fun dismissTerminalAiExplanation() {
        _terminalAiExplanation.value = null
    }

    private fun handleActionResult(result: ActionResult, query: String? = null) {
        if (!result.requiresConfirmation) {
            viewModelScope.launch(Dispatchers.IO) {
                runCatching { repository.recordCommand(query ?: result.title, result) }
            }
        }

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
