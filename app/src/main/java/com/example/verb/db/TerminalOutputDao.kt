package com.example.verb.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import kotlinx.coroutines.flow.Flow

@Dao
interface TerminalOutputDao {
    @Query("SELECT * FROM terminal_outputs ORDER BY timestamp DESC")
    fun getAllTerminalOutputs(): Flow<List<TerminalOutputEntity>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertTerminalOutput(entry: TerminalOutputEntity): Long

    @Query("DELETE FROM terminal_outputs")
    suspend fun clearAll()

    @Query("SELECT * FROM terminal_outputs WHERE command LIKE '%' || :query || '%' OR output LIKE '%' || :query || '%' ORDER BY timestamp DESC")
    fun searchOutputs(query: String): Flow<List<TerminalOutputEntity>>
}
