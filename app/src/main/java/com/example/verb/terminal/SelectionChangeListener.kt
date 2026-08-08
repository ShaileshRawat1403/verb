package com.example.verb.terminal

import androidx.compose.ui.text.TextRange

/**
 * Listener interface for capturing active text selections within the terminal buffer.
 * Passes the exact selected text range and string to the SemanticEngine for context-aware inspection.
 */
fun interface SelectionChangeListener {
    /**
     * Triggered when the active selection within the terminal buffer changes.
     * @param selectedRange The character range [start, end] of the selection in the terminal text.
     * @param selectedText The exact text string captured within the selection range.
     */
    fun onSelectionChanged(selectedRange: TextRange, selectedText: String)
}
