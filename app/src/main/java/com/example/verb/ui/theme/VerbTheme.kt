package com.example.verb.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

val DarkBackground = Color(0xFF101216)
val DarkSurface = Color(0xFF181B22)
val DarkSurfaceVariant = Color(0xFF222630)

/**
 * The terminal emulator canvas. Deliberately NOT a colorScheme token: a terminal is dark in both
 * themes, so this must not follow the system setting. The chrome around the terminal follows
 * `VerbTheme`; the viewport the shell draws into does not. Light text on a themed background was
 * exactly the light-mode regression this constant exists to prevent.
 */
val TerminalCanvas = Color(0xFF0D0E12)

/**
 * The four status roles, and there are exactly four.
 *
 * `docs/UX_FOUNDATION.md` §4 names them and no more:
 *
 * ```
 * green    confirmed · running · succeeded
 * yellow   recoverable · needs a decision
 * red      an observed failure
 * dim      secondary · unknown · caveat
 * ```
 *
 * They live here rather than in each screen because `MaterialTheme.colorScheme` has no slot for
 * "recoverable" -- these are Verb's own vocabulary, not Material's. The light values are darker
 * than the dark ones on purpose: the same hue that reads as green on a near-black panel is
 * illegible on white, and colour that cannot be read is colour that carries nothing.
 *
 * Colour never carries meaning alone. Every state that uses these also carries a glyph and a word
 * from [com.example.verb.ui.VerbStatusVocabulary]; these only make an already-readable status
 * quicker to scan.
 */
@Immutable
data class VerbStatusColors(
    val confirmed: Color,
    val recoverable: Color,
    val failed: Color,
    val caveat: Color
)

private val DarkStatusColors = VerbStatusColors(
    confirmed = Color(0xFF22C55E),
    recoverable = Color(0xFFEAB308),
    failed = Color(0xFFEF4444),
    caveat = Color(0xFF94A3B8)
)

private val LightStatusColors = VerbStatusColors(
    confirmed = Color(0xFF15803D),
    recoverable = Color(0xFFA16207),
    failed = Color(0xFFB91C1C),
    caveat = Color(0xFF64748B)
)

val LocalVerbStatusColors = staticCompositionLocalOf { DarkStatusColors }

/** The status palette for the current theme. Read this, never a literal. */
val VerbStatus: VerbStatusColors
    @Composable
    @ReadOnlyComposable
    get() = LocalVerbStatusColors.current

val PrimaryIndigo = Color(0xFF6366F1)

/**
 * The terminal dock's palette, and it does not follow the theme.
 *
 * The dock -- the command field and the key row -- sits directly beneath the terminal viewport and
 * reads as part of it. Theming it would strand a white input strip under a dark canvas, which is
 * the same mistake in reverse as the light-mode regression [TerminalCanvas] exists to prevent.
 *
 * The Quick Keys editor is deliberately *not* in here. A sheet is a surface Verb puts in front of
 * your work rather than part of the work, so it follows the theme like every other sheet.
 */
object TerminalDock {
    val surface = Color(0xFF161820)
    val field = TerminalCanvas
    val fieldFocused = Color(0xFF10131B)
    val key = Color(0xFF222630)
    val outline = Color(0xFF3B4252)
    val text = Color(0xFFE2E8F0)
    val dim = Color(0xFF94A3B8)
    val accent = PrimaryIndigo
    val accentSoft = Color(0xFF818CF8)
}
val SecondaryCyan = Color(0xFF38BDF8)
val AccentAmber = Color(0xFFF59E0B)
val DangerRed = Color(0xFFEF4444)
val TextPrimary = Color(0xFFF3F4F6)
val TextSecondary = Color(0xFF9CA3AF)

private val VerbDarkColorScheme = darkColorScheme(
    primary = PrimaryIndigo,
    onPrimary = Color.White,
    secondary = SecondaryCyan,
    onSecondary = Color.Black,
    tertiary = AccentAmber,
    error = DangerRed,
    background = DarkBackground,
    onBackground = TextPrimary,
    surface = DarkSurface,
    onSurface = TextPrimary,
    surfaceVariant = DarkSurfaceVariant,
    onSurfaceVariant = TextSecondary
)

private val VerbLightColorScheme = lightColorScheme(
    primary = PrimaryIndigo,
    onPrimary = Color.White,
    secondary = SecondaryCyan,
    onSecondary = Color.Black,
    tertiary = AccentAmber,
    error = DangerRed,
    background = Color(0xFFF8FAFC),
    onBackground = Color(0xFF0F172A),
    surface = Color.White,
    onSurface = Color(0xFF0F172A),
    surfaceVariant = Color(0xFFF1F5F9),
    onSurfaceVariant = Color(0xFF64748B)
)

@Composable
fun VerbTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {
    val colors = if (darkTheme) VerbDarkColorScheme else VerbLightColorScheme
    val status = if (darkTheme) DarkStatusColors else LightStatusColors

    CompositionLocalProvider(LocalVerbStatusColors provides status) {
        MaterialTheme(
            colorScheme = colors,
            content = content
        )
    }
}
