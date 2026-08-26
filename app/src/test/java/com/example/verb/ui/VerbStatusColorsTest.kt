package com.example.verb.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.test.junit4.createComposeRule
import com.example.verb.ui.theme.VerbStatus
import com.example.verb.ui.theme.VerbTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The four status roles from `docs/UX_FOUNDATION.md` §4, and the one property that matters about
 * them: they have to be legible against the surface they are drawn on. The same green that reads on
 * a near-black panel is illegible on white, and colour that cannot be read carries nothing.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class VerbStatusColorsTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    /**
     * Both palettes are captured in one composition: the Compose rule allows `setContent` only once
     * per test, so rendering the two themes separately silently returned the first one twice.
     */
    private fun palettes(): Pair<List<Color>, List<Color>> {
        var dark: List<Color> = emptyList()
        var light: List<Color> = emptyList()
        composeTestRule.setContent {
            VerbTheme(darkTheme = true) {
                dark = listOf(
                    VerbStatus.confirmed, VerbStatus.recoverable, VerbStatus.failed, VerbStatus.caveat
                )
            }
            VerbTheme(darkTheme = false) {
                light = listOf(
                    VerbStatus.confirmed, VerbStatus.recoverable, VerbStatus.failed, VerbStatus.caveat
                )
            }
        }
        return dark to light
    }

    @Test
    fun `dark theme status colours are light enough to read on a dark surface`() {
        palettes().first.forEach {
            assertTrue("$it is too dark for a dark surface", it.luminance() > 0.15f)
        }
    }

    @Test
    fun `light theme status colours are dark enough to read on a light surface`() {
        palettes().second.forEach {
            assertTrue("$it is too light for a light surface", it.luminance() < 0.40f)
        }
    }

    /** Four roles, four distinct colours: two that matched would make a state unreadable. */
    @Test
    fun `the four roles are distinct in both themes`() {
        val (dark, light) = palettes()
        assertTrue(dark.toSet().size == 4)
        assertTrue(light.toSet().size == 4)
        // And the two themes really are different palettes, not one reused.
        assertTrue(dark.toSet() != light.toSet())
    }
}
