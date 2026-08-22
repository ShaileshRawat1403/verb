package com.example.verb.terminal

import android.content.Context
import java.io.File

/**
 * Installs Verb's own command into the guest, alongside the agent wrappers.
 *
 * `verb` is how a person saves and restores their working world -- the agent logins, `$HOME/.env`
 * and Verb's session records -- which otherwise live only in app-private storage and die with an
 * uninstall. See `docs/BACKLOG.md`, "Mobile reliability".
 *
 * It lives in [AgentWrapperBootstrap.RELATIVE_BIN_DIR] because that directory is Verb-owned and
 * first on PATH, so nothing a package manager installs can take the name. The script itself is an
 * asset rather than a string in Kotlin: it is long, it is shell, and it is far easier to read and
 * test as a file.
 */
object VerbCliBootstrap {

    /** The command's name in the guest, and the file name in the Verb-owned bin directory. */
    const val COMMAND = "verb"

    private const val ASSET_PATH = "verb/world.sh"

    /**
     * Writes the command if it is missing or out of date. Safe to call on every launch, and cheap:
     * it compares before writing, so an unchanged script costs one read.
     */
    fun install(context: Context, filesDir: File): Boolean = runCatching {
        val binDir = File(filesDir, AgentWrapperBootstrap.RELATIVE_BIN_DIR)
        if (!binDir.isDirectory && !binDir.mkdirs()) {
            TerminalSessionLogger.warn(LogCategory.IO, "Verb CLI directory could not be created")
            return false
        }

        val script = context.assets.open(ASSET_PATH).use { it.readBytes().toString(Charsets.UTF_8) }
        val target = File(binDir, COMMAND)
        if (!target.isFile || target.readText() != script) {
            target.writeText(script)
        }
        target.setExecutable(true, false)
        true
    }.getOrElse { error ->
        TerminalSessionLogger.warn(LogCategory.IO, "Verb CLI install failed: ${error.message}")
        false
    }
}
