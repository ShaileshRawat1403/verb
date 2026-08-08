package com.termux.view

import android.view.KeyEvent
import android.view.MotionEvent
import com.termux.terminal.TerminalSession

/**
 * Interface for listening to UI and touch interaction events on TerminalView.
 * Copyright (C) Termux team.
 */
interface TerminalViewClient {
    fun onScale(scale: Float): Float
    fun onSingleTapUp(e: MotionEvent)
    fun onLongPress(e: MotionEvent): Boolean
    fun onKeyDown(keyCode: Int, e: KeyEvent, session: TerminalSession?): Boolean
    fun onKeyUp(keyCode: Int, e: KeyEvent): Boolean
    fun onSelectedTextClipboard(selectedText: String)
}
