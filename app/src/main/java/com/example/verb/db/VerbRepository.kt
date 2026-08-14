package com.example.verb.db

import android.content.Context
import com.example.verb.model.ActionResult
import com.example.verb.model.ChatMessage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.withContext

/**
 * Single access point for Verb's local Room persistence. Command history, terminal output
 * transcripts, and assistant chat messages survive process restarts here.
 */
class VerbRepository private constructor(private val database: VerbDatabase) {

    val commandHistory: Flow<List<CommandHistoryEntity>> = database.commandHistoryDao().getAllCommandHistory()
    val terminalOutputs: Flow<List<TerminalOutputEntity>> = database.terminalOutputDao().getAllTerminalOutputs()
    val chatMessagesFlow: Flow<List<ChatMessageEntity>> = database.chatMessageDao().getAllChatMessages()

    suspend fun recordCommand(query: String, result: ActionResult) = withContext(Dispatchers.IO) {
        val entity = CommandHistoryEntity(
            queryText = query,
            intentId = result.intentId,
            title = result.title,
            summary = result.summary,
            observedOutput = result.observedOutput,
            isSuccess = result.isSuccess,
            timestamp = result.timestamp
        )
        database.commandHistoryDao().insertCommand(entity)
    }

    suspend fun recordTerminalOutput(
        command: String,
        output: String,
        workingDir: String? = null,
        isError: Boolean = false
    ) = withContext(Dispatchers.IO) {
        val entity = TerminalOutputEntity(
            command = command,
            output = output,
            workingDirectory = workingDir,
            isError = isError,
            timestamp = System.currentTimeMillis()
        )
        database.terminalOutputDao().insertTerminalOutput(entity)
    }

    suspend fun saveChatMessage(message: ChatMessage) = withContext(Dispatchers.IO) {
        val entity = ChatMessageEntity.fromChatMessage(message)
        database.chatMessageDao().insertChatMessage(entity)
    }

    suspend fun saveAllChatMessages(messages: List<ChatMessage>) = withContext(Dispatchers.IO) {
        val entities = messages.map { ChatMessageEntity.fromChatMessage(it) }
        database.chatMessageDao().insertAll(entities)
    }

    suspend fun loadChatMessages(): List<ChatMessage> = withContext(Dispatchers.IO) {
        val entities = database.chatMessageDao().getChatMessagesList()
        entities.map { it.toChatMessage() }
    }

    suspend fun clearSessionData() = withContext(Dispatchers.IO) {
        database.commandHistoryDao().clearAll()
        database.terminalOutputDao().clearAll()
        database.chatMessageDao().clearAll()
    }

    companion object {
        @Volatile
        private var INSTANCE: VerbRepository? = null

        fun getInstance(context: Context): VerbRepository {
            return INSTANCE ?: synchronized(this) {
                val db = VerbDatabase.getInstance(context)
                val instance = VerbRepository(db)
                INSTANCE = instance
                instance
            }
        }
    }
}
