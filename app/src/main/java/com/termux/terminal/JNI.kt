package com.termux.terminal

import android.util.Log

/**
 * Authentic Termux JNI bindings for native PTY subprocess creation and TTY control.
 * Copyright (C) Termux team (Apache 2.0 / MIT)
 */
object JNI {

    private var isNativeLibraryLoaded = false

    init {
        try {
            System.loadLibrary("termux")
            isNativeLibraryLoaded = true
            Log.i("TermuxJNI", "Successfully loaded libtermux.so native PTY library.")
        } catch (t: Throwable) {
            isNativeLibraryLoaded = false
            Log.w("TermuxJNI", "libtermux.so not available in environment: ${t.message}")
        }
    }

    fun isLoaded(): Boolean = isNativeLibraryLoaded

    @JvmStatic
    external fun createSubprocess(
        executable: String,
        cwd: String,
        args: Array<String>,
        envVars: Array<String>,
        processId: IntArray,
        masterFd: IntArray
    ): Int

    @JvmStatic
    external fun setPtyWindowSize(fd: Int, rows: Int, cols: Int, widthPx: Int, heightPx: Int)

    @JvmStatic
    external fun setPtyUTF8Mode(fd: Int)

    @JvmStatic
    external fun close(fd: Int)

    @JvmStatic
    external fun waitFor(processId: Int): Int
}
