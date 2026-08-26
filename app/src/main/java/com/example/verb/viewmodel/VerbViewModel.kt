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
import com.example.verb.intent.IntentEngine
import com.example.verb.model.ActionResult
import com.example.verb.model.SemanticEntity
import com.example.verb.project.ProjectRepository
import com.example.verb.project.VerbProject
import com.example.verb.semantic.SemanticEngine
import com.example.verb.terminal.BundledToolBootstrap
import com.example.verb.terminal.AgentArtifactState
import com.example.verb.terminal.AgentCompatibilityState
import com.example.verb.terminal.AgentRuntimeCompatibilityProbe
import com.example.verb.terminal.AgentRuntimeInstaller
import com.example.verb.terminal.AgentRuntimeStatus
import com.example.verb.terminal.AgentRuntimeManifest
import com.example.verb.terminal.LogCategory
import com.example.verb.terminal.MuslLoaderBootstrap
import com.example.verb.terminal.TerminalHoldService
import com.example.verb.terminal.TerminalSessionState
import com.example.verb.ui.AgentKeyStatus
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
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import java.io.File


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
    private val projectRepository = ProjectRepository(application.applicationContext)
    private val runtimeCapabilityDetector = RuntimeCapabilityDetector(application.filesDir)
    private val agentRuntimeInstaller = AgentRuntimeInstaller(application.filesDir)
    private val claudeSessionStore =
        com.example.verb.session.SharedPreferencesVerbSessionStore(application.applicationContext)

    /** A separate store per agent, so launching one agent never overwrites another's recovery record. */
    private val codexSessionStore = com.example.verb.session.SharedPreferencesVerbSessionStore(
        application.applicationContext,
        com.example.verb.session.SharedPreferencesVerbSessionStore.CODEX_PREFERENCES_NAME
    )

    private val openCodeSessionStore = com.example.verb.session.SharedPreferencesVerbSessionStore(
        application.applicationContext,
        com.example.verb.session.SharedPreferencesVerbSessionStore.OPENCODE_PREFERENCES_NAME
    )

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

    /**
     * Held by [VerbTerminalSessionHolder], not constructed fresh here: this ViewModel's lifetime is
     * the screen's, and the session must outlive that (see [onCleared] and
     * `docs/VERB_SESSION_CONTRACT.md`). A VerbViewModel created because the Activity was recreated
     * for real reattaches to the same TerminalRuntime instead of spawning a duplicate session.
     */
    /** True only when this Android process already owned the PTY before this ViewModel was built. */
    private val hadExistingTerminalRuntime =
        com.example.verb.session.VerbTerminalSessionHolder.existing() != null

    val terminalRuntime = com.example.verb.session.VerbTerminalSessionHolder.getOrCreate {
        TerminalRuntime(
            workingDir = application.applicationContext.filesDir,
            bundledBinDir = bundledBinDir,
            initialProjectDirectory = projectRepository.selected()?.directory
        )
    }

    /**
     * One [com.example.verb.session.AgentSessionCoordinator] per recoverable agent -- see
     * `docs/VERB_SESSION_CONTRACT.md`. They share the lifecycle code and differ only by adapter and
     * store. Deliberately not [com.example.verb.session.VerbTerminalSessionHolder]: that owns the
     * process-scoped `TerminalRuntime` this ViewModel already reattaches to; these own
     * product-level identity for one agent's session running inside it, and the two stay separate
     * on purpose.
     */
    private val sessionCoordinators: Map<RuntimeProfileId, com.example.verb.session.AgentSessionCoordinator> =
        mapOf(
            RuntimeProfileId.CLAUDE_CODE to com.example.verb.session.ClaudeSessionCoordinator(
                filesDir = application.applicationContext.filesDir,
                terminalRuntimeAdapter = terminalRuntime,
                coroutineScope = viewModelScope,
                sessionStore = claudeSessionStore,
                processBindingConfirmed = hadExistingTerminalRuntime
            ),
            RuntimeProfileId.CODEX to com.example.verb.session.CodexSessionCoordinator(
                filesDir = application.applicationContext.filesDir,
                terminalRuntimeAdapter = terminalRuntime,
                coroutineScope = viewModelScope,
                sessionStore = codexSessionStore,
                processBindingConfirmed = hadExistingTerminalRuntime
            ),
            RuntimeProfileId.OPENCODE to com.example.verb.session.OpenCodeSessionCoordinator(
                filesDir = application.applicationContext.filesDir,
                // OpenCode's evidence is a live SQLite database, which the adapter copies before
                // reading; the copy belongs in cache, not in the user's files tree.
                scratchDir = application.applicationContext.cacheDir,
                terminalRuntimeAdapter = terminalRuntime,
                coroutineScope = viewModelScope,
                sessionStore = openCodeSessionStore,
                processBindingConfirmed = hadExistingTerminalRuntime
            )
        )

    /**
     * What the Agents screen renders per card. Combined rather than exposed one flow per agent, so
     * adding OpenCode later is a map entry above and nothing else.
     */
    val agentSessions: StateFlow<Map<RuntimeProfileId, com.example.verb.session.VerbSession>> =
        combine(sessionCoordinators.map { (id, coordinator) -> coordinator.session.map { id to it } }) { pairs ->
            pairs.mapNotNull { (id, session) -> session?.let { id to it } }.toMap()
        }.stateIn(viewModelScope, SharingStarted.Eagerly, emptyMap())

    private val _projects = MutableStateFlow(projectRepository.list())
    val projects: StateFlow<List<VerbProject>> = _projects.asStateFlow()
    private val _selectedProject = MutableStateFlow(projectRepository.selected())
    val selectedProject: StateFlow<VerbProject?> = _selectedProject.asStateFlow()

    private val _terminalBootstrapState =
        MutableStateFlow<TermuxBootstrapInstaller.State>(TermuxBootstrapInstaller.State.NotStarted)
    val terminalBootstrapState: StateFlow<TermuxBootstrapInstaller.State> = _terminalBootstrapState.asStateFlow()

    // Empty until the background probe in init{} completes. Populating this eagerly here means
    // inspecting every runtime profile -- each a real proot-spawned process -- synchronously on the
    // main thread before the first frame, which measured as a 12s+ cold start with 1400+ skipped
    // frames on device. The UI already renders an empty list as "nothing known yet".
    private val _runtimeProfileReports = MutableStateFlow<List<RuntimeProfileReport>>(emptyList())
    val runtimeProfileReports: StateFlow<List<RuntimeProfileReport>> = _runtimeProfileReports.asStateFlow()

    /**
     * Whether each agent has an authenticated session. Presence of a credential file only -- the
     * files are never opened. Recomputed alongside the readiness reports, since signing in happens
     * in the terminal and the tab must reflect it without a restart.
     *
     * Declared here, beside [_runtimeProfileReports], and not further down the file: the `init`
     * block below reaches [refreshRuntimeProfiles], which writes both flows, and a Kotlin property
     * declared after that block is still null when it runs. Putting these anywhere else crashes the
     * app on launch with a NullPointerException inside the ViewModel constructor.
     */
    private val agentSignInDetector =
        com.example.verb.terminal.AgentSignInDetector(getApplication<Application>().filesDir)
    private val _agentSignInStates = MutableStateFlow(readAgentSignInStates())
    val agentSignInStates: StateFlow<Map<RuntimeProfileId, com.example.verb.terminal.AgentSignInState>> =
        _agentSignInStates.asStateFlow()

    private fun readAgentSignInStates(): Map<RuntimeProfileId, com.example.verb.terminal.AgentSignInState> =
        RuntimeProfiles.all.filter { it.isAgent }.associate { it.id to agentSignInDetector.stateFor(it) }

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

    // The terminal is the workspace and the root. There is no permanent navigation: this holds only
    // what Verb has been asked to put in front of it, and [VerbSurface.None] is the resting state.
    private val _surface = MutableStateFlow<VerbSurface>(VerbSurface.None)
    val surface: StateFlow<VerbSurface> = _surface.asStateFlow()

    // Whether the open task was reached through the Verb sheet. Back then returns to that sheet,
    // because the user is retracing the list they chose from; a task opened directly (from the
    // workspace's first action, or from a link inside another task) returns straight to the
    // terminal rather than opening a surface the user never asked for.
    private val _taskOpenedFromSheet = MutableStateFlow(false)

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
        /** The variables the bundled agent CLIs read. Names only; values are never held here. */
        val AGENT_KEY_VARIABLES = listOf(
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "DEEPSEEK_API_KEY",
            "GEMINI_API_KEY"
        )

        const val PROFILE_INSTALL_TIMEOUT_MS = 15 * 60 * 1000L

        /**
         * [RuntimeProfiles.all]'s own launch commands, not separately hand-typed literals: the two
         * must never drift apart, since this is what distinguishes "the user opened a session Verb
         * tracks" from opening any other agent.
         */
        val TRACKED_AGENT_LAUNCH_COMMANDS: Map<RuntimeProfileId, String> =
            listOf(
                RuntimeProfileId.CLAUDE_CODE,
                RuntimeProfileId.CODEX,
                RuntimeProfileId.OPENCODE
            ).associateWith { id ->
                RuntimeProfiles.all.first { it.id == id }.launchLine!!
            }
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

    private val _terminalAiExplanation = MutableStateFlow<String?>(null)
    val terminalAiExplanation: StateFlow<String?> = _terminalAiExplanation.asStateFlow()

    /** The evidence snapshot the last ask sent, rendered beside every answer. */
    private val _terminalAiEvidence = MutableStateFlow<List<String>>(emptyList())
    val terminalAiEvidence: StateFlow<List<String>> = _terminalAiEvidence.asStateFlow()

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

        // Populates runtimeProfileReports / agentSignInStates for the first time on this launch.
        // Off the main thread for the same reason as the call inside installTermuxBootstrap():
        // each profile probe spawns a real process.
        viewModelScope.launch(Dispatchers.IO) { refreshRuntimeProfiles() }

        // E1: a live session holds the process at foreground priority, so backgrounding Verb stops
        // handing a running command or agent to the low-memory killer. The claim follows the
        // session, not the screen: RUNNING holds it, a finished or failed session releases it.
        viewModelScope.launch {
            terminalRuntime.sessionState.collect { state ->
                val context = getApplication<Application>()
                when (state) {
                    TerminalSessionState.RUNNING, TerminalSessionState.STARTING ->
                        TerminalHoldService.start(context)
                    TerminalSessionState.EXITED, TerminalSessionState.FAILED ->
                        TerminalHoldService.stop(context)
                    else -> Unit
                }
            }
        }
    }

    private fun runtimeReports(): List<RuntimeProfileReport> =
        RuntimeProfiles.all.map(runtimeCapabilityDetector::inspect)

    private fun refreshRuntimeProfiles() {
        _runtimeProfileReports.value = runtimeReports()
        _agentSignInStates.value = readAgentSignInStates()
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
        // musl-built agent CLIs need their interpreter present before any session starts.
        MuslLoaderBootstrap.install(context)
        TermuxBootstrapInstaller.ensureGuestLinkerConfig(context.filesDir)
        TermuxBootstrapInstaller.ensureSecureAptSources(context.filesDir)
        // Same "every launch, not just install" requirement as the two calls above: this used to
        // only run inside TermuxBootstrapInstaller.install(), so it silently never executed once a
        // device already had a bootstrap (the common case after the first launch).
        TermuxBootstrapInstaller.ensureGuestShellStartupCurrent(context.filesDir, context)
        if (TermuxBootstrapInstaller.isInstalled(context)) {
            _terminalBootstrapState.value = TermuxBootstrapInstaller.State.Ready
            TermuxBootstrapInstaller.ensureGuestDns(context)
            terminalRuntime.refreshEnvironment()
            // Profile refresh happens once, in init{}, off the main thread -- not here, so a normal
            // (already-installed) launch doesn't spawn a probe process per profile twice.
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

        // Populates runtimeProfileReports / agentSignInStates for the first time on this launch.
        // Off the main thread for the same reason as the call inside installTermuxBootstrap():
        // each profile probe spawns a real process.
        viewModelScope.launch(Dispatchers.IO) { refreshRuntimeProfiles() }
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
            .onSuccess { openTerminal() }
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
        val command = ProfileInstallProtocol.command(profile.installCommand, marker)
        withContext(Dispatchers.Main.immediate) {
            terminalRuntime.sendCommand(command)
        }
        val completed: Int? = withTimeoutOrNull(PROFILE_INSTALL_TIMEOUT_MS) {
            var result: Int? = null
            while (result == null) {
                val output = terminalRuntime.terminalOutput.value
                ProfileInstallProtocol.exitCode(output, marker)?.let { exitCode ->
                    result = exitCode
                    continue
                }
                if (runtimeCapabilityDetector.inspect(profile).isReady) {
                    result = 0
                    continue
                }
                delay(500)
            }
            result
        }
        return when {
            completed == null -> "${profile.displayName} timed out; inspect the terminal output."
            completed == 0 -> null
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

    /**
     * Which agent keys are present. Presence only -- the value is read to test emptiness and then
     * discarded, never stored in state, never rendered, never logged.
     */
    private val _agentKeyStatus = MutableStateFlow(readAgentKeyStatus())
    val agentKeyStatus: StateFlow<List<AgentKeyStatus>> = _agentKeyStatus.asStateFlow()


    private fun readAgentKeyStatus(): List<AgentKeyStatus> {
        val envFile = File(File(getApplication<Application>().filesDir, "home"), ".env")
        val declared = runCatching {
            if (!envFile.isFile) emptyMap() else envFile.readLines()
                .map { it.trim() }
                .filterNot { it.startsWith("#") }
                .mapNotNull { line ->
                    val body = line.removePrefix("export ").trim()
                    val separator = body.indexOf('=')
                    if (separator <= 0) null else body.substring(0, separator).trim() to
                        body.substring(separator + 1).trim().trim('"', '\'')
                }
                .toMap()
        }.getOrDefault(emptyMap())

        return AGENT_KEY_VARIABLES.map { name ->
            AgentKeyStatus(variable = name, isSet = declared[name]?.isNotEmpty() == true)
        }
    }

    /** Re-reads key presence, e.g. after the user has edited the file in the terminal. */
    fun refreshAgentKeyStatus() {
        _agentKeyStatus.value = readAgentKeyStatus()
    }

    /**
     * Starts an agent by typing its command into the real terminal and running it there, rather
     * than launching it out of sight. The user sees the command, can interrupt it, and can run it
     * again by hand next time.
     */
    fun launchAgent(command: String) {
        openTerminal()
        val tracked = TRACKED_AGENT_LAUNCH_COMMANDS.entries.firstOrNull { it.value == command }?.key
        val coordinator = tracked?.let(sessionCoordinators::get)
        if (coordinator == null) {
            sendTerminalCommand(command)
            return
        }
        // Captured before sendTerminalCommand so the coordinator's watch can tell which new
        // commandHistory record is this agent's, not anything already running.
        val idsBeforeLaunch = terminalRuntime.commandHistory.value.mapTo(mutableSetOf()) { it.id }
        sendTerminalCommand(command)
        coordinator.onLaunched(projectRepository.selected(), idsBeforeLaunch)
    }

    /** The Agents screen's Resume action once that agent's session is [com.example.verb.session.VerbSessionState.RECOVERABLE]. */
    fun resumeAgentSession(profileId: RuntimeProfileId) {
        val coordinator = sessionCoordinators[profileId] ?: return
        openTerminal()
        viewModelScope.launch(Dispatchers.Main.immediate) {
            coordinator.resume()
        }
    }

    /** The Agents screen's "Start new" action once that agent's session is [com.example.verb.session.VerbSessionState.ENDED]. */
    fun startNewAgentSession(profileId: RuntimeProfileId) {
        TRACKED_AGENT_LAUNCH_COMMANDS[profileId]?.let(::launchAgent)
    }

    /**
     * The newest world archive `verb export` has written, and what happened to it last.
     *
     * Verb never creates one of these on its own: an archive holds the agents' logins, so it exists
     * only because a person ran the command. What the app does is move it somewhere an uninstall
     * cannot reach.
     */
    private val _worldArchiveName = MutableStateFlow(
        com.example.verb.session.WorldArchive.newestArchive(application.filesDir)?.name
    )
    val worldArchiveName: StateFlow<String?> = _worldArchiveName.asStateFlow()

    private val _worldArchiveMessage = MutableStateFlow<String?>(null)
    val worldArchiveMessage: StateFlow<String?> = _worldArchiveMessage.asStateFlow()

    private val _continuityMessage = MutableStateFlow<String?>(null)
    val continuityMessage: StateFlow<String?> = _continuityMessage.asStateFlow()

    private val _continuityPreviewReady = MutableStateFlow(false)
    val continuityPreviewReady: StateFlow<Boolean> = _continuityPreviewReady.asStateFlow()

    private val _importedContinuitySessions = MutableStateFlow(
        com.example.verb.session.ContinuityArchive.importedSessionCount(application.filesDir)
    )
    val importedContinuitySessions: StateFlow<Int> = _importedContinuitySessions.asStateFlow()

    fun refreshWorldArchive() {
        _worldArchiveName.value =
            com.example.verb.session.WorldArchive.newestArchive(getApplication<Application>().filesDir)?.name
    }

    fun saveWorldToDownloads() {
        val context = getApplication<Application>()
        val archive = com.example.verb.session.WorldArchive.newestArchive(context.filesDir)
        if (archive == null) {
            _worldArchiveMessage.value = "No archive yet. Run verb export in the terminal first."
            return
        }
        _worldArchiveMessage.value = when (
            val outcome = com.example.verb.session.WorldArchive.saveToDownloads(context, archive)
        ) {
            is com.example.verb.session.WorldArchive.Outcome.Saved ->
                "Saved to ${outcome.displayName}. It will survive an uninstall; keep it somewhere safe."
            is com.example.verb.session.WorldArchive.Outcome.Failed -> outcome.reason
            com.example.verb.session.WorldArchive.Outcome.NothingToSave ->
                "No archive yet. Run verb export in the terminal first."
        }
    }

    fun stageWorldArchive(uri: android.net.Uri) {
        val context = getApplication<Application>()
        _worldArchiveMessage.value = when (
            val outcome = com.example.verb.session.WorldArchive.stageForImport(context, uri, context.filesDir)
        ) {
            is com.example.verb.session.WorldArchive.Outcome.Saved ->
                "Copied in as ~/${outcome.displayName}. In the terminal, run: verb import ~/${outcome.displayName}"
            is com.example.verb.session.WorldArchive.Outcome.Failed -> outcome.reason
            com.example.verb.session.WorldArchive.Outcome.NothingToSave -> "Nothing was copied."
        }
        refreshWorldArchive()
    }

    fun exportContinuity() {
        val context = getApplication<Application>()
        val project = _selectedProject.value
        if (project == null) {
            _continuityMessage.value = "Select a project before exporting continuity evidence."
            return
        }
        _continuityMessage.value = when (
            val outcome = com.example.verb.session.ContinuityArchive.exportToDownloads(
                context,
                project,
                sessionCoordinators.values.mapNotNull { it.session.value }
            )
        ) {
            is com.example.verb.session.ContinuityArchive.Outcome.Saved ->
                "Saved ${outcome.summary.sessions} session records to ${outcome.displayName}. " +
                    "No transcript, command text, terminal stream, credential, or absolute path was included."
            is com.example.verb.session.ContinuityArchive.Outcome.Failed -> outcome.reason
            else -> "Continuity export did not complete."
        }
    }

    fun previewContinuity(uri: android.net.Uri) {
        val context = getApplication<Application>()
        _continuityMessage.value = when (
            val outcome = com.example.verb.session.ContinuityArchive.previewImport(context, uri)
        ) {
            is com.example.verb.session.ContinuityArchive.Outcome.Previewed -> {
                _continuityPreviewReady.value = true
                "Preview: ${outcome.summary.display()}. Recorded state is history only; nothing local changed."
            }
            is com.example.verb.session.ContinuityArchive.Outcome.Failed -> {
                _continuityPreviewReady.value = false
                outcome.reason
            }
            else -> "Continuity preview did not complete."
        }
    }

    fun applyContinuityPreview() {
        val context = getApplication<Application>()
        _continuityMessage.value = when (
            val outcome = com.example.verb.session.ContinuityArchive.applyPreview(context)
        ) {
            is com.example.verb.session.ContinuityArchive.Outcome.Imported -> {
                _continuityPreviewReady.value = false
                _importedContinuitySessions.value =
                    com.example.verb.session.ContinuityArchive.importedSessionCount(context.filesDir)
                if (outcome.replay) {
                    "This exact evidence was already imported; nothing changed."
                } else {
                    "Imported ${outcome.summary.sessions} session records as read-only evidence. " +
                        "No local session or Resume action changed."
                }
            }
            is com.example.verb.session.ContinuityArchive.Outcome.Failed -> outcome.reason
            else -> "Continuity import did not complete."
        }
    }

    /** Opens the key file in the terminal's editor; Verb never displays or edits key values itself. */
    fun editAgentKeys() {
        openTerminal()
        sendTerminalCommand("nano ~/.env")
    }

    /** Opens the searchable Verb sheet. Always the user's move; nothing opens it on Verb's behalf. */
    fun openVerbSheet() {
        _surface.value = VerbSurface.Sheet
    }

    /**
     * Opens one named task.
     *
     * [fromSheet] records how the user got here so back can retrace it, and nothing else depends on
     * it -- it is navigation history, not product state.
     */
    fun openTask(task: VerbTask, fromSheet: Boolean = false) {
        // Opening anything that displays session recovery re-checks it first: the evidence an agent
        // leaves behind can appear after the coordinator's own bounded retries gave up, and this is
        // the moment the user is about to read that state off a card.
        if (task == VerbTask.AGENTS || task == VerbTask.SESSIONS) {
            sessionCoordinators.values.forEach { it.refresh() }
        }
        _taskOpenedFromSheet.value = fromSheet
        _surface.value = VerbSurface.Task(task)
    }

    /**
     * Dismisses the topmost Verb surface, innermost first: a task, then the sheet, then nothing.
     *
     * Returns false only when the terminal already owns the screen, so the caller can let the system
     * back gesture exit the app.
     */
    fun dismissVerbSurface(): Boolean {
        val next = _surface.value.afterBack(_taskOpenedFromSheet.value) ?: return false
        _surface.value = next
        _taskOpenedFromSheet.value = false
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

    fun submitAssistantPrompt(prompt: String) {
        if (prompt.isBlank() || _assistantState.value is AiAssistantState.Generating) return
        _assistantInput.value = prompt
        _assistantState.value = AiAssistantState.Generating
        viewModelScope.launch(Dispatchers.IO) {
            _assistantState.value = try {
                val response = aiAssistantService.respond(AiAssistantRequest(prompt))
                AiAssistantState.Answer(response)
            } catch (exception: Exception) {
                AiAssistantState.Failure(exception.message ?: "The assistant could not complete this request.")
            }
        }
    }

    fun submitIntent(intent: com.example.verb.model.VerbIntent) {
        _isExecuting.value = true
        _queryInput.value = intent.summary
        // The result appears in Ask Verb, so the surface that will show it is opened before the work
        // starts. `terminal.open` is the exception: its whole point is to leave Verb's surfaces.
        if (intent.id != "terminal.open") {
            ensureAskVerbVisible()
        }
        viewModelScope.launch(Dispatchers.IO) {
            try {
                if (intent.id == "terminal.open") {
                    openTerminal()
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
                    openTerminal()
                    return@launch
                }
                // A query can arrive from the semantic lens over the terminal, not only from Ask
                // Verb's own input. Under the tab model the result landed in a tab the user was not
                // looking at, so a suggested action appeared to do nothing at all. The surface that
                // renders the result is opened here, after resolution, so `terminal.open` never
                // flashes a surface on its way out.
                ensureAskVerbVisible()
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

    /**
     * Brings Ask Verb forward if it is not already there.
     *
     * Deliberately not a plain [openTask]: re-opening the surface the user is already on would reset
     * how they got there, so a person who reached Ask Verb through the sheet and then ran something
     * would find back sending them to the terminal instead of the list they came from.
     */
    private fun ensureAskVerbVisible() {
        if (_surface.value != VerbSurface.Task(VerbTask.ASK_VERB)) {
            openTask(VerbTask.ASK_VERB)
        }
    }

    /** Puts the terminal back in front. It was never destroyed; only covered. */
    fun openTerminal() {
        _taskOpenedFromSheet.value = false
        _surface.value = VerbSurface.None
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

    /** Sends command text to the live PTY. It remains volatile and is never persisted by Verb. */
    fun sendTerminalCommand(cmd: String) {
        terminalRuntime.sendCommand(cmd)
    }

    /** Asks the configured provider to explain structural facts; PTY output never leaves Verb. */
    fun explainTerminalOutput() {
        if (_isTerminalAiExplaining.value) return

        _isTerminalAiExplaining.value = true
        viewModelScope.launch(Dispatchers.IO) {
            _terminalAiExplanation.value = try {
                TerminalAiHelper.analyze(
                    service = aiAssistantService,
                    lastCommand = terminalRuntime.commandHistory.value.lastOrNull(),
                    workingDirectoryKnown = terminalRuntime.currentWorkingDirectory.value != null,
                    sessionState = terminalRuntime.sessionState.value
                )
            } catch (e: Exception) {
                e.message ?: "AI analysis failed."
            }
            _isTerminalAiExplaining.value = false
        }
    }

    /**
     * The M2 slice: the user's own question about this terminal moment, answered from exactly the
     * evidence Verb holds. The evidence snapshot is taken once, before the request, so what the UI
     * displays as "what the model saw" is what was actually sent even if the session moves on while
     * the request is in flight.
     */
    fun askTerminalAi(question: String) {
        if (question.isBlank() || _isTerminalAiExplaining.value) return

        val lastCommand = terminalRuntime.commandHistory.value.lastOrNull()
        val workingDirectoryKnown = terminalRuntime.currentWorkingDirectory.value != null
        val sessionState = terminalRuntime.sessionState.value
        _terminalAiEvidence.value =
            TerminalAiHelper.evidenceLines(lastCommand, workingDirectoryKnown, sessionState)

        _isTerminalAiExplaining.value = true
        viewModelScope.launch(Dispatchers.IO) {
            _terminalAiExplanation.value = try {
                TerminalAiHelper.ask(
                    service = aiAssistantService,
                    question = question,
                    lastCommand = lastCommand,
                    workingDirectoryKnown = workingDirectoryKnown,
                    sessionState = sessionState
                )
            } catch (e: Exception) {
                e.message ?: "AI analysis failed."
            }
            _isTerminalAiExplaining.value = false
        }
    }

    fun dismissTerminalAiExplanation() {
        _terminalAiExplanation.value = null
        _terminalAiEvidence.value = emptyList()
    }

    private fun handleActionResult(result: ActionResult, query: String? = null) {
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

    // No onCleared() override: terminalRuntime is owned by VerbTerminalSessionHolder now, not by
    // this ViewModel, so its screen going away is not a reason to end the session. Destroying it
    // here was docs/DURABLE_SESSION.md's "Activity finish" row -- diagnosed as self-inflicted, not
    // something Android required.
}
