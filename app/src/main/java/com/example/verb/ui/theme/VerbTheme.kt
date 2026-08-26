package com.example.verb.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

val DarkBackground = Color(0xFF101216)
val DarkSurface = Color(0xFF181B22)
val DarkSurfaceVariant = Color(0xFF222630)
val PrimaryIndigo = Color(0xFF6366F1)
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

    MaterialTheme(
        colorScheme = colors,
        content = content
    )
}
