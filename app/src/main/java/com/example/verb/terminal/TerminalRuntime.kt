package com.example.verb.terminal

import androidx.compose.ui.text.TextRange
import kotlinx.coroutines.flow.StateFlow
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
) : TerminalRuntimeAdapter {

    private var projectDirectory: File? = initialProjectDirectory
    private var activeAgentRuntime: AgentRuntimeInstaller.InstalledRuntime? = null

    var environment: TerminalEnvironment =
        TerminalEnvironmentResolver(workingDir, bundledBinDir = bundledBinDir, projectDirectory = projectDirectory).resolve()

    private val delegate: TerminalRuntimeAdapter = if (useFakeForTesting) {
        FakeTerminalRuntimeAdapter(workingDir)
    } else {
        TermuxTerminalRuntimeAdapter(
            workingDir = environment.workingDirectory,
            shellExecutable = environment.shellExecutable,
            arguments = environment.arguments,
            sessionEnvironment = environment.variables,
            guestPathMapper = guestPathMapper()
        )
    }

    /**
     * The guest->host binds valid for the environment currently resolved. Rebuilt alongside the
     * environment in [refreshEnvironment] so the two can never describe different runtimes: the
     * Agent Runtime binds the selected project at `/workspace`, while the Verb CLI userland binds
     * this app's files directory at [VerbGuestPaths.FILES]. Android's system shell establishes no
     * guest filesystem at all, so nothing is mappable there.
     */
    private fun guestPathMapper(): GuestPathMapper = when (environment.kind) {
        TerminalEnvironment.Kind.VERB_AGENT_LINUX_USERLAND ->
            projectDirectory?.let(GuestPathMapper::agentRuntime) ?: GuestPathMapper.NONE
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
     * Re-resolves the environment and reconfigures the live PTY session. Called once the Termux
     * bootstrap finishes installing so the terminal switches from the Android system shell to the
     * proot-backed Termux userland without recreating this runtime (and losing the bound view).
     */
    fun refreshEnvironment() {
        environment = activeAgentRuntime?.let { runtime ->
            QemuAgentRuntimeEnvironment(workingDir, requireProjectDirectory(), runtime.manifest).resolve(runtime.rootfs)
        } ?: TerminalEnvironmentResolver(workingDir, bundledBinDir = bundledBinDir, projectDirectory = projectDirectory).resolve()
        if (useFakeForTesting) return
        (delegate as? TermuxTerminalRuntimeAdapter)?.reconfigure(
            shellExecutable = environment.shellExecutable,
            arguments = environment.arguments,
            workingDirectory = environment.workingDirectory,
            sessionEnvironment = environment.variables,
            guestPathMapper = guestPathMapper()
        )
    }

    /** Selection changes define the next launch directory; later shell `cd` commands are not tracked. */
    fun selectProject(directory: File?) {
        projectDirectory = directory
        refreshEnvironment()
    }

    /** Switches the existing PTY to the separately installed Linux agent rootfs. */
    fun activateAgentRuntime(runtime: AgentRuntimeInstaller.InstalledRuntime) {
        activeAgentRuntime = runtime
        refreshEnvironment()
    }

    /** Returns the PTY to the normal Verb CLI userland. */
    fun deactivateAgentRuntime() {
        activeAgentRuntime = null
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
    override fun restartSession() = delegate.restartSession()
    override fun destroy() = delegate.destroy()
}
