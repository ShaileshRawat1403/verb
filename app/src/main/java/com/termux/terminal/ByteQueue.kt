package com.termux.terminal

/**
 * Thread-safe byte circular queue for buffering TTY stream chunks.
 * Copyright (C) Termux team.
 */
class ByteQueue(private val bufferSize: Int) {
    private val buffer = ByteArray(bufferSize)
    private var head = 0
    private var stored = 0
    private var open = true

    @Synchronized
    fun close() {
        open = false
        (this as java.lang.Object).notifyAll()
    }

    @Synchronized
    fun read(outBuffer: ByteArray, block: Boolean): Int {
        while (stored == 0) {
            if (!open) return -1
            if (!block) return 0
            try {
                (this as java.lang.Object).wait()
            } catch (e: InterruptedException) {
                return -1
            }
        }

        val bytesToRead = Math.min(stored, outBuffer.size)
        val firstChunk = Math.min(bytesToRead, bufferSize - head)
        System.arraycopy(buffer, head, outBuffer, 0, firstChunk)
        if (bytesToRead > firstChunk) {
            System.arraycopy(buffer, 0, outBuffer, firstChunk, bytesToRead - firstChunk)
        }

        head = (head + bytesToRead) % bufferSize
        stored -= bytesToRead
        (this as java.lang.Object).notifyAll()
        return bytesToRead
    }

    @Synchronized
    fun write(data: ByteArray, offset: Int, length: Int): Boolean {
        var bytesToWrite = length
        var currentOffset = offset
        while (bytesToWrite > 0) {
            while (stored == bufferSize) {
                if (!open) return false
                try {
                    (this as java.lang.Object).wait()
                } catch (e: InterruptedException) {
                    return false
                }
            }

            val tail = (head + stored) % bufferSize
            val spaceAtEnd = Math.min(bytesToWrite, bufferSize - tail)
            val chunk = Math.min(spaceAtEnd, bufferSize - stored)
            System.arraycopy(data, currentOffset, buffer, tail, chunk)

            stored += chunk
            bytesToWrite -= chunk
            currentOffset += chunk
            (this as java.lang.Object).notifyAll()
        }
        return true
    }
}
