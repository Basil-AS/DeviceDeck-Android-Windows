package com.spizganed.quickbuds.protocol.edifier

/** Edifier's AA host / BB state / CC acknowledgement frame format. */
data class EdifierFrame(val header: Header, val payload: ByteArray) {
    enum class Header(val wire: Int) {
        HOST(0xAA), STATE(0xBB), ACK(0xCC);

        companion object {
            fun fromWire(value: Int): Header? = entries.firstOrNull { it.wire == value }
        }
    }

    fun commandId(): Int? = payload.firstOrNull()?.toInt()?.and(0xFF)
    fun parameters(): ByteArray = if (payload.isEmpty()) byteArrayOf() else payload.copyOfRange(1, payload.size)

    override fun equals(other: Any?): Boolean = other is EdifierFrame && header == other.header && payload.contentEquals(other.payload)
    override fun hashCode(): Int = 31 * header.hashCode() + payload.contentHashCode()
}

/**
 * Frame codec for the Edifier `AA|length|payload|checksum` protocol.
 *
 * The checksum is the vendor client's additive 16-bit sum (seed 0x2019); it is not a CRC.
 * This only handles framing. It does not imply that a command is supported by a particular model.
 */
object EdifierFrameCodec {
    const val MAX_PAYLOAD_SIZE = 0xFF
    private const val CHECKSUM_SEED = 0x2019

    fun checksum(bytes: ByteArray, length: Int = bytes.size): Int {
        require(length in 0..bytes.size)
        var sum = CHECKSUM_SEED
        for (index in 0 until length) sum = (sum + (bytes[index].toInt() and 0xFF)) and 0xFFFF
        return sum
    }

    fun encodeHost(payload: ByteArray): ByteArray = encode(EdifierFrame.Header.HOST, payload)

    fun encode(header: EdifierFrame.Header, payload: ByteArray): ByteArray {
        require(payload.size <= MAX_PAYLOAD_SIZE) { "Edifier payload exceeds the one-byte length limit" }
        val frame = ByteArray(payload.size + 4)
        frame[0] = header.wire.toByte()
        frame[1] = payload.size.toByte()
        payload.copyInto(frame, destinationOffset = 2)
        val sum = checksum(frame, payload.size + 2)
        frame[frame.lastIndex - 1] = (sum ushr 8).toByte()
        frame[frame.lastIndex] = sum.toByte()
        return frame
    }

    /** Parse one complete frame. Returns null for truncation, invalid headers, or bad checksums. */
    fun parse(raw: ByteArray): EdifierFrame? {
        if (raw.size < 4) return null
        val header = EdifierFrame.Header.fromWire(raw[0].toInt() and 0xFF) ?: return null
        val payloadSize = raw[1].toInt() and 0xFF
        if (raw.size != payloadSize + 4) return null
        val expected = checksum(raw, raw.size - 2)
        val received = ((raw[raw.lastIndex - 1].toInt() and 0xFF) shl 8) or (raw.last().toInt() and 0xFF)
        if (expected != received) return null
        return EdifierFrame(header, raw.copyOfRange(2, raw.size - 2))
    }
}

/** Incremental decoder; BLE notifications may split or concatenate protocol frames. */
class EdifierFrameDecoder(private val maximumBufferedBytes: Int = 8192) {
    init { require(maximumBufferedBytes >= 4) }

    private var buffer = ByteArray(0)

    fun reset() { buffer = ByteArray(0) }

    fun feed(bytes: ByteArray): List<EdifierFrame> {
        if (bytes.isEmpty()) return emptyList()
        if (bytes.size > maximumBufferedBytes) {
            buffer = ByteArray(0)
            return emptyList()
        }
        if (buffer.size + bytes.size > maximumBufferedBytes) buffer = ByteArray(0)
        buffer += bytes

        val frames = mutableListOf<EdifierFrame>()
        while (buffer.isNotEmpty()) {
            val headerIndex = buffer.indexOfFirst { EdifierFrame.Header.fromWire(it.toInt() and 0xFF) != null }
            if (headerIndex < 0) {
                buffer = ByteArray(0)
                break
            }
            if (headerIndex > 0) buffer = buffer.copyOfRange(headerIndex, buffer.size)
            if (buffer.size < 2) break

            val totalSize = (buffer[1].toInt() and 0xFF) + 4
            if (buffer.size < totalSize) break

            val candidate = buffer.copyOfRange(0, totalSize)
            val frame = EdifierFrameCodec.parse(candidate)
            if (frame == null) {
                // Advance one byte so a damaged or false header cannot consume a valid next frame.
                buffer = buffer.copyOfRange(1, buffer.size)
            } else {
                frames += frame
                buffer = buffer.copyOfRange(totalSize, buffer.size)
            }
        }
        return frames
    }
}

/** Preset layouts reported by Edifier Connect's W820NB Plus D8 capability record. */
object EdifierEqualizer {
    private val profiles = mapOf(
        1 to listOf("Classic", "Popular", "Classical", "Rock"),
        2 to listOf("Classic", "Popular", "Classical", "Rock", "Dance", "Blues"),
        3 to listOf("Classic", "Popular", "Rock"),
        4 to listOf("Classic", "Popular", "Classical", "Rock", "Custom"),
        6 to listOf("Standard", "Surround", "Game"),
        7 to listOf("Classic", "Dynamic", "Custom"),
        8 to listOf("Classic", "Dynamic", "Spatial", "Custom"),
        9 to listOf("Classic", "HiFi Camel", "STAX"),
        10 to listOf("Classic", "Monitor", "Variable", "Vocal", "Custom"),
    )

    fun presets(profile: Int): List<Preset> = profiles[profile].orEmpty().mapIndexed { id, name -> Preset(id, name) }
    data class Preset(val id: Int, val name: String)
}
