package com.spizganed.quickbuds.protocol.baseus

/** Baseus Intelligent's bare BA request / AA response framing used by Bowie 30. */
data class BaseusFrame(val command: Int, val payload: ByteArray) {
    fun u8(index: Int): Int = payload[index].toInt() and 0xFF
}

object BaseusBowie30Codec {
    private const val MAX_BUFFER = 512

    fun request(command: Int, vararg payload: Int): ByteArray {
        require(command in 0..0xFF && payload.all { it in 0..0xFF })
        return byteArrayOf(0xBA.toByte(), command.toByte()) + payload.map { it.toByte() }.toByteArray()
    }

    /** Incremental decoder for BLE notifications, which may be split or coalesced. */
    class Decoder {
        private var buffer = ByteArray(0)

        fun reset() { buffer = ByteArray(0) }

        fun feed(input: ByteArray): List<BaseusFrame> {
            if (input.isEmpty()) return emptyList()
            if (input.size > MAX_BUFFER) { buffer = ByteArray(0); return emptyList() }
            if (buffer.size + input.size > MAX_BUFFER) buffer = ByteArray(0)
            buffer += input
            val frames = mutableListOf<BaseusFrame>()
            while (true) {
                val start = buffer.indexOf(0xAA.toByte())
                if (start < 0) { buffer = ByteArray(0); break }
                if (start > 0) buffer = buffer.copyOfRange(start, buffer.size)
                if (buffer.size < 2) break
                val size = when (buffer[1].toInt() and 0xFF) {
                    0x02, 0x33 -> 6
                    0x21 -> 5
                    0x22 -> 3
                    0x34, 0x35 -> 3
                    // Baseus Intelligent legacy gesture protocol: AA21 carries
                    // layout + left/right action; AA22 carries one result byte.
                    0x21 -> 5
                    0x22 -> 3
                    0x8B, 0x8D, 0x8F -> 3
                    0x8C -> 11
                    0x8E -> {
                        if (buffer.size < 6) break
                        6 + (buffer[5].toInt() and 0xFF)
                    }
                    else -> { buffer = buffer.copyOfRange(1, buffer.size); continue }
                }
                if (buffer.size < size) break
                val command = buffer[1].toInt() and 0xFF
                frames += BaseusFrame(command, buffer.copyOfRange(2, size))
                buffer = buffer.copyOfRange(size, buffer.size)
            }
            return frames
        }
    }
}
