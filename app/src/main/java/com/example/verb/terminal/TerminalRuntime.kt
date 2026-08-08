package com.example.verb.terminal

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.io.File
import java.io.InputStream
import java.io.OutputStream

class TerminalRuntime(private val workingDir: File) {

    private var process: Process? = null
    private var outputStream: OutputStream? = null

    private val _terminalOutput = MutableStateFlow<String>("")
    val terminalOutput: StateFlow<String> = _terminalOutput.asStateFlow()

    private val _isSessionActive = MutableStateFlow<Boolean>(false)
    val isSessionActive: StateFlow<Boolean> = _isSessionActive.asStateFlow()

    private val scope = CoroutineScope(Dispatchers.IO)

    init {
        startShellSession()
    }

    fun startShellSession() {
        if (_isSessionActive.value) return

        try {
            val pb = ProcessBuilder("/system/bin/sh")
                .directory(workingDir)
                .redirectErrorStream(true)

            val p = pb.start()
            process = p
            outputStream = p.outputStream
            _isSessionActive.value = true

            appendOutput("Verb Terminal V0.1 initialized.\n$ ")

            // Read output stream continuously
            scope.launch {
                val buffer = ByteArray(2048)
                val inputStream: InputStream = p.inputStream
                try {
                    var bytesRead: Int
                    while (inputStream.read(buffer).also { bytesRead = it } != -1) {
                        val text = String(buffer, 0, bytesRead)
                        appendOutput(text)
                    }
                } catch (e: Exception) {
                    appendOutput("\n[Session ended: ${e.message}]\n")
                } finally {
                    _isSessionActive.value = false
                }
            }
        } catch (e: Exception) {
            _isSessionActive.value = false
            appendOutput("Error starting terminal process: ${e.localizedMessage}\n$ ")
        }
    }

    fun sendInput(input: String) {
        val os = outputStream ?: return
        scope.launch {
            try {
                os.write(input.toByteArray())
                os.flush()
            } catch (e: Exception) {
                appendOutput("\n[Write error: ${e.message}]\n")
            }
        }
    }

    fun sendCommand(cmd: String) {
        sendInput("$cmd\n")
    }

    fun sendControlKey(key: String) {
        when (key) {
            "ESC" -> sendInput("\u001b")
            "CTRL_C" -> sendInput("\u0003")
            "TAB" -> sendInput("\t")
            "UP" -> sendInput("\u001b[A")
            "DOWN" -> sendInput("\u001b[B")
            "RIGHT" -> sendInput("\u001b[C")
            "LEFT" -> sendInput("\u001b[D")
            else -> sendInput(key)
        }
    }

    fun clearBuffer() {
        _terminalOutput.value = "$ "
    }

    private fun appendOutput(text: String) {
        val current = _terminalOutput.value
        val updated = if (current.length > 50_000) {
            current.takeLast(25_000) + text
        } else {
            current + text
        }
        _terminalOutput.value = updated
    }

    fun destroy() {
        runCatching {
            process?.destroy()
        }
        _isSessionActive.value = false
    }
}
