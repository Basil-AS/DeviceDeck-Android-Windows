package com.spizganed.quickbuds.protocol

import com.spizganed.quickbuds.protocol.baseus.BaseusMobilePowerCodec
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class BaseusMobilePowerCodecTest {
    @Test fun crcMatchesStandardCheckValue() {
        assertEquals(0x4B37, BaseusMobilePowerCodec.crc16("123456789".toByteArray()))
    }

    @Test fun appReadAndWriteFramesHaveExpectedLayoutAndChecksum() {
        val read = BaseusMobilePowerCodec.read(0x000D)
        assertArrayEquals(byteArrayOf(0x9A.toByte(), 0xAA.toByte(), 3, 0, 13, 0, 1, read[7], read[8]), read)
        assertEquals(BaseusMobilePowerCodec.crc16(read.copyOfRange(0, 7)), ((read[7].toInt() and 255) shl 8) or (read[8].toInt() and 255))

        val write = BaseusMobilePowerCodec.writeU16(0x0020, 2)
        assertArrayEquals(byteArrayOf(0x9A.toByte(), 0xAA.toByte(), 0x10, 0, 0x20, 0, 1, 2, 1, 0, 2, write[11], write[12]), write)
        assertEquals(BaseusMobilePowerCodec.crc16(write.copyOfRange(0, 11)), ((write[11].toInt() and 255) shl 8) or (write[12].toInt() and 255))
    }

    @Test fun screenTextUsesUtf8ByteLengthAndRejectsInvalidInput() {
        val frame = BaseusMobilePowerCodec.writeText("OK")
        assertArrayEquals(byteArrayOf(0x9A.toByte(), 0xAA.toByte(), 0x10, 0, 0x30, 0, 1, 2, 1, 79, 75, frame[11], frame[12]), frame)
        assertTrue(runCatching { BaseusMobilePowerCodec.writeText("  ") }.isFailure)
        assertTrue(runCatching { BaseusMobilePowerCodec.writeText("x".repeat(256)) }.isFailure)
    }

    @Test fun statusDecoderChecksChecksumAndRetainsSplitNotifications() {
        val body = byteArrayOf(0x9A.toByte(), 0xAA.toByte(), 3, 0, 13, 0, 1, 0, 100)
        val crc = BaseusMobilePowerCodec.crc16(body)
        val frame = body + byteArrayOf((crc ushr 8).toByte(), crc.toByte())
        val decoder = BaseusMobilePowerCodec.Decoder()
        assertTrue(decoder.feed(frame.copyOfRange(0, 5)).isEmpty())
        val parsed = decoder.feed(frame.copyOfRange(5, frame.size)).single()
        assertEquals(0x000D, parsed.code)
        assertEquals(100, parsed.value)
        assertEquals(100.0, parsed.scaled()!!, 0.0)

        val damaged = frame.copyOf().also { it[8] = 99 }
        assertNull(BaseusMobilePowerCodec.decodeStatus(damaged))
    }
}
