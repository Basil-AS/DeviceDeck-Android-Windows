package com.spizganed.quickbuds.protocol.baseus

/** App-layer frames used by Baseus Intelligent's Blade 2 power-device screen. */
object BaseusMobilePowerCodec {
    private val prefix = byteArrayOf(0x9A.toByte(), 0xAA.toByte())
    private val standardPortProtocols = listOf("", "PPS", "PD3.0", "QC2.0", "QC3.0", "AFC", "FCP", "SCP")
    private val blade2_140PortProtocols = listOf("", "DCP", "", "QC", "", "AFC", "FCP", "SCP", "PD")

    /** Baseus Intelligent packs USB-C 1's protocol in the low byte and USB-C 2's in the high byte. */
    fun negotiatedProtocols(value: Int, blade2_140: Boolean): String? {
        val table = if (blade2_140) blade2_140PortProtocols else standardPortProtocols
        val ports = listOf("C1" to (value and 0xFF), "C2" to ((value ushr 8) and 0xFF))
        val labels = ports.mapNotNull { (port, index) -> table.getOrNull(index)?.takeIf { it.isNotBlank() }?.let { "$port $it" } }
        return labels.takeIf { it.isNotEmpty() }?.joinToString(" · ")
    }

    data class Status(val code: Int, val multiplier: Int, val value: Int) {
        fun scaled(): Double? = multiplier.takeIf { it != 0 }?.let { value.toDouble() / it }
    }

    fun read(code: Int): ByteArray {
        require(code in 0..0xFFFF)
        return appendCrc(prefix + byteArrayOf(0x03) + u16(code) + byteArrayOf(0, 1))
    }

    fun writeU16(code: Int, value: Int): ByteArray {
        require(code in 0..0xFFFF && value in 0..0xFFFF)
        return appendCrc(prefix + byteArrayOf(0x10) + u16(code) + byteArrayOf(0, 1, 2, 1) + u16(value))
    }

    fun writeText(text: String): ByteArray {
        val bytes = text.toByteArray(Charsets.UTF_8)
        require(text.isNotBlank()) { "Screen text cannot be empty" }
        require(bytes.size <= 255) { "Screen text exceeds the one-byte protocol length" }
        return appendCrc(prefix + byteArrayOf(0x10) + u16(0x0030) + byteArrayOf(0, 1, bytes.size.toByte(), 1) + bytes)
    }

    fun crc16(bytes: ByteArray): Int {
        var crc = 0xFFFF
        for (byte in bytes) {
            crc = crc xor (byte.toInt() and 0xFF)
            repeat(8) { crc = if (crc and 1 != 0) (crc ushr 1) xor 0xA001 else crc ushr 1 }
        }
        return crc and 0xFFFF
    }

    fun decodeStatus(frame: ByteArray): Status? {
        if (frame.size != 11 || frame[0] != prefix[0] || frame[1] != prefix[1] || frame[2] != 0x03.toByte()) return null
        val carried = u16(frame[9], frame[10])
        if (crc16(frame.copyOfRange(0, 9)) != carried || frame[5] != 0.toByte()) return null
        return Status(u16(frame[3], frame[4]), frame[6].toInt() and 0xFF, u16(frame[7], frame[8]))
    }

    class Decoder {
        private var buffer = ByteArray(0)

        fun reset() { buffer = ByteArray(0) }

        fun feed(input: ByteArray): List<Status> {
            if (input.isEmpty() || input.size > MAX_BUFFER) return emptyList()
            if (buffer.size + input.size > MAX_BUFFER) buffer = ByteArray(0)
            buffer += input
            val output = mutableListOf<Status>()
            while (buffer.size >= 2) {
                val start = findPrefix(buffer)
                if (start < 0) {
                    val keepPrefixLead = buffer.last() == prefix[0]
                    buffer = if (keepPrefixLead) byteArrayOf(buffer.last()) else ByteArray(0)
                    break
                }
                if (start > 0) buffer = buffer.copyOfRange(start, buffer.size)
                if (buffer.size < STATUS_SIZE) break
                val frame = buffer.copyOfRange(0, STATUS_SIZE)
                val status = decodeStatus(frame)
                if (status != null) {
                    output += status
                    buffer = buffer.copyOfRange(STATUS_SIZE, buffer.size)
                } else {
                    buffer = buffer.copyOfRange(1, buffer.size)
                }
            }
            return output
        }

        private fun findPrefix(bytes: ByteArray): Int {
            for (i in 0 until bytes.lastIndex) if (bytes[i] == prefix[0] && bytes[i + 1] == prefix[1]) return i
            return -1
        }
    }

    private fun appendCrc(body: ByteArray): ByteArray {
        val crc = crc16(body)
        return body + byteArrayOf((crc ushr 8).toByte(), crc.toByte())
    }

    private fun u16(value: Int): ByteArray = byteArrayOf((value ushr 8).toByte(), value.toByte())
    private fun u16(high: Byte, low: Byte): Int = ((high.toInt() and 0xFF) shl 8) or (low.toInt() and 0xFF)

    private const val STATUS_SIZE = 11
    private const val MAX_BUFFER = 512
}
