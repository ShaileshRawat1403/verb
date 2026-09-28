package com.example.verb.viewmodel

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import com.example.verb.mobile.DesktopBridgeClient
import com.example.verb.mobile.DesktopScreen

/** Keeps a paired live connection through rotation without saving its capability to disk. */
class DesktopPhoneViewModel : ViewModel() {
    var linkText by mutableStateOf("")
    var pairedLink by mutableStateOf<String?>(null)
    var client by mutableStateOf<DesktopBridgeClient?>(null)
    var token by mutableStateOf<String?>(null)
    var screen by mutableStateOf<DesktopScreen?>(null)
    var status by mutableStateOf("Paste the link from your desktop Verb terminal.")
    var connected by mutableStateOf(false)
    var busy by mutableStateOf(false)
    var command by mutableStateOf("")
}
