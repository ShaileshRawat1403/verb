package com.example.verb.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import kotlinx.coroutines.flow.Flow

@Dao
interface CommandHistoryDao {
    @Query("SELECT * FROM command_history ORDER BY timestamp DESC")
    fun getAllCommandHistory(): Flow<List<CommandHistoryEntity>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertCommand(command: CommandHistoryEntity): Long

    @Query("DELETE FROM command_history")
    suspend fun clearAll()

    @Query("DELETE FROM command_history WHERE id = :id")
    suspend fun deleteById(id: Long)
}
