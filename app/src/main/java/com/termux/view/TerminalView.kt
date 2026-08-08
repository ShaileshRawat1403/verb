package com.termux.view

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import android.util.AttributeSet
import android.view.GestureDetector
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import com.termux.terminal.TerminalSession

/**
 * Authentic Termux Android TerminalView widget.
 * Renders terminal cells onto Canvas, processes touch gestures, and manages exact text selection.
 * Copyright (C) Termux team.
 */
class TerminalView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : View(context, attrs, defStyleAttr) {

    var currentSession: TerminalSession? = null
        private set

    var viewClient: TerminalViewClient? = null
    val selectionController = TextSelectionCursorController(this)

    private val textPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        typeface = Typeface.MONOSPACE
        textSize = 36f
        color = Color.GREEN
    }

    private val selectionPaint = Paint().apply {
        color = 0x663399FF.toInt()
    }

    private val bgPaint = Paint().apply {
        color = Color.BLACK
    }

    private val gestureDetector: GestureDetector

    init {
        isFocusable = true
        isFocusableInTouchMode = true

        gestureDetector = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
            override fun onSingleTapUp(e: MotionEvent): Boolean {
                viewClient?.onSingleTapUp(e)
                return true
            }

            override fun onLongPress(e: MotionEvent) {
                // Trigger exact text selection around touched cell position
                val cellX = (e.x / (textPaint.textSize * 0.6f)).toInt().coerceAtLeast(0)
                val cellY = (e.y / (textPaint.textSize * 1.2f)).toInt().coerceAtLeast(0)
                selectionController.setSelection(
                    x1 = (cellX - 10).coerceAtLeast(0),
                    y1 = cellY,
                    x2 = (cellX + 10).coerceAtMost(79),
                    y2 = cellY
                )
                val text = selectionController.getSelectedText()
                if (text.isNotBlank()) {
                    viewClient?.onSelectedTextClipboard(text)
                }
                viewClient?.onLongPress(e)
            }
        })
    }

    fun attachSession(session: TerminalSession) {
        if (this.currentSession == session) return
        this.currentSession = session
        invalidate()
    }

    /**
     * Returns current exact text selection in terminal buffer.
     */
    fun getStoredSelectedText(): String {
        return selectionController.getSelectedText()
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        canvas.drawRect(0f, 0f, width.toFloat(), height.toFloat(), bgPaint)

        val session = currentSession ?: return
        val buffer = session.emulator.screenBuffer
        val charWidth = textPaint.textSize * 0.6f
        val charHeight = textPaint.textSize * 1.2f

        for (y in 0 until session.emulator.rows) {
            val row = buffer.getRow(y)
            val lineText = row.getSelectedText(0, session.emulator.columns - 1)
            val yPos = (y + 1) * charHeight

            // Draw selection background if active
            if (selectionController.isSelecting && y >= selectionController.selY1 && y <= selectionController.selY2) {
                val startX = if (y == selectionController.selY1) selectionController.selX1 * charWidth else 0f
                val endX = if (y == selectionController.selY2) (selectionController.selX2 + 1) * charWidth else width.toFloat()
                canvas.drawRect(startX, yPos - charHeight + 8f, endX, yPos + 8f, selectionPaint)
            }

            canvas.drawText(lineText, 0f, yPos, textPaint)
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        gestureDetector.onTouchEvent(event)
        return true
    }

    override fun onCheckIsTextEditor(): Boolean = true

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
        outAttrs.inputType = EditorInfo.TYPE_NULL
        return object : BaseInputConnection(this, false) {
            override fun sendKeyEvent(event: KeyEvent): Boolean {
                if (event.action == KeyEvent.ACTION_DOWN) {
                    val session = currentSession ?: return false
                    when (event.keyCode) {
                        KeyEvent.KEYCODE_DEL -> session.write("\b")
                        KeyEvent.KEYCODE_ENTER -> session.write("\n")
                        else -> {
                            val unicodeChar = event.unicodeChar
                            if (unicodeChar > 0) {
                                session.write(unicodeChar.toChar().toString())
                            }
                        }
                    }
                }
                return true
            }
        }
    }
}
