package com.example.verb.terminal

import android.content.Context
import java.io.File

/**
 * Installs the musl dynamic loader so agent CLIs published as musl builds can run on-device.
 *
 * Several of the tools users actually want -- Claude Code, OpenCode, anything Bun-compiled -- ship
 * `linux-arm64-musl` binaries but no Android build. Those binaries are perfectly capable of running
 * here; they are aarch64 ELFs whose only unmet dependency is their interpreter,
 * `/lib/ld-musl-aarch64.so.1`, which Android does not provide. Without it the kernel refuses the
 * exec with `ENOENT`, which surfaces to the user as the deeply unhelpful "No such file or
 * directory" naming a file that plainly exists.
 *
 * Supplying that one file is the entire fix. [TerminalEnvironmentResolver] binds it at the absolute
 * path the binaries hard-code, so nothing has to be patched or rebuilt.
 *
 * The loader is a bundled asset rather than a download: it is then deterministic, offline, and
 * versioned with the app, with no install-time dependency on a third-party CDN. It is validated as
 * a 64-bit aarch64 ELF before being made executable, so a corrupt asset can never be handed to the
 * kernel as an interpreter. Sourced from Alpine's `musl` package (MIT; see THIRD_PARTY_LICENSES.md).
 */
object MuslLoaderBootstrap {

    /** The path musl-linked binaries hard-code as their interpreter. */
    const val GUEST_LOADER_PATH = "/lib/ld-musl-aarch64.so.1"

    /** Where the loader is installed inside Verb's own prefix. */
    const val RELATIVE_INSTALL_PATH = "usr/lib/ld-musl-aarch64.so.1"

    private const val ASSET_NAME = "ld-musl-aarch64.so.1"

    /** Where the musl-side shared libraries live inside Verb's prefix. */
    const val RELATIVE_LIB_DIR = "usr/lib/musl"

    /**
     * The C++ runtime some musl agent builds link against. OpenCode needs both; without them it
     * fails with `Error loading shared library libstdc++.so.6`. Bionic's own C++ runtime is not
     * ABI-compatible, so the musl builds are installed into a directory of their own and reached
     * only through the generated wrapper's `LD_LIBRARY_PATH`, never Verb's normal library path.
     */
    private val LIB_ASSETS = mapOf(
        "libstdc++.so.6.0.34" to "libstdc++.so.6",
        "libgcc_s.so.1" to null
    )

    /**
     * Copies the loader into the prefix if it is missing or invalid. Safe to call on every launch:
     * an already-valid loader is left untouched.
     *
     * @return the installed loader, or null when it could not be installed -- in which case musl
     *   binaries simply stay unavailable rather than failing in some partial state.
     */
    fun install(context: Context): File? = runCatching {
        val target = File(context.filesDir, RELATIVE_INSTALL_PATH)
        if (target.isFile && target.canExecute() && BundledToolBootstrap.isValidElf(target)) return target

        target.parentFile?.mkdirs()
        val staged = File(target.parentFile, "$ASSET_NAME.staged")
        context.assets.open(ASSET_NAME).use { input ->
            staged.outputStream().use { output -> input.copyTo(output) }
        }
        // Validate before it can ever be used as an interpreter, then move into place.
        if (!BundledToolBootstrap.isValidElf(staged)) {
            staged.delete()
            TerminalSessionLogger.warn(LogCategory.IO, "Bundled musl loader failed ELF validation")
            return null
        }
        staged.setExecutable(true, false)
        staged.setReadable(true, false)
        if (target.exists()) target.delete()
        if (!staged.renameTo(target)) {
            staged.delete()
            return null
        }
        installSupportLibraries(context)
        TerminalSessionLogger.info(LogCategory.IO, "musl loader installed for musl-built agent CLIs")
        target
    }.onFailure {
        TerminalSessionLogger.warn(LogCategory.IO, "musl loader install failed: ${it.message}")
    }.getOrNull()

    /**
     * Installs the musl C++ runtime beside the loader, with the `.so.6` soname link the binaries
     * actually request. Best-effort: a musl agent that does not need C++ still starts without it.
     */
    private fun installSupportLibraries(context: Context) {
        val libDir = File(context.filesDir, RELATIVE_LIB_DIR).apply { mkdirs() }
        for ((asset, soname) in LIB_ASSETS) {
            runCatching {
                val target = File(libDir, asset)
                if (!target.isFile) {
                    context.assets.open(asset).use { input ->
                        target.outputStream().use { output -> input.copyTo(output) }
                    }
                    target.setReadable(true, false)
                }
                if (soname != null) {
                    val link = File(libDir, soname)
                    if (!link.exists()) {
                        runCatching { android.system.Os.symlink(asset, link.absolutePath) }
                            .onFailure { target.copyTo(link, overwrite = true) }
                    }
                }
            }.onFailure {
                TerminalSessionLogger.warn(LogCategory.IO, "musl support library $asset failed: ${it.message}")
            }
        }
    }

    /** True when a valid loader is present, so musl binaries can be expected to start. */
    fun isInstalled(filesDir: File): Boolean =
        File(filesDir, RELATIVE_INSTALL_PATH).let { it.isFile && it.canExecute() }
}
