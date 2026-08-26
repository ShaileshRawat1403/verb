package com.example.verb.ui.theme

import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class VerbThemeChoiceTest {

    private fun store() = VerbThemeStore(ApplicationProvider.getApplicationContext())

    /** Only SYSTEM looks at the device. The other two are an override and must ignore it. */
    @Test
    fun `system follows the device and the overrides do not`() {
        assertEquals(true, VerbThemeChoice.SYSTEM.resolveDark(systemIsDark = true))
        assertEquals(false, VerbThemeChoice.SYSTEM.resolveDark(systemIsDark = false))

        assertEquals(false, VerbThemeChoice.LIGHT.resolveDark(systemIsDark = true))
        assertEquals(true, VerbThemeChoice.DARK.resolveDark(systemIsDark = false))
    }

    /** A fresh install must behave exactly as it did before this setting existed. */
    @Test
    fun `the default is system`() {
        assertEquals(VerbThemeChoice.SYSTEM, store().load())
    }

    @Test
    fun `a saved choice survives a new store`() {
        store().save(VerbThemeChoice.LIGHT)

        assertEquals(VerbThemeChoice.LIGHT, store().load())
    }

    /**
     * A preference Verb cannot parse is not a reason to fail to draw the app. An older or newer
     * build's value resolves to the default rather than throwing.
     */
    @Test
    fun `an unrecognised stored value falls back to system`() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        context.getSharedPreferences("verb_appearance", android.content.Context.MODE_PRIVATE)
            .edit().putString("theme_choice", "SEPIA").apply()

        assertEquals(VerbThemeChoice.SYSTEM, store().load())
    }
}
