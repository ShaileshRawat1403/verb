package com.example.verb.terminal

import android.content.Context
import android.net.ConnectivityManager
import android.os.Build
import java.io.File
import java.io.IOException
import java.io.RandomAccessFile
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.Locale
import java.util.zip.ZipFile

/**
 * Installs the Verb CLI userland into the app-private files directory.
 *
 * The bootstrap archive ships the Verb-branded runtime built from termux-packages with the
 * package name baked in as com.aistudio.verb.app (bash, apt, dpkg, coreutils, git, and more via
 * `apt`). It is extracted flat (its `usr/` contents land directly under filesDir/usr) and then
 * made runnable:
 *
 *  - SYMLINKS.txt is processed to create the SONAME/applet symlinks (the archive itself contains
 *    none - without these, bash/apt cannot even load their shared libraries).
 *  - The statically linked [PROOT_ASSET] binary is installed so the guest can resolve the baked-in
 *    /data/data/com.aistudio.verb.app paths.
 *  - The second-stage lock file is pre-created. Termux's post-extraction second stage re-runs
 *    package postinst scripts, but its uid check relies on `id -u` output that is corrupted by
 *    the cosmetic linker warning printed on some devices; the lock lets profile.d proceed and the
 *    postinsts are run later by dpkg itself during `pkg install`.
 *
 * SHA-256 of the archive is verified after download; a mismatching archive is never extracted.
 */
object TermuxBootstrapInstaller {

    const val ABI_ARM64 = "arm64-v8a"

    const val BOOTSTRAP_FILE_NAME = "bootstrap-aarch64.zip"
    const val BOOTSTRAP_URL =
        "https://github.com/ShaileshRawat1403/verb/releases/download/verb-bootstrap-core-shell-v1/bootstrap-aarch64.zip"
    const val BOOTSTRAP_SHA256 = "282e61af1fb9040abfcc2d02f3b5b7536c0c09d4fd11146468c7dce35af7d49e"

    /** Statically linked AArch64 proot v5.3.0, extracted from an app asset. */
    const val PROOT_ASSET = "proot-arm64-v8a"

    /**
     * GNU tar 1.35-3 (termux package), dynamically linked against the userland's libacl /
     * libandroid-glob / libandroid-selinux / libiconv. The bootstrap's dependency closure only
     * ships toybox's tar applet (apt does not depend on the GNU tar package), which cannot
     * satisfy dpkg's `tar --warning=no-timestamp` invocation. This asset is installed over it
     * when needed so dpkg can unpack packages even before a rebuilt bootstrap ships GNU tar.
     */
    const val TAR_ASSET = "gnu-tar-aarch64"

    private const val MARKER_FILE = ".termux-bootstrap-complete"
    private const val ARROW = "\u2190"

    /**
     * Static login banner written over the shipped `etc/motd`. Verb publishes a package
     * registry on GitHub Pages, so `pkg`/`apt` installs are supported and advertised.
     */
    private const val VERB_MOTD = "\u001B[1mVerb CLI\u001B[0m\n" +
        "\n" +
        "An on-device Linux terminal, preinstalled with git and a bash shell.\n" +
        "\n" +
        "Run `git --version` to confirm, or `pkg install <package>` for more tools.\n" +
        "\n"

    sealed interface State {
        data object NotStarted : State
        data class Downloading(val bytesRead: Long, val totalBytes: Long) : State
        data object Extracting : State
        data object Finalizing : State
        data class Error(val message: String) : State
        data object Ready : State
    }

    fun isInstalled(context: Context): Boolean {
        val filesDir = context.filesDir
        return File(filesDir, MARKER_FILE).isFile &&
            isCompleteBootstrap(File(filesDir, "usr"))
    }

    /**
     * Android owns the active network and DNS configuration. Mirror its current DNS servers into
     * the guest instead of relying on the bootstrap's public Google DNS defaults, which can be
     * unreachable on private Wi-Fi networks.
     */
    fun ensureGuestDns(context: Context) {
        runCatching {
            val connectivity = context.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager
                ?: return
            val dnsServers = connectivity.activeNetwork
                ?.let(connectivity::getLinkProperties)
                ?.dnsServers
                ?.mapNotNull { it.hostAddress?.substringBefore('%') }
                ?.distinct()
                ?.filter { it.isNotBlank() }
                .orEmpty()
            if (dnsServers.isEmpty()) return

            val resolver = File(context.filesDir, "usr/etc/resolv.conf")
            if (resolver.parentFile?.isDirectory != true) return
            val contents = dnsServers.joinToString(separator = "", postfix = "\n") { "nameserver $it\n" }
            if (resolver.readText() != contents) resolver.writeText(contents)
        }.onFailure {
            TerminalSessionLogger.warn(LogCategory.IO, "Guest DNS sync failed: ${it.message}")
        }
    }

    /** Applies source hardening to existing installations as well as new bootstraps. */
    fun ensureSecureAptSources(filesDir: File) {
        writeAptSources(File(filesDir, "usr"))
    }

    /**
     * Runs the install. Intended to be called from a background dispatcher; state transitions are
     * reported through [onState] from the calling thread.
     */
    @Synchronized
    fun install(context: Context, onState: (State) -> Unit) {
        val filesDir = context.filesDir
        ensureGuestLinkerConfig(filesDir)
        ensureSecureAptSources(filesDir)
        if (isInstalled(context)) {
            onState(State.Ready)
            return
        }

        if (primaryAbi() != ABI_ARM64) {
            onState(State.Error("Verb CLI userland currently supports arm64-v8a devices only (found ${primaryAbi()})."))
            return
        }

        val workDir = File(filesDir, ".bootstrap").apply { mkdirs() }
        val zipFile = File(workDir, BOOTSTRAP_FILE_NAME)
        val usrDir = File(filesDir, "usr")
        val stagingUsr = File(workDir, "usr-staging")

        try {
            if (stagingUsr.exists() && !stagingUsr.deleteRecursively()) {
                throw IOException("Could not clear the previous bootstrap staging directory.")
            }
            if (zipFile.isFile && !sha256(zipFile).equals(BOOTSTRAP_SHA256, ignoreCase = true)) {
                zipFile.delete()
            }
            if (!zipFile.isFile) {
                downloadBootstrap(zipFile, onState)
            }
            if (!sha256(zipFile).equals(BOOTSTRAP_SHA256, ignoreCase = true)) {
                onState(State.Error("Downloaded bootstrap failed its SHA-256 check; please retry."))
                return
            }

            onState(State.Extracting)
            extractZip(zipFile, stagingUsr)

            onState(State.Finalizing)
            finalize(context, stagingUsr, filesDir)
            if (!isCompleteBootstrap(stagingUsr)) {
                throw IOException("Bootstrap is missing required runtime files.")
            }
            activateBootstrap(stagingUsr, usrDir, workDir)

            File(filesDir, MARKER_FILE).writeText("verb-bootstrap-core-shell-v1\n")
            onState(State.Ready)
        } catch (e: Exception) {
            stagingUsr.deleteRecursively()
            onState(State.Error(e.message ?: e.javaClass.simpleName))
        }
    }

    private fun downloadBootstrap(target: File, onState: (State) -> Unit) {
        val partial = File(target.parentFile, "${target.name}.part")
        partial.delete()
        val connection = URL(BOOTSTRAP_URL).openConnection() as HttpURLConnection
        connection.instanceFollowRedirects = true
        connection.connectTimeout = 30_000
        connection.readTimeout = 60_000
        connection.setRequestProperty("User-Agent", "verb-terminal/0.1.0")
        connection.connect()
        try {
            val total = connection.contentLengthLong
            connection.inputStream.use { input ->
                partial.outputStream().use { output ->
                    val buffer = ByteArray(64 * 1024)
                    var read: Int
                    var totalRead = 0L
                    while (input.read(buffer).also { read = it } != -1) {
                        output.write(buffer, 0, read)
                        totalRead += read
                        onState(State.Downloading(totalRead, total))
                    }
                }
            }
            if (!partial.renameTo(target)) {
                throw IOException("Could not activate the downloaded bootstrap archive.")
            }
        } finally {
            partial.delete()
            connection.disconnect()
        }
    }

    private fun extractZip(zip: File, usrDir: File) {
        if (!usrDir.mkdirs() && !usrDir.isDirectory) {
            throw IOException("Could not create bootstrap staging directory.")
        }
        val root = usrDir.canonicalFile
        val storedModes = readStoredModes(zip)
        ZipFile(zip).use { zipFile ->
            val entries = zipFile.entries()
            while (entries.hasMoreElements()) {
                val entry = entries.nextElement()
                val outFile = File(usrDir, entry.name).canonicalFile
                if (outFile != root && !outFile.path.startsWith(root.path + File.separator)) {
                    throw IOException("bootstrap archive entry escapes extraction root: ${entry.name}")
                }
                if (entry.isDirectory) {
                    outFile.mkdirs()
                } else {
                    outFile.parentFile?.mkdirs()
                    zipFile.getInputStream(entry).use { input -> outFile.outputStream().use { out -> input.copyTo(out) } }
                    applyUnixMode(outFile, storedModes[entry.name] ?: 0)
                }
            }
        }
    }

    private fun isCompleteBootstrap(usrDir: File): Boolean =
        File(usrDir, "bin/login").let { it.isFile && it.canRead() && it.canExecute() } &&
            File(usrDir, "bin/proot").let { it.isFile && it.canRead() && it.canExecute() } &&
            File(usrDir, "bin/env").isFile &&
            File(usrDir, "lib/libtermux-exec.so").isFile &&
            File(usrDir, "etc/apt/sources.list").isFile

    private fun activateBootstrap(stagingUsr: File, usrDir: File, workDir: File) {
        val backupUsr = File(workDir, "usr-previous")
        if (backupUsr.exists() && !backupUsr.deleteRecursively()) {
            throw IOException("Could not clear the previous bootstrap backup.")
        }

        val hadExisting = usrDir.exists()
        if (hadExisting && !usrDir.renameTo(backupUsr)) {
            throw IOException("Could not stage the existing bootstrap for replacement.")
        }
        try {
            if (!stagingUsr.renameTo(usrDir)) {
                throw IOException("Could not activate the new bootstrap.")
            }
            backupUsr.deleteRecursively()
        } catch (error: Exception) {
            if (usrDir.exists()) usrDir.deleteRecursively()
            if (hadExisting) backupUsr.renameTo(usrDir)
            throw error
        }
    }

    /**
     * ZipEntry-based extraction would drop the archive's unix permissions, leaving executable
     * programs (notably the apt methods under lib/apt/methods) with mode 600 and breaking
     * `pkg install`. Restore each entry's mode from the zip central directory so executables
     * stay executable and libraries stay non-executable, mirroring the real Termux install.
     */
    private fun applyUnixMode(file: File, unixMode: Int) {
        if (unixMode == 0) return
        val permissions = unixMode and 0x1FF
        file.setExecutable((permissions and 0x49) != 0, false)
        file.setReadable((permissions and 0x124) != 0, false)
        file.setWritable((permissions and 0x92) != 0, false)
    }

    /**
     * Parses the zip's end-of-central-directory and central directory entries directly (little-
     * endian) to recover each path's stored unix mode from its external attributes field. This
     * avoids ZipEntry.getExternalAttributes()/getUnixMode(), which are not exposed by the
     * platform stubs this module compiles against.
     */
    private fun readStoredModes(zip: File): Map<String, Int> {
        val modes = HashMap<String, Int>()
        RandomAccessFile(zip, "r").use { raf ->
            val fileLen = raf.length()
            val tailLen = minOf(fileLen, 65557L)
            val tail = ByteArray(tailLen.toInt())
            raf.seek(fileLen - tailLen)
            raf.readFully(tail)
            var eocdPos = -1L
            for (i in tail.size - 22 downTo 0) {
                if (tail[i].toInt() and 0xFF == 0x50 &&
                    tail[i + 1].toInt() and 0xFF == 0x4B &&
                    tail[i + 2].toInt() and 0xFF == 0x05 &&
                    tail[i + 3].toInt() and 0xFF == 0x06
                ) {
                    eocdPos = fileLen - tailLen + i
                    break
                }
            }
            if (eocdPos < 0) return modes
            var pos = readUInt32(raf, eocdPos + 16)
            val end = readUInt32(raf, eocdPos + 12) + pos
            while (pos + 46 <= end) {
                if (readUInt32(raf, pos) != 0x02014B50L) break
                val nameLen = readUInt16(raf, pos + 28)
                val extraLen = readUInt16(raf, pos + 30)
                val commentLen = readUInt16(raf, pos + 32)
                val externalAttrs = readUInt32(raf, pos + 38)
                if (nameLen > 0 && nameLen <= 1024) {
                    val nameBytes = ByteArray(nameLen)
                    raf.seek(pos + 46)
                    raf.readFully(nameBytes)
                    modes[String(nameBytes, Charsets.UTF_8)] = ((externalAttrs shr 16) and 0xFFFF).toInt()
                }
                pos += 46L + nameLen + extraLen + commentLen
            }
        }
        return modes
    }

    private fun readUInt32(raf: RandomAccessFile, offset: Long): Long {
        raf.seek(offset)
        return raf.readUnsignedByte().toLong() or
            (raf.readUnsignedByte().toLong() shl 8) or
            (raf.readUnsignedByte().toLong() shl 16) or
            (raf.readUnsignedByte().toLong() shl 24)
    }

    private fun readUInt16(raf: RandomAccessFile, offset: Long): Int {
        raf.seek(offset)
        return raf.readUnsignedByte() or (raf.readUnsignedByte() shl 8)
    }

    private fun finalize(context: Context, usrDir: File, filesDir: File) {
        processSymlinks(usrDir)
        makeExecutable(File(usrDir, "bin"))
        makeExecutable(File(usrDir, "libexec"))
        makeExecutable(File(usrDir, "lib/apt/methods"))

        installProot(context, usrDir)
        replaceMotd(usrDir)
        writeAptSources(usrDir)
        installGnuTarIfNeeded(context, usrDir)
        stripDpkgPerlClangDep(usrDir)

        val secondStageDir = File(usrDir, "etc/termux/termux-bootstrap/second-stage")
        secondStageDir.mkdirs()
        val lock = File(secondStageDir, "termux-bootstrap-second-stage.sh.lock")
        if (!lock.exists()) {
            createSymlink(lock, "termux-bootstrap-second-stage.sh")
        }

        File(filesDir, "home").mkdirs()
        File(usrDir, "tmp").mkdirs()
        File(filesDir, "cache/apt/archives/partial").mkdirs()
        File(filesDir, "var/cache/apt").mkdirs()
        File(filesDir, "var/lib/apt/lists").mkdirs()

        ensureGuestLinkerConfig(filesDir)
    }

    /**
     * Android 12+ denies lstat() on the host /linkerconfig directory, so proot cannot canonicalize
     * it and the guest linker prints "WARNING: linker: failed to find generated linker
     * configuration" on every command. Copy the app's own linker config (readable by the app) into
     * filesDir/linkerconfig so the resolver can bind it over /linkerconfig. Best-effort: a failure
     * only means the harmless warnings keep appearing.
     */
    /**
     * Android 12+ denies lstat() on the host /linkerconfig directory, so proot cannot canonicalize
     * it and the guest linker prints "WARNING: linker: failed to find generated linker
     * configuration" on every command. Copy the app's own linker config (readable by the app) into
     * filesDir/linkerconfig so the resolver can bind it over /linkerconfig. Best-effort: a failure
     * only means the harmless warnings keep appearing.
     */
    fun ensureGuestLinkerConfig(filesDir: File) {
        runCatching {
            val source = File("/linkerconfig/ld.config.txt")
            if (!source.exists()) return
            val target = File(filesDir, "linkerconfig/ld.config.txt")
            if (target.isFile && target.length() == source.length()) return
            target.parentFile?.mkdirs()
            source.inputStream().use { input -> target.outputStream().use { out -> input.copyTo(out) } }
        }.onFailure {
            TerminalSessionLogger.warn(LogCategory.IO, "Guest linker config copy failed: ${it.message}")
        }
    }

    private fun installProot(context: Context, usrDir: File) {
        val proot = File(usrDir, "bin/proot")
        context.assets.open(PROOT_ASSET).use { input ->
            proot.outputStream().use { out -> input.copyTo(out) }
        }
        if (!BundledToolBootstrap.isValidElf(proot) || proot.length() < 1_000_000L) {
            proot.delete()
            throw IOException("Bundled proot asset is invalid or corrupt.")
        }
        // outputStream() creates the file without read bits on Android. execve() needs the
        // executable image to remain readable from the app domain, not merely executable.
        proot.setReadable(true, false)
        proot.setExecutable(true, false)
    }

    /**
     * Replaces a toybox `tar` with the bundled GNU tar so dpkg can unpack packages (dpkg passes
     * GNU-only `--warning=no-timestamp` to tar). No-op once GNU tar is present, including on
     * bootstraps that already ship the termux tar package. The asset is a dynamically linked
     * binary, so the check and the installed copy run against the userland's own libraries.
     */
    private fun installGnuTarIfNeeded(context: Context, usrDir: File) {
        val tar = File(usrDir, "bin/tar")
        if (isGnuTar(tar, usrDir)) return
        context.assets.open(TAR_ASSET).use { input ->
            tar.outputStream().use { out -> input.copyTo(out) }
        }
        tar.setExecutable(true, false)
        if (!isGnuTar(tar, usrDir)) {
            tar.delete()
            TerminalSessionLogger.warn(LogCategory.IO, "Bundled GNU tar asset failed to run; keeping existing tar.")
        }
    }

    private fun isGnuTar(tar: File, usrDir: File): Boolean {
        if (!tar.isFile) return false
        return try {
            val builder = ProcessBuilder(tar.absolutePath, "--version")
                .redirectErrorStream(true)
            builder.environment()["LD_LIBRARY_PATH"] = File(usrDir, "lib").absolutePath
            val process = builder.start()
            val output = process.inputStream.bufferedReader().use { it.readText() }
            process.waitFor()
            output.startsWith("tar (GNU tar)")
        } catch (ignored: Exception) {
            false
        }
    }

    /**
     * The bootstrap's `dpkg-perl` package declares `Depends: clang`. clang and its llvm toolchain
     * are not part of the minimal userland, so apt refuses every install on a fresh bootstrap
     * until `apt --fix-broken install` pulls the whole toolchain. Nothing in the runtime needs
     * dpkg-perl's C compiler (only `dpkg-scanpackages` reverse-depends on it), so the clang
     * dependency is removed from its status stanza. Future bootstraps exclude dpkg-perl itself.
     */
    private fun stripDpkgPerlClangDep(usrDir: File) {
        val status = File(usrDir, "var/lib/dpkg/status")
        if (!status.isFile) return
        val stanzas = status.readText().split("\n\n")
        var changed = false
        val updated = stanzas.map { stanza ->
            if (stanza.lines().firstOrNull() != "Package: dpkg-perl") return@map stanza
            stanza.lines().map { line ->
                if (!line.startsWith("Depends: ")) return@map line
                val deps = line.removePrefix("Depends: ").split(", ")
                val kept = deps.filterNot { it == "clang" || it.startsWith("clang ") }
                if (kept.size != deps.size) changed = true
                "Depends: " + kept.joinToString(", ")
            }.joinToString("\n")
        }
        if (changed) status.writeText(updated.joinToString("\n\n"))
    }

    /**
     * Replaces the shipped Termux welcome banner (usr/etc/motd) with the Verb-branded one so the
     * userland presents as Verb CLI on every login.
     */
    private fun replaceMotd(usrDir: File) {
        File(usrDir, "etc/motd").writeText(VERB_MOTD)
    }

    /**
     * Removes the former unsigned Verb package registry. Public builds must not opt out of apt
     * signature verification; the signed Termux repository remains in the bootstrap sources.
     */
    private fun writeAptSources(usrDir: File) {
        val sources = File(usrDir, "etc/apt/sources.list")
        if (!sources.isFile) return
        val lines = sources.readLines()
        val trustedVerbLines = lines.filterNot { it.contains("github.io/verb/apt") }
        if (trustedVerbLines != lines) sources.writeText(trustedVerbLines.joinToString("\n") + "\n")
    }

    /**
     * Every line is "TARGET←LINK" and creates a relative (or, for the termux-keyring entries,
     * absolute) symlink. Absolute targets only resolve inside the proot guest, which is where they
     * are used (apt trusted.gpg.d). Empty archive directories that would otherwise break a link's
     * parent are created up front.
     */
    private fun processSymlinks(usrDir: File) {
        val symlinks = File(usrDir, "SYMLINKS.txt")
        if (!symlinks.isFile) return
        symlinks.readLines().forEach { line ->
            val parts = line.split(ARROW)
            if (parts.size != 2) return@forEach
            val target = parts[0].trim()
            val linkRel = parts[1].trim().removePrefix("./")
            if (target.isEmpty() || linkRel.isEmpty()) return@forEach
            val link = File(usrDir, linkRel)
            link.parentFile?.mkdirs()
            if (!link.exists()) {
                createSymlink(link, target)
            }
        }
    }

    /**
     * Creates a symlink via the system ln binary (java.nio.file is unavailable below API 26).
     * A missing or broken symlink is tolerated; the underlying binary still runs.
     */
    private fun createSymlink(link: File, target: String) {
        try {
            val process = ProcessBuilder("/system/bin/ln", "-s", target, link.absolutePath)
                .redirectErrorStream(true)
                .start()
            process.waitFor()
        } catch (ignored: Exception) {
            // Nothing fatal: a missing link only degrades the linked alias, not the binary.
        }
    }

    private fun makeExecutable(dir: File) {
        dir.listFiles()?.forEach { f ->
            if (f.isDirectory) {
                f.setExecutable(true, false)
                makeExecutable(f)
            } else {
                f.setReadable(true, false)
                f.setExecutable(true, false)
            }
        }
    }

    private fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buffer = ByteArray(64 * 1024)
            var read: Int
            while (input.read(buffer).also { read = it } != -1) {
                digest.update(buffer, 0, read)
            }
        }
        return digest.digest().joinToString("") { String.format(Locale.US, "%02x", it) }
    }

    private fun primaryAbi(): String =
        Build.SUPPORTED_ABIS.firstOrNull { it == ABI_ARM64 || it == "x86_64" } ?: ABI_ARM64
}
