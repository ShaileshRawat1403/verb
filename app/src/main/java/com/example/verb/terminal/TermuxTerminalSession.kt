package com.example.verb.terminal

import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.InputStream
import java.io.OutputStream
import java.util.UUID

/**
 * Represents an authentic Termux TTY Terminal Session with PTY process management.
 * Connects Termux JNI PTY bindings to session stream listeners and window sizing controls.
 */
class TermuxTerminalSession(
    val workingDir: File,
    val shellExecutable: String = "/system/bin/sh",
    var rows: Int = 24,
    var cols: Int = 80,
    val onOutputReceived: (String) -> Unit,
    val onSessionTerminated: (Int) -> Unit
) {

    val sessionId: String = UUID.randomUUID().toString()
    private var masterFd: Int = -1
    private var pid: Int = -1

    private var inputStream: InputStream? = null
    private var outputStream: OutputStream? = null
    private var fallbackProcess: Process? = null

    private var isRunning: Boolean = false
    private val scope = CoroutineScope(Dispatchers.IO)
    private var readJob: Job? = null

    companion object {
        private const val TAG = "TermuxTerminalSession"
    }

    /**
     * Initializes and starts the Termux PTY session.
     */
    fun startSession() {
        if (isRunning) return

        val env = mutableMapOf<String, String>()
        env["TERM"] = "xterm-256color"
        env["COLORTERM"] = "truecolor"
        env["HOME"] = workingDir.absolutePath
        val sysPath = System.getenv("PATH") ?: "/system/bin:/system/xbin"
        env["PATH"] = "$sysPath:/data/data/com.termux/files/usr/bin"
        env["LANG"] = "en_US.UTF-8"

        val pidOut = IntArray(1)
        val masterFdOut = IntArray(1)

        val targetExecutable = if (File(shellExecutable).exists()) {
            shellExecutable
        } else if (File("/system/bin/sh").exists()) {
            "/system/bin/sh"
        } else if (File("/bin/sh").exists()) {
            "/bin/sh"
        } else if (File("/bin/bash").exists()) {
            "/bin/bash"
        } else {
            shellExecutable
        }

        val success = JniPtyBridge.createSubprocess(
            executable = targetExecutable,
            cwd = workingDir,
            args = arrayOf("-l"),
            env = env,
            processIdOut = pidOut,
            masterFdOut = masterFdOut
        )

        if (success && masterFdOut[0] > 0) {
            masterFd = masterFdOut[0]
            pid = pidOut[0]
            val fdObj = JniPtyBridge.createFileDescriptor(masterFd)
            inputStream = FileInputStream(fdObj)
            outputStream = FileOutputStream(fdObj)
            JniPtyBridge.setPtyWindowSize(masterFd, rows, cols)
            Log.i(TAG, "Started native Termux PTY session $sessionId with PID $pid and masterFd $masterFd")
        } else {
            // Managed TTY fallback process when native JNI PTY is not present in build environment
            Log.i(TAG, "Native PTY unavailable. Initializing managed TTY session for shell.")
            try {
                val pb = ProcessBuilder(targetExecutable)
                    .directory(workingDir)
                    .redirectErrorStream(true)

                val pbEnv = pb.environment()
                pbEnv.putAll(env)

                val p = pb.start()
                fallbackProcess = p
                inputStream = p.inputStream
                outputStream = p.outputStream
                pid = try {
                    val field = p.javaClass.getDeclaredField("pid")
                    field.isAccessible = true
                    field.getInt(p)
                } catch (e: Throwable) {
                    -1
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to start shell process: ${e.message}", e)
                onOutputReceived("\n[Failed to start terminal session: ${e.message}]\n")
                onSessionTerminated(-1)
                return
            }
        }

        isRunning = true
        startReadingStream()
    }

    private fun startReadingStream() {
        readJob = scope.launch {
            val inStream = inputStream ?: return@launch
            val buffer = ByteArray(4096)
            try {
                while (isRunning) {
                    val bytesRead = inStream.read(buffer)
                    if (bytesRead == -1) break
                    if (bytesRead > 0) {
                        val text = String(buffer, 0, bytesRead, Charsets.UTF_8)
                        onOutputReceived(text)
                    }
                }
            } catch (e: Exception) {
                if (isRunning) {
                    Log.w(TAG, "Error reading from PTY session stream: ${e.message}")
                }
            } finally {
                isRunning = false
                val exitCode = fallbackProcess?.exitValue() ?: 0
                onSessionTerminated(exitCode)
            }
        }
    }

    /**
     * Writes user input or commands into the TTY session master descriptor.
     */
    fun writeInput(data: String) {
        if (!isRunning) return
        val outStream = outputStream ?: return
        scope.launch {
            try {
                outStream.write(data.toByteArray(Charsets.UTF_8))
                outStream.flush()
            } catch (e: Exception) {
                Log.e(TAG, "Error writing to TTY session input: ${e.message}", e)
            }
        }
    }

    /**
     * Sends control keys / ASCII terminal escape sequences to TTY.
     */
    fun sendControlKey(key: String) {
        when (key) {
            "ESC" -> writeInput("\u001b")
            "CTRL_C" -> writeInput("\u0003")
            "TAB" -> writeInput("\t")
            "UP" -> writeInput("\u001b[A")
            "DOWN" -> writeInput("\u001b[B")
            "RIGHT" -> writeInput("\u001b[C")
            "LEFT" -> writeInput("\u001b[D")
            else -> writeInput(key)
        }
    }

    /**
     * Resizes the TTY terminal window dimensions.
     */
    fun updateWindowSize(newRows: Int, newCols: Int, widthPx: Int = 0, heightPx: Int = 0) {
        this.rows = newRows
        this.cols = newCols
        if (masterFd > 0) {
            JniPtyBridge.setPtyWindowSize(masterFd, newRows, newCols, widthPx, heightPx)
        }
    }

    /**
     * Sends a Linux POSIX signal (e.g. SIGINT = 2, SIGHUP = 1, SIGKILL = 9) to session PID.
     */
    fun sendSignal(signal: Int) {
        if (pid > 0) {
            JniPtyBridge.signalProcessNative(pid, signal)
        }
    }

    /**
     * Terminates and cleans up the TTY session.
     */
    fun destroySession() {
        isRunning = false
        readJob?.cancel()

        if (masterFd > 0) {
            JniPtyBridge.closePty(masterFd)
            masterFd = -1
        }

        fallbackProcess?.destroy()
        fallbackProcess = null
    }
}
