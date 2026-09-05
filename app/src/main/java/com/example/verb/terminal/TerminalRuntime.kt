package com.example.verb.terminal

import androidx.compose.ui.text.TextRange
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.io.File

/**
 * Universal Terminal Runtime implementing [TerminalRuntimeAdapter].
 * Automatically selects [TermuxTerminalRuntimeAdapter] in native Android environments or [FakeTerminalRuntimeAdapter] in test/JVM environments.
 */
class TerminalRuntime(
    private val workingDir: File,
    private val useFakeForTesting: Boolean = false,
    private val bundledBinDir: File? = null,
    initialProjectDirectory: File? = null
) : VerbTerminal {

    /**
     * Everything that decides how a session is launched. Kept as one value so "what the live
     * session is running" and "what the next session would run" can be compared as a whole, rather
     * than as four fields that can drift apart.
     */
    private data class LaunchSpec(
        val environment: TerminalEnvironment,
        val projectDirectory: File?,
        val agentRuntime: AgentRuntimeInstaller.InstalledRuntime?
    )

    private var projectDirectory: File? = initialProjectDirectory
    private var activeAgentRuntime: AgentRuntimeInstaller.InstalledRuntime? = null
    private var activeGuestCommand: List<String>? = null

    /** What the **live** session was actually launched with. Never speculative. */
    private var applied: LaunchSpec = resolveSpec()

    /**
     * A resolved launch configuration that differs from [applied] and has not been applied, because
     * applying it would destroy a healthy session.
     */
    private var pending: LaunchSpec? = null

    private val _pendingEnvironmentChange = MutableStateFlow(false)

    /**
     * True when the environment has changed underneath a running session -- a different project was
     * selected, a bootstrap finished, an Agent Runtime was switched -- and the change cannot take
     * effect without a new shell.
     *
     * Surfaced rather than acted on. Verb used to destroy and restart the PTY for any of these,
     * which meant selecting a project silently killed whatever was running in the terminal, agent
     * sessions included. Changing metadata is not a reason to end someone's work; the user is told
     * the next session will differ and decides when that happens.
     */
    override val pendingEnvironmentChange: StateFlow<Boolean> = _pendingEnvironmentChange.asStateFlow()

    /**
     * The environment the live session is running under.
     *
     * Deliberately the applied one, not the most recently resolved one: callers use this to describe
     * and map the session that exists, and answering with a configuration that is merely queued
     * would make diagnostics and guest-path translation describe a session that is not running.
     */
    override val environment: TerminalEnvironment get() = applied.environment

    private fun resolveSpec(): LaunchSpec {
        val runtime = activeAgentRuntime
        val resolved = if (runtime != null) {
            val cmd = activeGuestCommand ?: listOf("/bin/bash")
            QemuAgentRuntimeEnvironment(workingDir, requireProjectDirectory(), runtime.manifest)
                .resolveGuestCommand(runtime.rootfs, cmd)
        } else {
            TerminalEnvironmentResolver(
                workingDir,
                bundledBinDir = bundledBinDir,
                projectDirectory = projectDirectory
            ).resolve()
        }
        return LaunchSpec(resolved, projectDirectory, runtime)
    }

    private val delegate: TerminalRuntimeAdapter = if (useFakeForTesting) {
        FakeTerminalRuntimeAdapter(workingDir)
    } else {
        TermuxTerminalRuntimeAdapter(
            workingDir = applied.environment.workingDirectory,
            shellExecutable = applied.environment.shellExecutable,
            arguments = applied.environment.arguments,
            sessionEnvironment = applied.environment.variables,
            guestPathMapper = guestPathMapper(applied)
        )
    }

    /**
     * The guest->host binds valid for [spec]. Built from a spec rather than from current mutable
     * state so it always describes the same launch the session was started with: the Agent Runtime
     * binds the selected project at `/workspace`, the Verb CLI userland binds this app's files
     * directory at [VerbGuestPaths.FILES], and Android's system shell establishes no guest
     * filesystem at all.
     */
    private fun guestPathMapper(spec: LaunchSpec): GuestPathMapper = when (spec.environment.kind) {
        TerminalEnvironment.Kind.VERB_AGENT_LINUX_USERLAND ->
            spec.projectDirectory?.let(GuestPathMapper::agentRuntime) ?: GuestPathMapper.NONE
        TerminalEnvironment.Kind.VERB_LOCAL_USERLAND -> GuestPathMapper.verbUserland(workingDir)
        TerminalEnvironment.Kind.ANDROID_SYSTEM_SHELL -> GuestPathMapper.NONE
    }

    /**
     * The concrete Termux adapter backing this runtime, or null when a fake is in use. Lets the UI
     * bind the authentic [com.termux.view.TerminalView] instead of the transcript-based fallback.
     */
    val termuxDelegate: TermuxTerminalRuntimeAdapter?
        get() = delegate as? TermuxTerminalRuntimeAdapter

    /**
     * Whatever the delegate says: the real adapter is its own canvas, the fake has none. Forwarded
     * rather than derived from [termuxDelegate] so there is one place that decides, and it is the
     * object that actually owns the view.
     */
    override val renderTarget: StateFlow<TermuxTerminalRuntimeAdapter?> get() = delegate.renderTarget

    /**
     * Re-resolves the launch configuration **without touching a healthy session**.
     *
     * This used to be a destroy-and-restart, and it was the only thing this function did, which made
     * "change the launch directory" and "kill the terminal" the same operation. It is called from
     * project selection, from Agent Runtime switching, and from the ViewModel's own constructor via
     * the bootstrap check -- so every launch tore down the PTY during startup, and every project
     * switch ended whatever was running.
     *
     * Now it resolves, compares, and decides:
     *
     * - identical to what is running: nothing happens at all, which is the common startup case;
     * - different, with no live session: applied immediately, since there is nothing to preserve;
     * - different, with a live session: recorded as [pendingEnvironmentChange] and left alone.
     */
    override fun refreshEnvironment() {
        val resolved = resolveSpec()
        if (resolved.environment == applied.environment && resolved.projectDirectory == applied.projectDirectory) {
            pending = null
            _pendingEnvironmentChange.value = false
            return
        }
        if (!isSessionActive.value) {
            apply(resolved)
            return
        }
        pending = resolved
        _pendingEnvironmentChange.value = true
    }

    /** Starts a session under [spec], replacing any current one. The only teardown path left here. */
    private fun apply(spec: LaunchSpec) {
        applied = spec
        pending = null
        _pendingEnvironmentChange.value = false
        if (useFakeForTesting) return
        (delegate as? TermuxTerminalRuntimeAdapter)?.reconfigure(
            shellExecutable = spec.environment.shellExecutable,
            arguments = spec.environment.arguments,
            workingDirectory = spec.environment.workingDirectory,
            sessionEnvironment = spec.environment.variables,
            guestPathMapper = guestPathMapper(spec)
        )
    }

    /**
     * Names this terminal in the process-wide diagnostics log.
     *
     * Only the session holder knows the id, and it only knows it after the factory that built this
     * runtime has returned, so the label arrives afterwards rather than through the constructor.
     */
    fun setDiagnosticsLabel(label: String) {
        (delegate as? TermuxTerminalRuntimeAdapter)?.diagnosticsLabel = label
    }

    /**
     * Selection changes define the **next** launch directory. A running shell is already somewhere,
     * and Verb neither moves it nor pretends it moved: the live working directory keeps reporting
     * whatever the shell's actual cwd is.
     */
    override fun selectProject(directory: File?) {
        projectDirectory = directory
        refreshEnvironment()
    }

    /** Switches the **next** session to the separately installed Linux agent rootfs. */
    override fun activateAgentRuntime(
        runtime: AgentRuntimeInstaller.InstalledRuntime,
        guestCommand: List<String>?
    ) {
        activeAgentRuntime = runtime
        activeGuestCommand = guestCommand
        refreshEnvironment()
    }

    /** Returns the next session to the normal Verb CLI userland. */
    override fun deactivateAgentRuntime() {
        activeAgentRuntime = null
        activeGuestCommand = null
        refreshEnvironment()
    }

    private fun requireProjectDirectory(): File =
        projectDirectory?.takeIf { it.isDirectory }
            ?: error("Select a Verb project before opening the Agent Runtime.")

    override val sessionState: StateFlow<TerminalSessionState> get() = delegate.sessionState
    override val terminalOutput: StateFlow<String> get() = delegate.terminalOutput
    override val activeSelectionText: StateFlow<String> get() = delegate.activeSelectionText
    override val activeSelectionRange: StateFlow<TextRange> get() = delegate.activeSelectionRange
    override val isSessionActive: StateFlow<Boolean> get() = delegate.isSessionActive
    override val terminalContextState: StateFlow<TerminalContextState> get() = delegate.terminalContextState
    override val commandHistory: StateFlow<List<CommandExecutionRecord>> get() = delegate.commandHistory
    override val shellIntegrationActive: StateFlow<Boolean> get() = delegate.shellIntegrationActive
    override val urlToOpen: StateFlow<String?> get() = delegate.urlToOpen
    override fun consumeUrlToOpen() = delegate.consumeUrlToOpen()
    override val clipboardCopyEvent: StateFlow<String?> get() = delegate.clipboardCopyEvent
    override fun consumeClipboardCopyEvent() = delegate.consumeClipboardCopyEvent()

    override fun startSession() = delegate.startSession()
    override fun attachSession() = delegate.attachSession()
    override fun sendText(text: String) = delegate.sendText(text)
    override fun sendCommand(cmd: String) = delegate.sendCommand(cmd)
    override fun sendControlKey(key: String) = delegate.sendControlKey(key)
    override fun resize(rows: Int, cols: Int) = delegate.resize(rows, cols)
    override fun selectedText(): String = delegate.selectedText()
    override fun notifySelectionChanged(selectedRange: TextRange, selectedText: String) =
        delegate.notifySelectionChanged(selectedRange, selectedText)

    override fun addSelectionChangeListener(listener: SelectionChangeListener) =
        delegate.addSelectionChangeListener(listener)

    override fun removeSelectionChangeListener(listener: SelectionChangeListener) =
        delegate.removeSelectionChangeListener(listener)

    override val launchWorkingDirectory: File get() = delegate.launchWorkingDirectory
    override val currentWorkingDirectory: StateFlow<TerminalWorkingDirectory?>
        get() = delegate.currentWorkingDirectory

    override fun clearBuffer() = delegate.clearBuffer()
    /**
     * The user's explicit "start a new shell". This is also the one moment a
     * [pendingEnvironmentChange] takes effect: the restart they asked for is exactly the new shell
     * the change needed, so it is applied here rather than forced on them earlier.
     */
    override fun restartSession() {
        val target = pending
        if (target != null) apply(target) else delegate.restartSession()
    }
    override fun destroy() = delegate.destroy()
}
