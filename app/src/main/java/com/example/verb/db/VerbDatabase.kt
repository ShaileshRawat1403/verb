package com.example.verb.db

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase

@Database(
    entities = [
        CommandHistoryEntity::class,
        TerminalOutputEntity::class,
        ChatMessageEntity::class
    ],
    version = 1,
    exportSchema = false
)
abstract class VerbDatabase : RoomDatabase() {

    abstract fun commandHistoryDao(): CommandHistoryDao
    abstract fun terminalOutputDao(): TerminalOutputDao
    abstract fun chatMessageDao(): ChatMessageDao

    companion object {
        @Volatile
        private var INSTANCE: VerbDatabase? = null

        fun getInstance(context: Context): VerbDatabase {
            return INSTANCE ?: synchronized(this) {
                val instance = Room.databaseBuilder(
                    context.applicationContext,
                    VerbDatabase::class.java,
                    "verb_database"
                )
                .fallbackToDestructiveMigration(true)
                .build()
                INSTANCE = instance
                instance
            }
        }
    }
}
