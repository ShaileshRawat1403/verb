package com.example.verb.db

import androidx.room.Entity
import androidx.room.PrimaryKey
import com.example.verb.model.ChatMessage
import com.example.verb.model.ChatSender
import org.json.JSONArray

@Entity(tableName = "chat_messages")
data class ChatMessageEntity(
    @PrimaryKey val id: String,
    val sender: String,
    val text: String,
    val timestamp: Long,
    val actionTitle: String? = null,
    val actionSummary: String? = null,
    val actionObservedOutput: String? = null,
    val suggestedCommandsJson: String? = null,
    val linkedTerminalSnippet: String? = null
) {
    fun toChatMessage(): ChatMessage {
        val cmds = mutableListOf<String>()
        if (!suggestedCommandsJson.isNullOrBlank()) {
            try {
                val arr = JSONArray(suggestedCommandsJson)
                for (i in 0 until arr.length()) {
                    cmds.add(arr.getString(i))
                }
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
        val senderEnum = try {
            ChatSender.valueOf(sender)
        } catch (e: Exception) {
            ChatSender.AGENT
        }
        return ChatMessage(
            id = id,
            sender = senderEnum,
            text = text,
            timestamp = timestamp,
            suggestedCommands = cmds,
            linkedTerminalSnippet = linkedTerminalSnippet
        )
    }

    companion object {
        fun fromChatMessage(msg: ChatMessage): ChatMessageEntity {
            val cmdsJson = if (msg.suggestedCommands.isNotEmpty()) {
                val arr = JSONArray()
                msg.suggestedCommands.forEach { arr.put(it) }
                arr.toString()
            } else null

            return ChatMessageEntity(
                id = msg.id,
                sender = msg.sender.name,
                text = msg.text,
                timestamp = msg.timestamp,
                actionTitle = msg.actionResult?.title,
                actionSummary = msg.actionResult?.summary,
                actionObservedOutput = msg.actionResult?.observedOutput,
                suggestedCommandsJson = cmdsJson,
                linkedTerminalSnippet = msg.linkedTerminalSnippet
            )
        }
    }
}
