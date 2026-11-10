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
 * Frame codec for the Edifier BLE v1 and v2 command protocols.
 *
 * V1 uses an additive 16-bit sum seeded with 0x2019. V2 uses the low byte of the
 * additive sum and the app code. This does not imply that a command is supported by a model.
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

    fun encodeHost(payload: ByteArray, protocol: EdifierBleProtocol = EdifierBleProtocol()): ByteArray =
        encode(EdifierFrame.Header.HOST, payload, protocol)

    fun encode(
        header: EdifierFrame.Header,
        payload: ByteArray,
        protocol: EdifierBleProtocol = EdifierBleProtocol(),
    ): ByteArray {
        require(payload.size <= MAX_PAYLOAD_SIZE) { "Edifier payload exceeds the one-byte length limit" }
        if (protocol.version == 2) {
            val encryptedPayload = protocol.transformPayload(payload.drop(1).toByteArray())
            val v2 = ByteArray(encryptedPayload.size + 6)
            v2[0] = header.wire.toByte()
            v2[1] = protocol.appCode.toByte()
            v2[2] = payload.firstOrNull() ?: 0.toByte()
            v2[3] = ((encryptedPayload.size ushr 8) and 0xFF).toByte()
            v2[4] = (encryptedPayload.size and 0xFF).toByte()
            encryptedPayload.copyInto(v2, destinationOffset = 5)
            v2[v2.lastIndex] = checksumV2(v2, v2.lastIndex).toByte()
            return v2
        }
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
    fun parse(raw: ByteArray, protocol: EdifierBleProtocol = EdifierBleProtocol()): EdifierFrame? {
        if (raw.size < 4) return null
        val header = EdifierFrame.Header.fromWire(raw[0].toInt() and 0xFF) ?: return null
        if (protocol.version == 2) {
            if (raw.size < 6 || (raw[1].toInt() and 0xFF) != protocol.appCode) return null
            val parameterSize = ((raw[3].toInt() and 0xFF) shl 8) or (raw[4].toInt() and 0xFF)
            if (raw.size != parameterSize + 6 || checksumV2(raw, raw.lastIndex) != (raw.last().toInt() and 0xFF)) return null
            val parameters = protocol.transformPayload(raw.copyOfRange(5, raw.lastIndex))
            return EdifierFrame(header, byteArrayOf(raw[2]) + parameters)
        }
        val payloadSize = raw[1].toInt() and 0xFF
        if (raw.size != payloadSize + 4) return null
        val expected = checksum(raw, raw.size - 2)
        val received = ((raw[raw.lastIndex - 1].toInt() and 0xFF) shl 8) or (raw.last().toInt() and 0xFF)
        if (expected != received) return null
        return EdifierFrame(header, raw.copyOfRange(2, raw.size - 2))
    }

    private fun checksumV2(bytes: ByteArray, length: Int): Int {
        var sum = 0
        for (index in 0 until length) sum = (sum + (bytes[index].toInt() and 0xFF)) and 0xFF
        return sum
    }
}

/** Incremental decoder; BLE notifications may split or concatenate protocol frames. */
class EdifierFrameDecoder(
    private val protocol: EdifierBleProtocol = EdifierBleProtocol(),
    private val maximumBufferedBytes: Int = 8192,
) {
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

            val totalSize = if (protocol.version == 2) {
                if (buffer.size < 5) break
                (((buffer[3].toInt() and 0xFF) shl 8) or (buffer[4].toInt() and 0xFF)) + 6
            } else (buffer[1].toInt() and 0xFF) + 4
            if (buffer.size < totalSize) break

            val candidate = buffer.copyOfRange(0, totalSize)
            val frame = EdifierFrameCodec.parse(candidate, protocol)
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
