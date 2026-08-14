package com.example.verb.db

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "terminal_outputs")
data class TerminalOutputEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val command: String,
    val output: String,
    val workingDirectory: String?,
    val isError: Boolean,
    val timestamp: Long
)
