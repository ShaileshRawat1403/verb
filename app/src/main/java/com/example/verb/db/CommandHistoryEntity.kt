package com.example.verb.db

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "command_history")
data class CommandHistoryEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val queryText: String,
    val intentId: String,
    val title: String,
    val summary: String,
    val observedOutput: String?,
    val isSuccess: Boolean,
    val timestamp: Long
)
