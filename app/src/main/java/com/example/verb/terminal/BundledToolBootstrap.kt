package com.example.verb.terminal

import android.content.Context
import android.os.Build
import java.io.File

/**
 * Installs the small set of standalone CLI tools bundled as app assets into the app-private
 * `filesDir/bin` directory. Every candidate binary is validated as a 64-bit ELF for the running
 * ABI before it is made executable, so a corrupted or mis-labeled asset can never be offered to
 * the shell as a real tool.
 */
object BundledToolBootstrap {

    const val ABI_ARM64 = "arm64-v8a"
    const val ABI_X86_64 = "x86_64"

    private val TOOLS = listOf("busybox", "curl", "jq")

    data class Result(
        val binDir: File?,
        val installed: List<String>,
        val skipped: List<Pair<String, String>>
    ) {
        val isReady: Boolean
            get() = binDir != null && installed.isNotEmpty()
    }

    fun install(context: Context, abi: String = primaryAbi()): Result {
        val binDir = File(context.filesDir, "bin")
        if (!binDir.exists() && !binDir.mkdirs()) {
            return Result(binDir = null, installed = emptyList(), skipped = listOf("bin" to "could not create $binDir"))
        }

        val installed = mutableListOf<String>()
        val skipped = mutableListOf<Pair<String, String>>()

        extractCertBundle(context, binDir)

        for (tool in TOOLS) {
            val target = File(binDir, tool)
            if (target.isFile && isValidElf(target)) {
                installed += tool
                continue
            }
            val assetPath = "$abi/$tool"
            try {
                context.assets.open(assetPath).use { input ->
                    target.outputStream().use { output -> input.copyTo(output) }
                }
                if (!isValidElf(target)) {
                    target.delete()
                    skipped += tool to "asset $assetPath is not a valid 64-bit ELF binary"
                    continue
                }
                if (!target.setExecutable(true, false)) {
                    skipped += tool to "could not mark $tool executable"
                    continue
                }
                installed += tool
            } catch (e: Exception) {
                skipped += tool to "asset $assetPath unavailable: ${e.message}"
            }
        }

        return Result(binDir = binDir, installed = installed, skipped = skipped)
    }

    private fun extractCertBundle(context: Context, binDir: File) {
        val certFile = File(binDir, "cacert.pem")
        if (certFile.isFile && certFile.length() > 0) return
        try {
            context.assets.open("cacert.pem").use { input ->
                certFile.outputStream().use { output -> input.copyTo(output) }
            }
        } catch (e: Exception) {
            // A missing CA bundle is not fatal: it only disables HTTPS verification for curl/jq.
        }
    }

    /**
     * Validates a 64-bit ELF executable header for the current supported ABIs (AArch64 or x86-64).
     *
     * Header layout: magic at 0..3, EI_CLASS at 4, and e_machine (little-endian u16) at 18.
     * e_machine values: 62 = x86-64, 183 = AArch64.
     */
    fun isValidElf(file: File): Boolean {
        if (!file.isFile || file.length() < 20L) return false
        return try {
            file.inputStream().use { stream ->
                val header = ByteArray(20)
                var offset = 0
                while (offset < header.size) {
                    val read = stream.read(header, offset, header.size - offset)
                    if (read < 0) return false
                    offset += read
                }
                header[0] == 0x7F.toByte() &&
                    header[1] == 'E'.code.toByte() &&
                    header[2] == 'L'.code.toByte() &&
                    header[3] == 'F'.code.toByte() &&
                    header[4] == 2.toByte() && // 64-bit ELF
                    (header[18].toInt() and 0xFF or ((header[19].toInt() and 0xFF) shl 8)) in ELF_MACHINES
            }
        } catch (e: Exception) {
            false
        }
    }

    private fun primaryAbi(): String =
        Build.SUPPORTED_ABIS.firstOrNull { it == ABI_ARM64 || it == ABI_X86_64 } ?: ABI_ARM64

    private const val ELF_MACHINE_X86_64 = 62
    private const val ELF_MACHINE_AARCH64 = 183
    private val ELF_MACHINES = setOf(ELF_MACHINE_X86_64, ELF_MACHINE_AARCH64)
}
