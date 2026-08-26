package com.example.verb.ui.theme

import android.content.Context

/**
 * Whether Verb follows the device, or is told.
 *
 * [SYSTEM] is the default and means "keep following", so a fresh install behaves exactly as it did
 * before this existed. The other two are an override, not a preference tree: `docs/BACKLOG.md` §D0
 * requires configuration to be *found by name*, never browsed, which is why this is reached by
 * typing "theme" into the Verb sheet rather than by opening settings and hunting for a section.
 */
enum class VerbThemeChoice(val label: String, val description: String) {
    SYSTEM("System", "Follow the device setting"),
    LIGHT("Light", "Always light, whatever the device says"),
    DARK("Dark", "Always dark, whatever the device says");

    /** [systemIsDark] is what the device reports; only [SYSTEM] pays any attention to it. */
    fun resolveDark(systemIsDark: Boolean): Boolean = when (this) {
        SYSTEM -> systemIsDark
        LIGHT -> false
        DARK -> true
    }
}

/**
 * Stores the choice. One key, one file, no schema.
 *
 * An unreadable or unrecognised value resolves to [VerbThemeChoice.SYSTEM] rather than throwing:
 * a preference Verb cannot parse is not a reason to fail to draw the app.
 */
class VerbThemeStore(context: Context) {

    private val preferences =
        context.applicationContext.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    fun load(): VerbThemeChoice {
        val stored = preferences.getString(KEY, null) ?: return VerbThemeChoice.SYSTEM
        return VerbThemeChoice.entries.firstOrNull { it.name == stored } ?: VerbThemeChoice.SYSTEM
    }

    fun save(choice: VerbThemeChoice) {
        preferences.edit().putString(KEY, choice.name).apply()
    }

    private companion object {
        const val FILE = "verb_appearance"
        const val KEY = "theme_choice"
    }
}
