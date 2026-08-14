package com.example.verb.model

import android.content.Context
import android.content.SharedPreferences
import org.json.JSONArray
import org.json.JSONObject

/**
 * Lightweight JSON-based chat memory fallback for environments where Room is not available
 * (early boot, diagnostics, and headless tooling). The primary chat store is [com.example.verb.db.VerbRepository].
 */
class AgentMemoryStore(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("verb_agent_memory", Context.MODE_PRIVATE)

    fun saveMessages(messages: List<ChatMessage>) {
        try {
            val jsonArray = JSONArray()
            // Keep at most 50 most recent conversation turns for memory efficiency
            val toSave = messages.takeLast(50)
            for (msg in toSave) {
                jsonArray.put(msg.toJson())
            }
            prefs.edit().putString(KEY_CHAT_HISTORY, jsonArray.toString()).apply()
        } catch (e: Exception) {
            e.printStackTrace()
        }
    }

    fun loadMessages(): List<ChatMessage> {
        val jsonStr = prefs.getString(KEY_CHAT_HISTORY, null) ?: return defaultWelcomeMessages()
        return try {
            val jsonArray = JSONArray(jsonStr)
            val list = mutableListOf<ChatMessage>()
            for (i in 0 until jsonArray.length()) {
                val obj = jsonArray.getJSONObject(i)
                list.add(ChatMessage.fromJson(obj))
            }
            if (list.isEmpty()) defaultWelcomeMessages() else list
        } catch (e: Exception) {
            e.printStackTrace()
            defaultWelcomeMessages()
        }
    }

    fun clearMemory() {
        prefs.edit().remove(KEY_CHAT_HISTORY).apply()
    }

    private fun defaultWelcomeMessages(): List<ChatMessage> {
        return listOf(
            ChatMessage(
                sender = ChatSender.AGENT,
                text = "Hello! I am Verb AI Agent. I have persistent session memory and direct visibility into your local Android terminal session.\n\nYou can ask me questions, request command execution, analyze terminal session outputs, or ask me how to perform tasks.",
                suggestedCommands = listOf("ls -la", "pwd", "git status", "check storage")
            )
        )
    }

    companion object {
        private const val KEY_CHAT_HISTORY = "chat_history_v1"
    }
}
