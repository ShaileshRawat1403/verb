package com.example.verb.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.Audiotrack
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material.icons.filled.Code
import androidx.compose.material.icons.filled.Description
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.MoreHoriz
import androidx.compose.material.icons.filled.Rocket
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material.icons.filled.Videocam
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.example.verb.terminal.TerminalRuntimeAdapter
import com.example.verb.terminal.VerbGuestPaths
import java.io.File
import java.util.Locale

private val DirBlue = Color(0xFF3B82F6)
private val CodeGreen = Color(0xFF34D399)
private val MediaPurple = Color(0xFFA78BFA)
private val ArchiveAmber = Color(0xFFF59E0B)

/**
 * Filesystem browser reachable from the terminal header. Navigates the host filesystem but
 * renders guest-facing paths (e.g. `$PREFIX`, `~`) whenever the current directory sits inside the
 * Verb userland so paths match what the shell would print.
 */
@Composable
fun FileExplorerDrawer(
    terminalRuntime: TerminalRuntimeAdapter?,
    onFileClicked: (String) -> Unit
) {
    val launchDir = terminalRuntime?.launchWorkingDirectory

    // A SNAPSHOT, deliberately not a subscription. The browser opens wherever the shell currently
    // is, and from then on the user owns navigation: a later `cd` in the terminal must not yank an
    // open browser out from under someone who has navigated somewhere else. This composable is
    // mounted only while the sheet is open (see TerminalScreen), so closing and reopening it
    // re-runs this `remember` and takes a fresh snapshot -- which is the intended way to resync.
    //
    // Only a MAPPED host path is ever used. A guest path Verb could not translate through a known
    // bind (`/system`, `/etc`, ...) has a null hostPath and falls back to the launch directory --
    // never a `File(guestPath)`, and never `/`.
    val initialDir = remember(terminalRuntime) {
        terminalRuntime?.currentWorkingDirectory?.value?.hostPath ?: launchDir
    }

    // The terminal starts in filesDir/home, while Prefix lives beside home at filesDir/usr. Derived
    // from the launch directory, which is stable for the session, so the $PREFIX / ~ labels below
    // do not move around as the user browses.
    val appFilesDir = launchDir?.takeIf { it.name == "home" }?.parentFile ?: launchDir
    val prefixDir = appFilesDir?.let { File(it, "usr") }
    val homeDir = appFilesDir?.let { File(it, "home") }

    // File("/") only when there is no runtime at all (preview/headless). It is never used as a
    // fallback for a guest path that failed to map.
    var currentDir by remember(terminalRuntime) { mutableStateOf(initialDir ?: File("/")) }
    var files by remember { mutableStateOf(emptyList<File>()) }
    var error by remember { mutableStateOf<String?>(null) }
    var copiedPath by remember { mutableStateOf<String?>(null) }

    val textPrimary = MaterialTheme.colorScheme.onSurface
    val textSecondary = MaterialTheme.colorScheme.onSurfaceVariant
    val headerBg = MaterialTheme.colorScheme.surface
    val rowHoverBg = MaterialTheme.colorScheme.surfaceVariant
    val chipBg = MaterialTheme.colorScheme.surfaceVariant

    LaunchedEffect(currentDir) {
        val list = currentDir.listFiles()
        if (list == null) {
            error = if (!currentDir.exists()) "This path does not exist." else "Permission denied — cannot read this folder."
            files = emptyList()
        } else {
            error = null
            files = list.toList().sortedWith(compareBy({ !it.isDirectory }, { it.name.lowercase(Locale.US) }))
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        // Breadcrumb header: scrollable path segments, each tappable.
        Surface(color = headerBg, modifier = Modifier.fillMaxWidth()) {
            Column(modifier = Modifier.padding(vertical = 8.dp)) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 8.dp)
                ) {
                    IconButton(
                        onClick = { currentDir.parentFile?.let { currentDir = it } },
                        enabled = currentDir.parentFile != null,
                        modifier = Modifier.size(36.dp)
                    ) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Up one level",
                            tint = if (currentDir.parentFile != null) textPrimary else textSecondary
                        )
                    }
                    Text(
                        text = displayPath(currentDir, prefixDir, homeDir),
                        fontFamily = FontFamily.Monospace,
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Bold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        color = textPrimary,
                        modifier = Modifier
                            .weight(1f, fill = false)
                            .testTag("file_explorer_current_path")
                    )
                }

                // Tappable breadcrumbs.
                val segments = buildList {
                    var cursor: File? = currentDir
                    while (cursor != null && cursor.parentFile != null) {
                        add(0, cursor)
                        cursor = cursor.parentFile
                    }
                    // Ensure "/" is the root segment when the host path sits elsewhere.
                    val root = File("/")
                    if (isEmpty() || first().absolutePath != root.absolutePath) {
                        add(0, root)
                    }
                }
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .padding(start = 8.dp, end = 8.dp, bottom = 2.dp)
                ) {
                    segments.forEachIndexed { index, segment ->
                        val isLast = index == segments.lastIndex
                        Text(
                            text = if (segment.absolutePath == "/") "/" else segment.name,
                            fontSize = 12.sp,
                            color = if (isLast) textPrimary else textSecondary,
                            fontWeight = if (isLast) FontWeight.SemiBold else FontWeight.Normal,
                            modifier = Modifier
                                .clickable(enabled = !isLast) { currentDir = segment }
                                .padding(vertical = 2.dp, horizontal = 3.dp)
                        )
                        if (!isLast) {
                            Icon(
                                imageVector = Icons.Default.ChevronRight,
                                contentDescription = null,
                                tint = textSecondary,
                                modifier = Modifier.size(12.dp)
                            )
                        }
                    }
                }
            }
        }

        // Quick-access shortcuts.
        Row(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 12.dp, vertical = 8.dp)
        ) {
            // "Shell" resolves the live directory at TAP time, not at composition time. That keeps
            // it a truthful manual resync -- the browser follows the shell only when the user asks
            // it to -- and it still refuses an unmapped guest path, falling back to the launch
            // directory rather than navigating somewhere the guest path does not name on the host.
            listOf<Triple<ImageVector, String, () -> File?>>(
                Triple(Icons.Default.Home, "Home") { homeDir },
                Triple(Icons.Default.Rocket, "Prefix") { prefixDir },
                Triple(Icons.Default.StarBorder, "Root") { File("/") },
                Triple(Icons.Default.Terminal, "Shell") {
                    terminalRuntime?.currentWorkingDirectory?.value?.hostPath ?: launchDir
                }
            ).forEach { (icon, label, resolveTarget) ->
                Surface(
                    shape = RoundedCornerShape(999.dp),
                    color = chipBg,
                    onClick = { resolveTarget()?.let { currentDir = it } }
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.padding(horizontal = 10.dp, vertical = 5.dp)
                    ) {
                        Icon(
                            imageVector = icon,
                            contentDescription = null,
                            tint = DirBlue,
                            modifier = Modifier.size(14.dp)
                        )
                        Spacer(modifier = Modifier.width(4.dp))
                        Text(
                            text = label,
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Medium,
                            color = textPrimary
                        )
                    }
                }
            }
        }

        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)

        copiedPath?.let { path ->
            Text(
                text = "Terminal path copied: $path",
                fontSize = 12.sp,
                color = CodeGreen,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
            )
        }

        when {
            error != null -> EmptyState(message = error!!, icon = Icons.Default.MoreHoriz, textColor = textSecondary)
            files.isEmpty() -> EmptyState(
                message = "This folder is empty.",
                icon = Icons.Default.FolderOpen,
                textColor = textSecondary
            )
            else -> LazyColumn(modifier = Modifier.fillMaxSize()) {
                items(files, key = { it.absolutePath }) { file ->
                    FileRow(
                        file = file,
                        textPrimary = textPrimary,
                        textSecondary = textSecondary,
                        hoverBg = rowHoverBg,
                        onClick = {
                            if (file.isDirectory) {
                                currentDir = file
                            } else {
                                val path = terminalPath(file, prefixDir, homeDir)
                                copiedPath = path
                                onFileClicked(path)
                            }
                        }
                    )
                }
            }
        }
    }
}

@Composable
private fun FileRow(
    file: File,
    textPrimary: Color,
    textSecondary: Color,
    hoverBg: Color,
    onClick: () -> Unit
) {
    val isDir = file.isDirectory
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 10.dp)
    ) {
        Icon(
            imageVector = fileIcon(file, isDir),
            contentDescription = null,
            tint = if (isDir) DirBlue else fileTint(file),
            modifier = Modifier.size(22.dp)
        )
        Spacer(modifier = Modifier.width(14.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = file.name,
                fontSize = 14.sp,
                fontWeight = if (isDir) FontWeight.SemiBold else FontWeight.Normal,
                color = textPrimary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            if (!isDir) {
                Text(
                    text = "${formatSize(file.length())} · ${formatDate(file.lastModified())}",
                    fontSize = 11.sp,
                    color = textSecondary
                )
            }
        }
        if (isDir) {
            Icon(
                imageVector = Icons.Default.ChevronRight,
                contentDescription = null,
                tint = textSecondary,
                modifier = Modifier.size(16.dp)
            )
        }
    }
    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
}

@Composable
private fun EmptyState(message: String, icon: ImageVector, textColor: Color) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 48.dp)
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = textColor.copy(alpha = 0.6f),
            modifier = Modifier.size(36.dp)
        )
        Spacer(modifier = Modifier.width(0.dp))
        Text(
            text = message,
            fontSize = 13.sp,
            color = textColor,
            modifier = Modifier.padding(top = 10.dp)
        )
    }
}

private fun fileIcon(file: File, isDir: Boolean): ImageVector = when {
    isDir -> Icons.Default.Folder
    else -> when (file.extension.lowercase(Locale.US)) {
        "jpg", "jpeg", "png", "gif", "webp", "heic", "svg", "bmp" -> Icons.Default.Image
        "mp3", "wav", "ogg", "flac", "m4a", "aac" -> Icons.Default.Audiotrack
        "mp4", "mov", "mkv", "webm", "avi" -> Icons.Default.Videocam
        "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "deb", "tgz" -> Icons.Default.Archive
        "kt", "java", "py", "sh", "rs", "c", "cpp", "h", "hpp", "js", "ts", "json",
        "yaml", "yml", "toml", "xml", "html", "css", "go", "swift", "gradle", "kts" -> Icons.Default.Code
        else -> Icons.Default.Description
    }
}

private fun fileTint(file: File): Color = when (file.extension.lowercase(Locale.US)) {
    "jpg", "jpeg", "png", "gif", "webp", "heic", "svg", "bmp",
    "mp3", "wav", "ogg", "flac", "m4a", "aac",
    "mp4", "mov", "mkv", "webm", "avi" -> MediaPurple
    "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "deb", "tgz" -> ArchiveAmber
    "kt", "java", "py", "sh", "rs", "c", "cpp", "h", "hpp", "js", "ts", "json",
    "yaml", "yml", "toml", "xml", "html", "css", "go", "swift", "gradle", "kts" -> CodeGreen
    else -> Color(0xFF94A3B8)
}

private fun displayPath(file: File, prefixDir: File?, homeDir: File?): String {
    val absolute = file.absolutePath
    if (homeDir != null && absolute.startsWith(homeDir.absolutePath)) {
        return if (absolute == homeDir.absolutePath) "~" else "~" + absolute.removePrefix(homeDir.absolutePath)
    }
    if (prefixDir != null && absolute.startsWith(prefixDir.absolutePath)) {
        return if (absolute == prefixDir.absolutePath) {
            "\$PREFIX"
        } else {
            "\$PREFIX" + absolute.removePrefix(prefixDir.absolutePath)
        }
    }
    return absolute
}

private fun terminalPath(file: File, prefixDir: File?, homeDir: File?): String {
    val absolute = file.absolutePath
    if (homeDir != null && absolute.startsWith(homeDir.absolutePath)) {
        return VerbGuestPaths.HOME + absolute.removePrefix(homeDir.absolutePath)
    }
    if (prefixDir != null && absolute.startsWith(prefixDir.absolutePath)) {
        return VerbGuestPaths.PREFIX + absolute.removePrefix(prefixDir.absolutePath)
    }
    return absolute
}

private fun formatSize(bytes: Long): String {
    if (bytes < 1024) return "$bytes B"
    val kb = bytes / 1024.0
    if (kb < 1024) return String.format(Locale.US, "%.1f KB", kb)
    val mb = kb / 1024.0
    if (mb < 1024) return String.format(Locale.US, "%.1f MB", mb)
    return String.format(Locale.US, "%.1f GB", mb / 1024.0)
}

private fun formatDate(epochMillis: Long): String {
    val formatter = java.text.SimpleDateFormat("MMM d, HH:mm", Locale.US)
    return formatter.format(java.util.Date(epochMillis))
}
