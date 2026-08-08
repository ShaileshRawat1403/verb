package com.termux.terminal

import android.util.Log
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.InputStream
import java.io.OutputStream
import java.util.UUID

/**
 * Authentic Termux TerminalSession connecting native PTY file descriptors to TerminalEmulator state and TTY streams.
 * Copyright (C) Termux team.
 */
class TerminalSession(
    val shellPath: String,
    val cwd: String,
    val args: Array<String>,
    val env: Array<String>,
    var client: TerminalSessionClient?
) {

    val handle: String = UUID.randomUUID().toString()
    var pid: Int = -1
    private var terminalFileDescriptor: Int = -1

    var emulator: TerminalEmulator = TerminalEmulator(80, 24, 2000, this)
        private set

    private var terminalInputStream: InputStream? = null
    private var terminalOutputStream: OutputStream? = null

    val processQueue = ByteQueue(4096)
    var isRunning: Boolean = false
        private set

    var exitCode: Int = -1
        private set

    private var readerThread: Thread? = null
    private var processWaitThread: Thread? = null

    init {
        initializeSession()
    }

    private fun initializeSession() {
        if (!JNI.isLoaded()) {
            Log.e("TerminalSession", "Native PTY library (libtermux.so) is not loaded.")
            return
        }

        val pidOut = IntArray(1)
        val masterFdOut = IntArray(1)

        val result = JNI.createSubprocess(
            executable = shellPath,
            cwd = cwd,
            args = args,
            envVars = env,
            processId = pidOut,
            masterFd = masterFdOut
        )

        if (result == 0 && masterFdOut[0] > 0) {
            this.pid = pidOut[0]
            this.terminalFileDescriptor = masterFdOut[0]
            this.isRunning = true

            val fdObj = createFileDescriptor(terminalFileDescriptor)
            terminalInputStream = FileInputStream(fdObj)
            terminalOutputStream = FileOutputStream(fdObj)

            JNI.setPtyUTF8Mode(terminalFileDescriptor)
            JNI.setPtyWindowSize(terminalFileDescriptor, emulator.rows, emulator.columns, 0, 0)

            startIoThreads()
        } else {
            Log.e("TerminalSession", "JNI createSubprocess failed with code $result")
            isRunning = false
        }
    }

    private fun startIoThreads() {
        readerThread = Thread({
            val inStream = terminalInputStream ?: return@Thread
            val buffer = ByteArray(4096)
            try {
                while (isRunning) {
                    val bytesRead = inStream.read(buffer)
                    if (bytesRead == -1) break
                    if (bytesRead > 0) {
                        emulator.append(buffer, bytesRead)
                    }
                }
            } catch (e: Throwable) {
                Log.w("TerminalSession", "Terminal reader thread closed: ${e.message}")
            } finally {
                finishIfRunning()
            }
        }, "TermuxReader-$handle")
        readerThread?.start()

        processWaitThread = Thread({
            if (pid > 0 && JNI.isLoaded()) {
                val status = JNI.waitFor(pid)
                exitCode = status
                finishIfRunning()
            }
        }, "TermuxWaiter-$handle")
        processWaitThread?.start()
    }

    fun write(data: String) {
        write(data.toByteArray(Charsets.UTF_8))
    }

    fun write(data: ByteArray) {
        if (!isRunning) return
        val outStream = terminalOutputStream ?: return
        try {
            outStream.write(data)
            outStream.flush()
        } catch (e: Throwable) {
            Log.e("TerminalSession", "Error writing to TTY output stream: ${e.message}")
        }
    }

    fun updateSize(rows: Int, cols: Int) {
        emulator.updateSize(cols, rows)
        if (terminalFileDescriptor > 0 && JNI.isLoaded()) {
            JNI.setPtyWindowSize(terminalFileDescriptor, rows, cols, 0, 0)
        }
    }

    fun finishIfRunning() {
        if (!isRunning) return
        isRunning = false

        if (terminalFileDescriptor > 0 && JNI.isLoaded()) {
            JNI.close(terminalFileDescriptor)
            terminalFileDescriptor = -1
        }

        client?.onSessionFinished(this)
    }

    private fun createFileDescriptor(fd: Int): java.io.FileDescriptor {
        val fileDescriptor = java.io.FileDescriptor()
        try {
            val field = java.io.FileDescriptor::class.java.getDeclaredField("descriptor")
            field.isAccessible = true
            field.setInt(fileDescriptor, fd)
        } catch (e: Throwable) {
            Log.e("TerminalSession", "Failed reflecting FileDescriptor descriptor field", e)
        }
        return fileDescriptor
    }
}
