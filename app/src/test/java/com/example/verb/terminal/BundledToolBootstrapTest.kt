package com.example.verb.terminal

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class BundledToolBootstrapTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private fun writeElfHeader(file: File, machine: Int) {
        val header = ByteArray(20)
        header[0] = 0x7F
        header[1] = 'E'.code.toByte()
        header[2] = 'L'.code.toByte()
        header[3] = 'F'.code.toByte()
        header[4] = 2 // ELFCLASS64
        header[18] = (machine and 0xFF).toByte()
        header[19] = ((machine shr 8) and 0xFF).toByte()
        file.writeBytes(header)
    }

    @Test
    fun `valid aarch64 ELF header is accepted`() {
        val file = temporaryFolder.newFile("busybox")
        writeElfHeader(file, 183) // e_machine = EM_AARCH64
        assertTrue(BundledToolBootstrap.isValidElf(file))
    }

    @Test
    fun `valid x86_64 ELF header is accepted`() {
        val file = temporaryFolder.newFile("jq")
        writeElfHeader(file, 62) // e_machine = EM_X86_64
        assertTrue(BundledToolBootstrap.isValidElf(file))
    }

    @Test
    fun `text file is rejected`() {
        val file = temporaryFolder.newFile("curl")
        file.writeText("Not Found")
        assertFalse(BundledToolBootstrap.isValidElf(file))
    }

    @Test
    fun `truncated file is rejected`() {
        val file = temporaryFolder.newFile("short")
        file.writeBytes(byteArrayOf(0x7F, 'E'.code.toByte(), 'L'.code.toByte()))
        assertFalse(BundledToolBootstrap.isValidElf(file))
    }

    @Test
    fun `file without ELF magic is rejected`() {
        val file = temporaryFolder.newFile("garbage")
        writeElfHeader(file, 183)
        file.writeBytes(ByteArray(20) { 0x41 })
        assertFalse(BundledToolBootstrap.isValidElf(file))
    }

    @Test
    fun `32-bit ELF header is rejected`() {
        val file = temporaryFolder.newFile("i386-tool")
        val header = ByteArray(20)
        header[0] = 0x7F
        header[1] = 'E'.code.toByte()
        header[2] = 'L'.code.toByte()
        header[3] = 'F'.code.toByte()
        header[4] = 1 // ELFCLASS32
        header[18] = 3 // EM_386
        file.writeBytes(header)
        assertFalse(BundledToolBootstrap.isValidElf(file))
    }
}
