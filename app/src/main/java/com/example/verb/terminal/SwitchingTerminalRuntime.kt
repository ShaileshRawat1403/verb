package com.example.verb.terminal

import androidx.compose.ui.text.TextRange
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/**
 * One [VerbTerminal]-shaped view of whichever session is currently in front.
 *
 * A project has sessions now, but almost nothing in the app needs to know that. The workspace, the
 * dock, the runs sheet and the file explorer all ask the same question they always asked -- what is
 * this terminal doing -- and this answers it about the session the person is looking at. Switching
 * sessions is therefore a change of *answer*, not a change of *caller*: the screens re-render
 * because their flows emit, not because anything rewired them.
 *
 * That is the whole reason this exists. The alternative was threading a session identity through
 * ninety-odd call sites so each could look up its own runtime, which would have made every screen
 * responsible for a decision that belongs in exactly one place.
 *
 * Reads follow the active session ([flatMapLatest], so a switch cancels the old collection and
 * starts the new one). Writes go to whoever is in front at the moment of the call — typing into a
 * terminal you are not looking at is not a thing a person can do, so there is no ambiguity to
 * resolve. When there is no session at all, reads report the quiet defaults and writes are dropped
 * rather than queued: a keystroke aimed at a terminal that does not exist has nowhere to be
 * delivered later.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class SwitchingTerminalRuntime(
    private val scope: CoroutineScope,
    private val active: StateFlow<VerbTerminal?>
) : VerbTerminal {

    private val selectionListenerLock = Any()
    private val selectionListeners = LinkedHashSet<SelectionChangeListener>()
    private var selectionListenerRuntime: VerbTerminal? = null

    init {
        // Selection callbacks are registrations rather than flows, so flatMapLatest cannot move
        // them for us. Keep every caller attached to the terminal in front and detach it from the
        // old one. Without this, switching terminals moved the canvas and all observable state but
        // left Inspect wired to the PTY that was no longer visible.
        scope.launch {
            active.collect { runtime ->
                synchronized(selectionListenerLock) {
                    rebindSelectionListeners(runtime)
                }
            }
        }
    }

    private fun <T> following(initial: T, select: (VerbTerminal) -> StateFlow<T>): StateFlow<T> =
        active
            .flatMapLatest { runtime -> runtime?.let(select) ?: MutableStateFlow(initial) }
            .stateIn(scope, SharingStarted.Eagerly, initial)

    override val sessionState: StateFlow<TerminalSessionState> =
        following(TerminalSessionState.EXITED) { it.sessionState }

    override val terminalOutput: StateFlow<String> = following("") { it.terminalOutput }

    override val activeSelectionText: StateFlow<String> = following("") { it.activeSelectionText }

    override val activeSelectionRange: StateFlow<TextRange> =
        following(TextRange.Zero) { it.activeSelectionRange }

    override val isSessionActive: StateFlow<Boolean> = following(false) { it.isSessionActive }

    override val terminalContextState: StateFlow<TerminalContextState> =
        following(TerminalContextState()) { it.terminalContextState }

    override val commandHistory: StateFlow<List<CommandExecutionRecord>> =
        following(emptyList()) { it.commandHistory }

    override val shellIntegrationActive: StateFlow<Boolean> =
        following(false) { it.shellIntegrationActive }

    override val urlToOpen: StateFlow<String?> = following(null) { it.urlToOpen }

    override val clipboardCopyEvent: StateFlow<String?> = following(null) { it.clipboardCopyEvent }

    override val currentWorkingDirectory: StateFlow<TerminalWorkingDirectory?> =
        following(null) { it.currentWorkingDirectory }

    /**
     * The active session's own canvas, followed like any other read.
     *
     * State is what this facade merges; the view is not. Two terminals are two PTYs and therefore
     * two `TerminalView`s, each with its own scrollback, selection and emulator, and switching has
     * to mount the other one rather than repaint this one. So the workspace asks here and gets a
     * concrete adapter back -- the facade stays the single logical terminal for everyone else, and
     * the renderer still reaches the real thing.
     */
    override val renderTarget: StateFlow<TermuxTerminalRuntimeAdapter?> =
        following(null) { it.renderTarget }

    /**
     * Followed, so the banner describes the terminal in front rather than the project.
     *
     * A pending change belongs to the session it was made against: selecting a project while
     * Terminal 1 runs an agent leaves Terminal 2 with nothing pending, and switching between them
     * must show and hide the offer accordingly.
     */
    override val pendingEnvironmentChange: StateFlow<Boolean> =
        following(false) { it.pendingEnvironmentChange }

    /**
     * Not a flow on the interface, so it is read at call time rather than followed. It changes when
     * the active session changes, which is exactly what a caller asking "where was this launched"
     * about the session in front of them wants.
     */
    override val launchWorkingDirectory: File
        get() = active.value?.launchWorkingDirectory ?: File(".")

    override fun consumeUrlToOpen() { active.value?.consumeUrlToOpen() }

    override fun consumeClipboardCopyEvent() { active.value?.consumeClipboardCopyEvent() }

    override fun startSession() { active.value?.startSession() }

    override fun attachSession() { active.value?.attachSession() }

    override fun sendText(text: String) { active.value?.sendText(text) }

    override fun sendCommand(cmd: String) { active.value?.sendCommand(cmd) }

    override fun sendControlKey(key: String) { active.value?.sendControlKey(key) }

    override fun resize(rows: Int, cols: Int) { active.value?.resize(rows, cols) }

    override fun selectedText(): String = active.value?.selectedText() ?: ""

    override fun notifySelectionChanged(selectedRange: TextRange, selectedText: String) {
        active.value?.notifySelectionChanged(selectedRange, selectedText)
    }

    override fun addSelectionChangeListener(listener: SelectionChangeListener) =
        synchronized(selectionListenerLock) {
            // The collector starts asynchronously. Reconcile with the current value here so a
            // listener registered during first composition cannot miss the active terminal.
            rebindSelectionListeners(active.value)
            if (selectionListeners.add(listener)) {
                selectionListenerRuntime?.addSelectionChangeListener(listener)
            }
        }

    override fun removeSelectionChangeListener(listener: SelectionChangeListener) =
        synchronized(selectionListenerLock) {
            if (selectionListeners.remove(listener)) {
                selectionListenerRuntime?.removeSelectionChangeListener(listener)
            }
        }

    private fun rebindSelectionListeners(next: VerbTerminal?) {
        if (next === selectionListenerRuntime) return
        selectionListenerRuntime?.let { previous ->
            selectionListeners.forEach(previous::removeSelectionChangeListener)
        }
        selectionListenerRuntime = next
        next?.let { current ->
            selectionListeners.forEach(current::addSelectionChangeListener)
        }
    }

    /**
     * Falls back to the resolver's own answer when there is no session, rather than inventing one:
     * the environment is a fact about a launched terminal, and with none launched there is nothing
     * to report.
     */
    override val environment: TerminalEnvironment
        get() = active.value?.environment
            ?: TerminalEnvironmentResolver(File(".")).resolve()

    override fun refreshEnvironment() { active.value?.refreshEnvironment() }

    override fun activateAgentRuntime(
        runtime: AgentRuntimeInstaller.InstalledRuntime,
        guestCommand: List<String>?
    ) {
        active.value?.activateAgentRuntime(runtime, guestCommand)
    }

    override fun deactivateAgentRuntime() { active.value?.deactivateAgentRuntime() }

    override fun selectProject(directory: File?) { active.value?.selectProject(directory) }

    override fun clearBuffer() { active.value?.clearBuffer() }

    override fun restartSession() { active.value?.restartSession() }

    /**
     * Destroys the session in front, not every session. Closing one terminal is not closing the
     * project, and a caller reaching for this through the facade means the one they are looking at.
     */
    override fun destroy() { active.value?.destroy() }
}
