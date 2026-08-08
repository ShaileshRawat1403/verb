package com.example.verb.terminal

import android.util.Log
import java.io.File
import java.io.FileDescriptor

/**
 * JNI Bindings for Termux Pseudo-Terminal (PTY) and TTY session management.
 * Provides native bindings to libtermux-pty / openpty / createSubprocess JNI functions.
 */
object JniPtyBridge {

    private const val TAG = "JniPtyBridge"
    private var isNativeLibraryLoaded = false

    init {
        try {
            System.loadLibrary("termux-pty")
            isNativeLibraryLoaded = true
            Log.i(TAG, "Successfully loaded libtermux-pty.so JNI library.")
        } catch (e: Throwable) {
            isNativeLibraryLoaded = false
            Log.w(TAG, "libtermux-pty.so native library not present. Operating with managed TTY session fallback.", e)
        }
    }

    /**
     * Native call to create a Termux PTY subprocess.
     * @param executable Path to shell executable (e.g., /system/bin/sh or Termux bootstrap shell).
     * @param cwd Working directory path.
     * @param args Command line arguments.
     * @param env Environment key-value strings.
     * @param processId Out parameter array for process ID [0].
     * @param masterFd Out parameter array for master PTY file descriptor [0].
     * @return 0 on success, or negative error code.
     */
    external fun createSubprocessNative(
        executable: String,
        cwd: String,
        args: Array<String>,
        env: Array<String>,
        processId: IntArray,
        masterFd: IntArray
    ): Int

    /**
     * Native call to update PTY window size (TIOCSWINSZ winsize struct).
     */
    external fun setPtyWindowSizeNative(fd: Int, rows: Int, cols: Int, widthPx: Int, heightPx: Int): Int

    /**
     * Native call to close PTY master descriptor.
     */
    external fun closePtyNative(fd: Int)

    /**
     * Native call to send signal to PTY process group.
     */
    external fun signalProcessNative(pid: Int, signal: Int): Int

    /**
     * Public wrapper for PTY creation with JNI / fallback strategy.
     */
    fun createSubprocess(
        executable: String,
        cwd: File,
        args: Array<String> = emptyArray(),
        env: Map<String, String> = emptyMap(),
        processIdOut: IntArray = IntArray(1),
        masterFdOut: IntArray = IntArray(1)
    ): Boolean {
        if (isNativeLibraryLoaded) {
            try {
                val envArray = env.map { "${it.key}=${it.value}" }.toTypedArray()
                val ret = createSubprocessNative(
                    executable = executable,
                    cwd = cwd.absolutePath,
                    args = args,
                    env = envArray,
                    processId = processIdOut,
                    masterFd = masterFdOut
                )
                return ret == 0 && masterFdOut[0] > 0
            } catch (e: Throwable) {
                Log.e(TAG, "JNI createSubprocess failed: ${e.message}", e)
            }
        }
        return false
    }

    /**
     * Public wrapper for setting PTY window dimensions.
     */
    fun setPtyWindowSize(masterFd: Int, rows: Int, cols: Int, widthPx: Int = 0, heightPx: Int = 0) {
        if (isNativeLibraryLoaded && masterFd > 0) {
            try {
                setPtyWindowSizeNative(masterFd, rows, cols, widthPx, heightPx)
            } catch (e: Throwable) {
                Log.e(TAG, "JNI setPtyWindowSize failed: ${e.message}", e)
            }
        }
    }

    /**
     * Public wrapper to close PTY file descriptor.
     */
    fun closePty(masterFd: Int) {
        if (isNativeLibraryLoaded && masterFd > 0) {
            try {
                closePtyNative(masterFd)
            } catch (e: Throwable) {
                Log.e(TAG, "JNI closePty failed: ${e.message}", e)
            }
        }
    }

    /**
     * Create FileDescriptor object from integer file descriptor handle via reflection.
     */
    fun createFileDescriptor(fd: Int): FileDescriptor {
        val fileDescriptor = FileDescriptor()
        try {
            val field = FileDescriptor::class.java.getDeclaredField("descriptor")
            field.isAccessible = true
            field.setInt(fileDescriptor, fd)
        } catch (e: Throwable) {
            Log.e(TAG, "Failed to reflect FileDescriptor descriptor field", e)
        }
        return fileDescriptor
    }
}
