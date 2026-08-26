package com.example.verb.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onRoot
import com.example.verb.ai.AiProviderConfig
import com.example.verb.ai.AiProviderId
import com.example.verb.ai.AiProviderSettings
import com.example.verb.ui.theme.VerbTheme
import com.github.takahirom.roborazzi.captureRoboImage
import java.nio.file.Path
import kotlin.io.path.readText
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The terminal chrome follows the system theme (`VerbTheme`), while the terminal emulator view
 * itself stays dark by design. These tests hold both halves of that sentence.
 *
 * The render tests prove every migrated surface still composes under both schemes -- the failure
 * this guards against is a colour lookup that crashes or a sheet whose content silently depends on
 * a dark container.
 *
 * The source assertion is the real regression lock: any new hardcoded colour in the five chrome
 * files must either use a scheme token or be added to the documented allowlist below *with a
 * reason*. It scans both hex literals and named Compose colours (`Color.White` and friends), since
 * a named constant hardcodes exactly the same way a hex literal does.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TerminalChromeThemeTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private val readyProvider = AiProviderSettings(
        config = AiProviderConfig(AiProviderId.OPENAI, "test-model", "https://api.openai.com/v1"),
        hasApiKey = true
    )

    /**
     * One composition per test (the rule forbids a second `setContent`), containing the surface
     * twice -- once under the dark scheme, once under light. Both instances must compose.
     */
    private fun bothThemes(content: @Composable () -> Unit) {
        composeTestRule.setContent {
            Column {
                VerbTheme(darkTheme = true) { content() }
                VerbTheme(darkTheme = false) { content() }
            }
        }
    }

    private fun assertRenderedInBothThemes(tag: String) {
        composeTestRule.onAllNodesWithTag(tag).assertCountEquals(2)
    }

    /** The workspace content, shared by the render test and the viewport-colour test. */
    private fun terminalWorkspace(): @Composable () -> Unit = {
        TerminalScreen(
            terminalOutput = "",
            terminalRuntime = null,
            onSendCommand = {},
            onSendKey = {},
            onSendText = {},
            onClearTerminal = {},
            onInspectText = {},
            onSubmitIntent = {},
        )
    }

    @Test
    fun runsSheetComposesUnderDarkAndLight() {
        bothThemes { RunsSheet(terminalRuntime = null, onDismiss = {}) }
        assertRenderedInBothThemes("terminal_runs_sheet")
    }

    @Test
    fun diagnosticsSheetComposesUnderDarkAndLight() {
        bothThemes { TerminalDiagnosticsSheet(terminalRuntime = null, onDismiss = {}) }
        assertRenderedInBothThemes("terminal_diagnostics_sheet")
    }

    @Test
    fun fileExplorerComposesUnderDarkAndLight() {
        bothThemes { FileExplorerDrawer(terminalRuntime = null, onFileClicked = {}) }
        assertRenderedInBothThemes("file_explorer_current_path")
    }

    @Test
    fun usbDiagnosticCardComposesUnderDarkAndLight() {
        bothThemes { UsbDebuggingDiagnosticCard() }
        assertRenderedInBothThemes("usb_debugging_card")
    }

    @Test
    fun terminalWorkspaceComposesUnderDarkAndLight() {
        bothThemes(content = terminalWorkspace())
        assertRenderedInBothThemes("verb_sheet_trigger")
    }

    /**
     * The regression this exists to hold: the Task 1 migration themed the root workspace
     * background, the Termux canvas is transparent, and light terminal text on a light page was
     * unreadable -- while all 552 tests stayed green. The viewport must sample identical pixels
     * under both schemes, and those pixels must be dark in both.
     *
     * Compose's own captureToImage cannot produce a frame on Robolectric, so the screenshot comes
     * from Roborazzi, which renders one through the same native-graphics pipeline.
     */
    @Test
    fun terminalViewportKeepsItsColourAcrossThemes() {
        composeTestRule.setContent {
            Column(modifier = Modifier.fillMaxSize()) {
                Box(modifier = Modifier.weight(1f)) {
                    VerbTheme(darkTheme = true) { terminalWorkspace()() }
                }
                Box(modifier = Modifier.weight(1f)) {
                    VerbTheme(darkTheme = false) { terminalWorkspace()() }
                }
            }
        }

        val screenshot = java.io.File.createTempFile("terminal-viewport", ".png")
        screenshot.deleteOnExit()
        composeTestRule.onRoot().captureRoboImage(screenshot)
        val bitmap = android.graphics.BitmapFactory.decodeFile(screenshot.absolutePath)
        assertTrue(
            "The theme screenshot did not render (${bitmap.width}x${bitmap.height}).",
            bitmap.width > 0 && bitmap.height > 0
        )

        fun sample(fractionX: Float, fractionY: Float): Int {
            val x = (bitmap.width * fractionX).toInt().coerceIn(0, bitmap.width - 1)
            val y = (bitmap.height * fractionY).toInt().coerceIn(0, bitmap.height - 1)
            return bitmap.getPixel(x, y)
        }

        // Middle of each half's viewport band: header above, keyboard dock below.
        val darkHalf = sample(0.5f, 0.30f)
        val lightHalf = sample(0.5f, 0.80f)

        assertEquals(
            "The terminal viewport changed colour between themes " +
                "(dark=${Integer.toHexString(darkHalf)}, light=${Integer.toHexString(lightHalf)}). " +
                "The canvas must stay dark in both.",
            darkHalf,
            lightHalf
        )
        assertTrue(
            "The terminal viewport is not dark (${Integer.toHexString(darkHalf)}); " +
                "light terminal text would be unreadable.",
            Color(darkHalf).luminance() < 0.2f
        )
    }

    /**
     * Hardcoded colours still allowed in the chrome files, each with its reason. Everything else
     * -- hex or named -- fails this test and must either migrate to `MaterialTheme.colorScheme`,
     * or to `VerbStatusColors` when it is one of the four status roles, or join this list
     * deliberately. The status palettes that used to live here are gone: they are theme-aware now.
     */
    private val allowedHexByFile: Map<String, Set<String>> = mapOf(
        "TerminalScreen.kt" to setOf(
            // The Compose fallback view that prints PTY output: terminal content, which stays dark
            // in both themes for the same reason TerminalCanvas does.
            "ffe2e8f0"
        ),
        "TerminalDiagnosticsSheet.kt" to emptySet(),
        "FileExplorerDrawer.kt" to setOf(
            // File-type category tints (folder/code/media/archive). These are category identity,
            // not state, so they are not one of the four UX_FOUNDATION status roles and do not
            // belong in VerbStatusColors.
            "ff3b82f6", "ff34d399", "ffa78bfa", "fff59e0b"
        ),
        "RunsSheet.kt" to emptySet(),
        "UsbDebuggingDiagnosticCard.kt" to emptySet()
    )

    /** Named Compose colours hardcode exactly like hex literals; none are allowed in chrome. */
    private val namedColourRegex =
        Regex("""Color\.(White|Black|Red|Green|Blue|Yellow|Gray|LightGray|DarkGray|Cyan|Magenta|Transparent)\b""")
    private val hexColourRegex = Regex("""Color\(\s*(0x[0-9A-Fa-f]{8})\s*\)""")

    @Test
    fun chromeFilesCarryOnlyDocumentedHardcodedColours() {
        val uiDir = appModuleDir().resolve("src/main/java/com/example/verb/ui")
        val violations = mutableListOf<String>()

        for ((fileName, allowed) in allowedHexByFile) {
            val source = uiDir.resolve(fileName).readText()
            source.lines().forEachIndexed { index, line ->
                hexColourRegex.findAll(line).forEach { match ->
                    val argb = match.groupValues[1].substring(2).lowercase()
                    if (argb !in allowed) {
                        violations += "$fileName:${index + 1} hardcoded hex ${match.value} " +
                            "(use MaterialTheme.colorScheme, or document it in allowedHexByFile)"
                    }
                }
                namedColourRegex.findAll(line).forEach { match ->
                    violations += "$fileName:${index + 1} hardcoded named colour ${match.value} " +
                        "(use MaterialTheme.colorScheme)"
                }
            }
        }

        assert(violations.isEmpty()) {
            "Hardcoded colours found in terminal chrome:\n" + violations.joinToString("\n")
        }
    }

    /** Works whether Gradle starts the test JVM at the repo root or inside `app/`. */
    private fun appModuleDir(): Path {
        var dir: Path? = Path.of(System.getProperty("user.dir")).toAbsolutePath()
        repeat(6) {
            val current = dir ?: return@repeat
            if (current.resolve("settings.gradle.kts").toFile().exists()) {
                return current.resolve("app")
            }
            dir = current.parent
        }
        error("Could not locate the Verb app module from ${System.getProperty("user.dir")}")
    }
}
