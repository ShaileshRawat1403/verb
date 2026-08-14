package com.example.verb.model

import org.json.JSONArray
import org.json.JSONObject

enum class ChatSender {
    USER,
    AGENT
}

data class ChatMessage(
    val id: String = java.util.UUID.randomUUID().toString(),
    val sender: ChatSender,
    val text: String,
    val timestamp: Long = System.currentTimeMillis(),
    val actionResult: ActionResult? = null,
    val suggestedCommands: List<String> = emptyList(),
    val linkedTerminalSnippet: String? = null
) {
    fun toJson(): JSONObject {
        val obj = JSONObject()
        obj.put("id", id)
        obj.put("sender", sender.name)
        obj.put("text", text)
        obj.put("timestamp", timestamp)
        if (suggestedCommands.isNotEmpty()) {
            val arr = JSONArray()
            suggestedCommands.forEach { arr.put(it) }
            obj.put("suggestedCommands", arr)
        }
        if (!linkedTerminalSnippet.isNullOrBlank()) {
            obj.put("linkedTerminalSnippet", linkedTerminalSnippet)
        }
        return obj
    }

    companion object {
        fun fromJson(obj: JSONObject): ChatMessage {
            val id = obj.optString("id", java.util.UUID.randomUUID().toString())
            val senderStr = obj.optString("sender", ChatSender.AGENT.name)
            val sender = try { ChatSender.valueOf(senderStr) } catch (e: Exception) { ChatSender.AGENT }
            val text = obj.optString("text", "")
            val timestamp = obj.optLong("timestamp", System.currentTimeMillis())

            val commands = mutableListOf<String>()
            val cmdArr = obj.optJSONArray("suggestedCommands")
            if (cmdArr != null) {
                for (i in 0 until cmdArr.length()) {
                    commands.add(cmdArr.getString(i))
                }
            }
            val snippet = if (obj.has("linkedTerminalSnippet")) obj.optString("linkedTerminalSnippet") else null

            return ChatMessage(
                id = id,
                sender = sender,
                text = text,
                timestamp = timestamp,
                suggestedCommands = commands,
                linkedTerminalSnippet = snippet
            )
        }
    }
}
