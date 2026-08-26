package com.example.verb.terminal

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.view.KeyEvent
import android.view.MotionEvent
import androidx.compose.ui.text.TextRange
import com.termux.terminal.TerminalSession
import com.termux.terminal.TerminalSessionClient
import com.termux.view.TerminalView
import com.termux.view.TerminalViewClient
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList

/**
 * Production Termux Terminal Adapter implementing [TerminalRuntimeAdapter].
 * Directly manages authentic Termux [TerminalSession] and [TerminalView] instances.
 * NO silent ProcessBuilder fallbacks are performed in production.
 */
class TermuxTerminalRuntimeAdapter(
    var workingDir: File,
    var shellExecutable: String = "/system/bin/sh",
    private var arguments: Array<String> = arrayOf("-l"),
    private var sessionEnvironment: Array<String>? = null,
    private var guestPathMapper: GuestPathMapper = GuestPathMapper.NONE
) : TerminalRuntimeAdapter, TerminalSessionClient, TerminalViewClient {
    private var session: TerminalSession? = null
    var terminalView: TerminalView? = null

    fun bindTerminalView(view: TerminalView) {
        // TerminalView is a plain Android View, so Compose does not make it focusable for us.
        // These flags keep user-initiated focus requests available to the terminal view.
        view.isFocusable = true
        view.isFocusableInTouchMode = true
        terminalView = view
        view.setTerminalViewClient(this)
        // The renderer (and its font metrics) is created lazily by setTextSize(). Without it the
        // first layout NPEs in TerminalView.updateSize(). Width is 0 here so updateSize() no-ops
        // safely; the size is applied once the view is measured.
        if (view.mRenderer == null) {
            minTextSizePx = dpToPx(MIN_TEXT_SIZE_DP, view)
            maxTextSizePx = dpToPx(MAX_TEXT_SIZE_DP, view)
            // Coerce the loaded value into range before it becomes the base, so an accumulated
            // scale of exactly 1 always maps to the size currently on screen.
            baseTextSizePx = loadPersistedTextSizePx(view).coerceIn(minTextSizePx, maxTextSizePx)
            appliedTextSizePx = baseTextSizePx
            view.setTextSize(baseTextSizePx)
        }
        session?.let { view.attachSession(it) }
    }

    /**
     * Font size actually applied to the mounted [TerminalView], in pixels. Vendored
     * `TerminalView.setTextSize(int)` takes pixels despite its javadoc claiming dp
     * (`TerminalRenderer` feeds it straight into `TextPaint.setTextSize`), so everything here is
     * px end to end.
     */
    var appliedTextSizePx: Int? = null
        private set

    private var baseTextSizePx = 0
    private var minTextSizePx = 0
    private var maxTextSizePx = 0

    private fun loadPersistedTextSizePx(view: TerminalView): Int {
        val prefs = view.context.applicationContext.getSharedPreferences(
            TEXT_SIZE_PREFERENCES, Context.MODE_PRIVATE
        )
        val stored = prefs.getInt(TEXT_SIZE_PX_KEY, -1)
        return if (stored > 0) stored else dpToPx(DEFAULT_TEXT_SIZE_DP, view)
    }

    private fun dpToPx(dp: Float, view: TerminalView): Int =
        android.util.TypedValue.applyDimension(
            android.util.TypedValue.COMPLEX_UNIT_DIP, dp, view.context.resources.displayMetrics
        ).toInt()

    private val textSizePersistHandler = android.os.Handler(android.os.Looper.getMainLooper())
    private val textSizePersistRunnable = Runnable {
        val size = appliedTextSizePx ?: return@Runnable
        terminalView?.context?.applicationContext
            ?.getSharedPreferences(TEXT_SIZE_PREFERENCES, Context.MODE_PRIVATE)
            ?.edit()
            ?.putInt(TEXT_SIZE_PX_KEY, size)
            ?.apply()
    }

    private val _sessionState = MutableStateFlow<TerminalSessionState>(TerminalSessionState.STARTING)
    override val sessionState: StateFlow<TerminalSessionState> = _sessionState.asStateFlow()

    private val _terminalOutput = MutableStateFlow<String>("")
    override val terminalOutput: StateFlow<String> = _terminalOutput.asStateFlow()

    private val _isSessionActive = MutableStateFlow<Boolean>(false)
    override val isSessionActive: StateFlow<Boolean> = _isSessionActive.asStateFlow()

    private val _terminalContextState = MutableStateFlow(TerminalContextState())
    override val terminalContextState: StateFlow<TerminalContextState> = _terminalContextState.asStateFlow()

    private val commandTracker = CommandExecutionTracker()
    override val commandHistory: StateFlow<List<CommandExecutionRecord>> = commandTracker.history
    override val shellIntegrationActive: StateFlow<Boolean> = commandTracker.shellIntegrationActive

    override val launchWorkingDirectory: File get() = workingDir

    /**
     * Republished from [CommandExecutionTracker.currentWorkingDirectory] with the guest path mapped
     * to a host path where the active runtime's binds allow it. Kept as its own StateFlow (rather
     * than a mapped Flow) so it stays a synchronously readable snapshot, matching every other state
     * this adapter exposes, and so the mapping runs once per prompt instead of once per collector.
     */
    private val _currentWorkingDirectory = MutableStateFlow<TerminalWorkingDirectory?>(null)
    override val currentWorkingDirectory: StateFlow<TerminalWorkingDirectory?> =
        _currentWorkingDirectory.asStateFlow()

    private fun refreshCurrentWorkingDirectory() {
        val guestPath = commandTracker.currentWorkingDirectory.value
        _currentWorkingDirectory.value = guestPath?.let {
            TerminalWorkingDirectory(guestPath = it, hostPath = guestPathMapper.toHostPath(it))
        }
    }

    private val _urlToOpen = MutableStateFlow<String?>(null)
    override val urlToOpen: StateFlow<String?> = _urlToOpen.asStateFlow()
    override fun consumeUrlToOpen() {
        _urlToOpen.value = null
    }

    private val _clipboardCopyEvent = MutableStateFlow<String?>(null)
    override val clipboardCopyEvent: StateFlow<String?> = _clipboardCopyEvent.asStateFlow()
    override fun consumeClipboardCopyEvent() {
        _clipboardCopyEvent.value = null
    }

    private val _activeSelectionText = MutableStateFlow<String>("")
    override val activeSelectionText: StateFlow<String> = _activeSelectionText.asStateFlow()

    private val _activeSelectionRange = MutableStateFlow<TextRange>(TextRange.Zero)
    /**
     * Note: The activeSelectionRange currently represents a range local to the extracted string
     * rather than exact terminal cell coordinates.
     */
    override val activeSelectionRange: StateFlow<TextRange> = _activeSelectionRange.asStateFlow()

    private val selectionListeners = CopyOnWriteArrayList<SelectionChangeListener>()

    init {
        startSession()
    }

    private var probedExec = false

    private fun probeExec(binPath: String, arg: String): String {
        return try {
            val p = ProcessBuilder(binPath, arg).redirectErrorStream(true).start()
            if (!waitForProbe(p)) {
                p.destroy()
                return "TIMEOUT"
            }
            val out = p.inputStream.readBytes().decodeToString().take(300).trim()
            val code = p.exitValue()
            "OK exit=$code out=[$out]"
        } catch (e: Exception) {
            "FAILED ${e.javaClass.simpleName}: ${e.message}"
        }
    }

    private fun waitForProbe(process: Process): Boolean {
        val deadline = android.os.SystemClock.uptimeMillis() + 2_000L
        while (true) {
            try {
                process.exitValue()
                return true
            } catch (_: IllegalThreadStateException) {
                if (android.os.SystemClock.uptimeMillis() >= deadline) return false
                Thread.sleep(25L)
            }
        }
    }

    private fun probeExecOnce() {
        if (probedExec) return
        probedExec = true
        val sh = probeExec("/system/bin/sh", "-c")
        android.util.Log.i(TAG, "probe /system/bin/sh -c -> $sh")
        val proot = probeExec(shellExecutable, "--version")
        android.util.Log.i(TAG, "probe proot --version -> $proot")
        if (workingDir.parentFile != null) {
            val bash = File(workingDir.parentFile, "usr/bin/bash")
            if (bash.isFile) {
                android.util.Log.i(TAG, "probe bootstrap bash --version -> ${probeExec(bash.absolutePath, "--version")}")
            }
        }
    }

    override fun startSession() {
        if (_isSessionActive.value && session != null) return
        probeExecOnce()

        _sessionState.value = TerminalSessionState.STARTING
        TerminalSessionLogger.info(
            LogCategory.LIFECYCLE,
            "Starting Termux PTY session: shell=$shellExecutable cwd=${workingDir.absolutePath}"
        )
        android.util.Log.i(TAG, "startSession shell=$shellExecutable args=${arguments.joinToString(" ")} cwd=${workingDir.absolutePath}")

        val envArray = sessionEnvironment ?: defaultSystemEnvironment()

        try {
            Class.forName("com.termux.terminal.JNI")
            val newSession = TerminalSession(
                shellExecutable,
                workingDir.absolutePath,
                arguments,
                envArray,
                2000,
                this
            )
            
            // Initialize the emulator immediately for headless execution
            newSession.updateSize(80, 24, 0, 0)
            
            if (newSession.isRunning) {
                session = newSession
                _isSessionActive.value = true
                _sessionState.value = TerminalSessionState.RUNNING
                refreshTerminalContext(newSession)
                terminalView?.attachSession(newSession)
                TerminalSessionLogger.info(LogCategory.LIFECYCLE, "Termux PTY session running (pid ${newSession.pid})")
            } else {
                // Truthful failure reporting in production — NO silent fallback
                _isSessionActive.value = false
                _sessionState.value = TerminalSessionState.FAILED
                appendOutput("\n[FAILED to start Termux PTY session: libtermux.so or PTY allocation failed]\n")
                TerminalSessionLogger.error(
                    LogCategory.JNI,
                    "Termux PTY session failed to allocate (isRunning=false)"
                )
                android.util.Log.e(TAG, "PTY session failed to allocate (isRunning=false), shell=$shellExecutable")
            }
        } catch (t: Throwable) {
            _isSessionActive.value = false
            _sessionState.value = TerminalSessionState.FAILED
            refreshTerminalContext()
            appendOutput("\n[FAILED to start Termux PTY session: ${t.message}]\n")
            TerminalSessionLogger.error(LogCategory.JNI, "Termux PTY session failed: ${t.message}")
            android.util.Log.e(TAG, "PTY session failed: ${t.message}", t)
        }
    }

    private fun defaultSystemEnvironment(): Array<String> {
        val sysPath = System.getenv("PATH") ?: "/system/bin:/system/xbin"
        return arrayOf(
            "TERM=xterm-256color",
            "COLORTERM=truecolor",
            "HOME=${workingDir.absolutePath}",
            "PATH=$sysPath",
            "LANG=en_US.UTF-8",
            "TMPDIR=${workingDir.absolutePath}/.tmp",
            "CURL_CA_BUNDLE=${workingDir.absolutePath}/cacert.pem"
        )
    }

    override fun attachSession() {
        if (session != null && _isSessionActive.value) {
            _sessionState.value = TerminalSessionState.RUNNING
        } else {
            startSession()
        }
    }

    override fun sendText(text: String) {
        // A finished or reclaimed session silently drops writes. Restart it so typed commands
        // actually execute instead of appearing to do nothing (the "terminal won't run" symptom).
        val current = session
        if (current == null || !current.isRunning) {
            android.util.Log.i(TAG, "Session not running; auto-restarting before write")
            restartSession()
        }
        val target = session
        target?.let {
            writeToSession(it, text)
            // The PTY reader updates the emulator asynchronously. Keep the canvas responsive while
            // the IME is resizing the Compose layout; a touch should not be required to repaint it.
            terminalView?.postInvalidateOnAnimation()
        }
    }

    private fun writeToSession(target: TerminalSession, text: String) {
        val bytes = text.toByteArray(Charsets.UTF_8)
        if (bytes.isNotEmpty()) target.write(bytes, 0, bytes.size)
    }

    override fun sendCommand(cmd: String) {
        sendText("$cmd\n")
    }

    override fun sendControlKey(key: String) {
        val s = session ?: return
        val cursorApp = s.emulator?.isCursorKeysApplicationMode ?: false
        
        // Helper to get KeyHandler code
        fun getCode(keyCode: Int, shift: Boolean = false): String? {
            val mod = if (shift) com.termux.terminal.KeyHandler.KEYMOD_SHIFT else 0
            return com.termux.terminal.KeyHandler.getCode(keyCode, mod, cursorApp, false)
        }

        when (key) {
            "ESC" -> getCode(android.view.KeyEvent.KEYCODE_ESCAPE)?.let { writeToSession(s, it) }
            "TAB" -> getCode(android.view.KeyEvent.KEYCODE_TAB)?.let { writeToSession(s, it) }
            "SHIFT_TAB" -> getCode(android.view.KeyEvent.KEYCODE_TAB, true)?.let { writeToSession(s, it) }
            "UP" -> getCode(android.view.KeyEvent.KEYCODE_DPAD_UP)?.let { writeToSession(s, it) }
            "DOWN" -> getCode(android.view.KeyEvent.KEYCODE_DPAD_DOWN)?.let { writeToSession(s, it) }
            "RIGHT" -> getCode(android.view.KeyEvent.KEYCODE_DPAD_RIGHT)?.let { writeToSession(s, it) }
            "LEFT" -> getCode(android.view.KeyEvent.KEYCODE_DPAD_LEFT)?.let { writeToSession(s, it) }
            "HOME" -> getCode(android.view.KeyEvent.KEYCODE_MOVE_HOME)?.let { writeToSession(s, it) }
            "END" -> getCode(android.view.KeyEvent.KEYCODE_MOVE_END)?.let { writeToSession(s, it) }
            "PGUP" -> getCode(android.view.KeyEvent.KEYCODE_PAGE_UP)?.let { writeToSession(s, it) }
            "PGDN" -> getCode(android.view.KeyEvent.KEYCODE_PAGE_DOWN)?.let { writeToSession(s, it) }
            "DEL" -> getCode(android.view.KeyEvent.KEYCODE_FORWARD_DEL)?.let { writeToSession(s, it) }
            "BACKSPACE" -> getCode(android.view.KeyEvent.KEYCODE_DEL)?.let { writeToSession(s, it) }
            "PASTE" -> onPasteTextFromClipboard(s)
            else -> {
                if (key.startsWith("CTRL_") && key.length == 6) {
                    val c = key.last()
                    if (c in 'A'..'Z') {
                        val codePoint = c - 'A' + 1
                        writeToSession(s, codePoint.toChar().toString())
                        return
                    }
                }
                writeToSession(s, key)
            }
        }
    }

    override fun resize(rows: Int, cols: Int) {
        session?.updateSize(cols, rows, 0, 0)
    }

    override fun selectedText(): String {
        return terminalView?.storedSelectedText ?: _activeSelectionText.value
    }

    override fun notifySelectionChanged(selectedRange: TextRange, selectedText: String) {
        // The selection range is documented as local to the extracted string
        _activeSelectionRange.value = selectedRange
        _activeSelectionText.value = selectedText
        for (listener in selectionListeners) {
            listener.onSelectionChanged(selectedRange, selectedText)
        }
    }

    override fun addSelectionChangeListener(listener: SelectionChangeListener) {
        if (!selectionListeners.contains(listener)) {
            selectionListeners.add(listener)
        }
    }

    override fun removeSelectionChangeListener(listener: SelectionChangeListener) {
        selectionListeners.remove(listener)
    }

    override fun clearBuffer() {
        _terminalOutput.value = "$ "
        session?.reset()
    }

    override fun restartSession() {
        destroy()
        startSession()
    }

    /**
     * Swaps the session launch configuration (shell/argv/working directory/environment) and
     * restarts the session against the same terminal view. Used by [TerminalRuntime.refreshEnvironment]
     * once the Termux bootstrap finishes installing so the PTY starts proot instead of the
     * Android system shell.
     */
    fun reconfigure(
        shellExecutable: String,
        arguments: Array<String>,
        workingDirectory: File,
        sessionEnvironment: Array<String>,
        guestPathMapper: GuestPathMapper
    ) {
        android.util.Log.i(TAG, "reconfigure shell=$shellExecutable args=${arguments.joinToString(" ")} cwd=${workingDirectory.absolutePath}")
        // destroy() already cleared the live working directory; the mapper is swapped here because
        // the new session may run against an entirely different set of binds (Verb userland vs the
        // Agent Runtime's /workspace), and a stale mapper would translate the next session's guest
        // paths against the previous session's host roots.
        destroy()
        this.shellExecutable = shellExecutable
        this.arguments = arguments
        this.workingDir = workingDirectory
        this.sessionEnvironment = sessionEnvironment
        this.guestPathMapper = guestPathMapper
        startSession()
    }

    override fun destroy() {
        outputHandler.removeCallbacks(publishSnapshotRunnable)
        pendingSnapshotSession = null
        _sessionState.value = TerminalSessionState.STOPPING
        _isSessionActive.value = false
        commandTracker.onSessionEnded()
        // The shell that owned this directory is gone, so the live cwd goes back to unknown rather
        // than lingering as a stale value across the restart.
        refreshCurrentWorkingDirectory()
        session?.finishIfRunning()
        session = null
        refreshTerminalContext()
        selectionListeners.clear()
        _sessionState.value = TerminalSessionState.EXITED
        TerminalSessionLogger.info(LogCategory.LIFECYCLE, "Termux PTY session destroyed")
    }

    private companion object {
        const val TAG = "VerbTerminal"
        const val OUTPUT_THROTTLE_MS = 120L
        const val METRICS_WINDOW_MS = 5_000L
        const val DEFAULT_TEXT_SIZE_DP = 9f
        const val MIN_TEXT_SIZE_DP = 6f
        const val MAX_TEXT_SIZE_DP = 20f
        const val TEXT_SIZE_PREFERENCES = "verb_terminal_display"
        const val TEXT_SIZE_PX_KEY = "text_size_px"
        const val TEXT_SIZE_PERSIST_DELAY_MS = 500L
    }

    // TerminalSessionClient callbacks
    //
    // Codex/heavy TUI output can invoke this dozens of times per second. Rebuilding the full
    // transcript into a String and publishing it to a StateFlow on every single callback was
    // competing with the live PTY redraw on the UI thread (transcript rebuild cost + Compose
    // recomposition of every terminalOutput consumer, including screens that never render the
    // real TerminalView) and made typing and Codex's streaming replies feel laggy. The real
    // terminal canvas never reads _terminalOutput -- it draws straight from the emulator's screen
    // buffer -- so only the derived snapshot (used for AI-explain and history recording) is
    // throttled here; the visible redraw below still fires on every callback, unchanged.
    override fun onTextChanged(changedSession: TerminalSession) {
        refreshTerminalContext(changedSession)
        terminalView?.postInvalidateOnAnimation()
        scheduleOutputSnapshot(changedSession)
        recordCallbackForMetrics()
    }

    // Rolling counters answering "did the throttle actually change anything" with numbers
    // instead of a feeling. Every PTY callback increments metricsCallbackCount here; every time
    // the throttle in scheduleOutputSnapshot() actually lets a publish through,
    // publishOutputSnapshot() increments metricsPublishCount. Every METRICS_WINDOW_MS a one-line
    // summary goes to
    // TerminalSessionLogger under LogCategory.DIAGNOSTIC, visible in the existing Diagnostics
    // sheet -- no new UI, no rebuild-to-toggle: run a real Codex conversation, open Diagnostics,
    // filter to Diagnostic.
    private var metricsWindowStart = android.os.SystemClock.uptimeMillis()
    private var metricsCallbackCount = 0
    private var metricsPublishCount = 0

    private fun recordCallbackForMetrics() {
        metricsCallbackCount++
        val now = android.os.SystemClock.uptimeMillis()
        val elapsed = now - metricsWindowStart
        if (elapsed >= METRICS_WINDOW_MS) {
            val throttleRatio = if (metricsCallbackCount > 0) {
                100 - (metricsPublishCount * 100 / metricsCallbackCount)
            } else 0
            TerminalSessionLogger.info(
                LogCategory.DIAGNOSTIC,
                "PTY callback rate: $metricsCallbackCount onTextChanged / $metricsPublishCount " +
                    "snapshots published in ${elapsed}ms ($throttleRatio% coalesced by throttle)"
            )
            metricsWindowStart = now
            metricsCallbackCount = 0
            metricsPublishCount = 0
        }
    }

    private val outputHandler = android.os.Handler(android.os.Looper.getMainLooper())
    private var lastOutputPublishAt = 0L
    private var pendingSnapshotSession: TerminalSession? = null
    private val publishSnapshotRunnable = Runnable {
        pendingSnapshotSession?.let { publishOutputSnapshot(it) }
        pendingSnapshotSession = null
    }

    /**
     * Leading+trailing throttle: the first callback in a burst publishes immediately, further
     * callbacks within [OUTPUT_THROTTLE_MS] are coalesced, and a trailing publish is always
     * scheduled so the final chunk of a burst is never dropped once output goes idle.
     */
    private fun scheduleOutputSnapshot(session: TerminalSession) {
        val now = android.os.SystemClock.uptimeMillis()
        val elapsed = now - lastOutputPublishAt
        if (elapsed >= OUTPUT_THROTTLE_MS) {
            outputHandler.removeCallbacks(publishSnapshotRunnable)
            pendingSnapshotSession = null
            publishOutputSnapshot(session)
        } else {
            pendingSnapshotSession = session
            outputHandler.removeCallbacks(publishSnapshotRunnable)
            outputHandler.postDelayed(publishSnapshotRunnable, OUTPUT_THROTTLE_MS - elapsed)
        }
    }

    private fun publishOutputSnapshot(session: TerminalSession) {
        val transcript = session.emulator?.screen?.transcriptText ?: ""
        _terminalOutput.value = if (transcript.length > 50_000) transcript.takeLast(50_000) else transcript
        lastOutputPublishAt = android.os.SystemClock.uptimeMillis()
        metricsPublishCount++
    }

    override fun onTitleChanged(changedSession: TerminalSession) {}

    override fun onSessionFinished(finishedSession: TerminalSession) {
        // A session destroyed during a restart (reconfigure/destroy) reports its own finish
        // asynchronously, AFTER the replacement session is already running. Ignore those stale
        // callbacks or they clobber the live session's state into FAILED/EXITED.
        if (finishedSession !== session) {
            android.util.Log.w(TAG, "Ignoring stale onSessionFinished for a replaced session (pid ${finishedSession.pid})")
            return
        }
        // A natural shell exit does not pass through destroy(). Clear all session-owned advisory
        // state here too, or the finished shell's cwd/handshake would remain visible as live.
        commandTracker.onSessionEnded()
        refreshCurrentWorkingDirectory()
        _isSessionActive.value = false
        _sessionState.value = if (finishedSession.exitStatus == 0) TerminalSessionState.EXITED else TerminalSessionState.FAILED
        refreshTerminalContext()
        appendOutput("\n[Session terminated with code ${finishedSession.exitStatus}]\n$ ")
        TerminalSessionLogger.warn(LogCategory.LIFECYCLE, "Termux PTY session finished with exit code ${finishedSession.exitStatus}")
        android.util.Log.w(TAG, "Session finished exit=${finishedSession.exitStatus} shell=$shellExecutable")
    }

    override fun onCopyTextToClipboard(session: TerminalSession, text: String) {
        terminalView?.context?.let { ctx ->
            val clipboard = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            clipboard.setPrimaryClip(ClipData.newPlainText("Termux Selection", text))
            _clipboardCopyEvent.value = "Copied to clipboard"
        }
    }

    override fun onPasteTextFromClipboard(session: TerminalSession?) {
        val s = session ?: this.session
        terminalView?.context?.let { ctx ->
            val clipboard = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            val clipData = clipboard.primaryClip
            if (clipData != null && clipData.itemCount > 0) {
                val text = clipData.getItemAt(0).coerceToText(ctx).toString()
                s?.emulator?.paste(text)
            }
        }
    }
    
    override fun onBell(session: TerminalSession) {}

    override fun onColorsChanged(session: TerminalSession) {}

    /**
     * Advisory OSC 7/633 shell-integration marker from the vendored emulator (see
     * [com.termux.terminal.TerminalOutput.onShellIntegrationOsc]). [ShellIntegrationParser] never
     * throws, so a malformed or forged marker just fails to parse into an event and is dropped
     * here -- it can never crash this callback or the emulator that invoked it.
     *
     * Diagnostics logged here are metadata only. OSC 633 `E` carries a user's typed command
     * *before* [com.example.verb.semantic.SecretGuard] redaction reaches it (redaction happens
     * inside [CommandExecutionTracker], not here), so `rawArgs`, command text, working directory,
     * and full record contents (its `toString()` included -- it embeds both) must never be
     * logged. Android logcat is a persistent, device-local, often-readable-by-other-apps sink;
     * logging any of that would be a real secret leak, not a hypothetical one.
     */
    override fun onShellIntegrationOsc(session: TerminalSession, oscCode: Int, rawArgs: String) {
        val event = ShellIntegrationParser.parse(oscCode, rawArgs)
        // Shell Awareness P0 has no UI yet; this is the developer-only diagnostics surface for
        // physical-device verification (matches the existing Log.i lifecycle logging in this
        // class). Payload length and the event's type name are safe: neither reveals content.
        android.util.Log.i(
            TAG,
            "shellIntegrationOsc osc=$oscCode payloadLength=${rawArgs.length} eventType=${event?.let { it::class.simpleName } ?: "null"}"
        )
        if (event == null) return
        commandTracker.onEvent(event)
        refreshCurrentWorkingDirectory()
        val last = commandTracker.history.value.lastOrNull()
        android.util.Log.i(
            TAG,
            "commandHistory size=${commandTracker.history.value.size} lastState=${last?.state} lastExitCode=${last?.exitCode}"
        )
    }
    
    override fun onTerminalCursorStateChange(state: Boolean) {}
    
    override fun setTerminalShellPid(session: TerminalSession, pid: Int) {}
    
    override fun getTerminalCursorStyle(): Int = 0

    // TerminalViewClient callbacks

    /**
     * Pinch zoom. `scale` is the accumulated factor since the view was created (the vendored view
     * multiplies it in and feeds us the running total), so the applied size is always
     * `baseTextSizePx * scale` -- consistent across recreations because the base itself is loaded
     * from storage at bind time. The clamped ratio is returned so the view's accumulated factor
     * tracks what is actually displayed rather than drifting past the clamp.
     */
    override fun onScale(scale: Float): Float {
        val base = baseTextSizePx
        if (base <= 0) return scale
        val newPx = (base * scale).toInt().coerceIn(minTextSizePx, maxTextSizePx)
        if (newPx != appliedTextSizePx) {
            appliedTextSizePx = newPx
            terminalView?.setTextSize(newPx)
            // A pinch delivers a callback per gesture frame; debounce the write so one gesture
            // costs one persist instead of sixty.
            textSizePersistHandler.removeCallbacks(textSizePersistRunnable)
            textSizePersistHandler.postDelayed(textSizePersistRunnable, TEXT_SIZE_PERSIST_DELAY_MS)
        }
        return newPx.toFloat() / base
    }

    /**
     * Set by the terminal screen: a confirmed single tap on the canvas should move input focus to
     * the command field and raise the IME. It rides the view's own gesture recognizer because the
     * recognizer is what already separates taps from scrolls, flings, pinches and long-press
     * selection -- a Compose-side watcher cannot make that call, since an interop view that claims
     * the touch stream is never offered the release that would confirm a tap.
     */
    var onCanvasTap: (() -> Unit)? = null

    override fun onSingleTapUp(e: MotionEvent) {
        onCanvasTap?.invoke()
        val view = terminalView ?: return
        val columnAndRow = view.getColumnAndRow(e, true)
        val column = columnAndRow[0]
        val row = columnAndRow[1]
        val emulator = session?.emulator ?: return
        val url = findUrlAtBuffer(emulator, row, column) ?: return
        _urlToOpen.value = url
    }

    private fun findUrlAtBuffer(emulator: com.termux.terminal.TerminalEmulator, row: Int, column: Int): String? {
        val lines = mutableMapOf<Int, String>()

        fun readLine(bufferRow: Int): String? = lines.getOrPut(bufferRow) {
            emulator.getScreen().getSelectedText(0, bufferRow, 100_000, bufferRow).trimEnd()
        }

        // Codex and other CLIs often print a URL longer than the terminal width. Search a small
        // row window with soft-wrapped rows joined, while preserving the tap's offset in that
        // joined text. Normal line breaks remain boundaries in the single-row fast path below.
        runCatching {
            readLine(row)?.let { line ->
                if (line.isNotBlank()) {
                    findUrlAt(line, column)?.let { return it }
                }
            }

            val radius = 8
            for (start in row - radius..row) {
                val joined = StringBuilder()
                var tappedOffset = -1
                for (bufferRow in start..(row + radius)) {
                    val line = readLine(bufferRow) ?: ""
                    if (bufferRow == row) tappedOffset = joined.length + column
                    joined.append(line)
                }
                if (tappedOffset >= 0) {
                    findUrlAt(joined.toString(), tappedOffset)?.let { return it }
                }
            }
        }
        return null
    }
    
    override fun onInspectText(text: String) {
        // Notify Semantic Lens about inspected text. The selection range is local to the extracted string.
        notifySelectionChanged(TextRange(0, text.length), text)
    }

    override fun onLongPress(e: MotionEvent): Boolean {
        return false
    }

    override fun shouldBackButtonBeMappedToEscape(): Boolean = false
    override fun shouldEnforceCharBasedInput(): Boolean = false
    override fun shouldUseCtrlSpaceWorkaround(): Boolean = false
    override fun isTerminalViewSelected(): Boolean = true
    override fun copyModeChanged(copyMode: Boolean) {}
    
    override fun onKeyDown(keyCode: Int, e: KeyEvent, session: TerminalSession?): Boolean = false
    override fun onKeyUp(keyCode: Int, e: KeyEvent): Boolean = false
    override fun readControlKey(): Boolean = false
    override fun readAltKey(): Boolean = false
    override fun readShiftKey(): Boolean = false
    override fun readFnKey(): Boolean = false
    override fun onCodePoint(codePoint: Int, ctrlDown: Boolean, session: TerminalSession?): Boolean = false
    
    override fun onEmulatorSet() {}
    
    override fun logError(tag: String, message: String) {}
    override fun logWarn(tag: String, message: String) {}
    override fun logInfo(tag: String, message: String) {}
    override fun logDebug(tag: String, message: String) {}
    override fun logVerbose(tag: String, message: String) {}
    override fun logStackTraceWithMessage(tag: String, message: String, e: Exception) {}
    override fun logStackTrace(tag: String, e: Exception) {}

    private fun appendOutput(text: String) {
        val current = _terminalOutput.value
        val updated = if (current.length > 50_000) {
            current.takeLast(25_000) + text
        } else {
            current + text
        }
        _terminalOutput.value = updated
    }

    private fun refreshTerminalContext(activeSession: TerminalSession? = session) {
        val emulator = activeSession?.emulator
        _terminalContextState.value = if (_isSessionActive.value && activeSession != null && emulator != null) {
            TerminalContextState(
                capability = TerminalContextCapability.SESSION_ONLY,
                sessionId = activeSession.mHandle,
                alternateScreenState = if (emulator.isAlternateBufferActive) {
                    AlternateScreenState.ACTIVE
                } else {
                    AlternateScreenState.INACTIVE
                }
            )
        } else {
            TerminalContextState()
        }
    }
}
