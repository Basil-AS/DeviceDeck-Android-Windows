package com.spizganed.quickbuds.protocol.edifier

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class EdifierFrameCodecTest {
    @Test fun d8ProfilesExposeOnlyLayoutsWithKnownPresetSemantics() {
        assertEquals(listOf("Classic", "Dynamic", "Custom"), EdifierEqualizer.presets(7).map { it.name })
        assertEquals(listOf(0, 1, 2, 3, 4), EdifierEqualizer.presets(10).map { it.id })
        assertTrue(EdifierEqualizer.presets(5).isEmpty())
        assertTrue(EdifierEqualizer.presets(11).isEmpty())
        assertTrue(EdifierEqualizer.presets(0).isEmpty())
    }

    @Test fun batteryQueryMatchesDocumentedFrame() {
        assertArrayEquals(
            byteArrayOf(0xAA.toByte(), 0x01, 0xD0.toByte(), 0x21, 0x94.toByte()),
            EdifierFrameCodec.encodeHost(byteArrayOf(0xD0.toByte()))
        )
    }

    @Test fun completeStateFrameIsValidatedAndDecoded() {
        val frame = EdifierFrameCodec.parse(
            byteArrayOf(0xBB.toByte(), 0x02, 0xD0.toByte(), 0x37, 0x21, 0xDD.toByte())
        )

        assertEquals(EdifierFrame.Header.STATE, frame?.header)
        assertEquals(0xD0, frame?.commandId())
        assertArrayEquals(byteArrayOf(0x37), frame?.parameters())
    }

    @Test fun decoderHandlesEverySplitBoundaryAndAdjacentFrames() {
        val first = EdifierFrameCodec.encode(EdifierFrame.Header.STATE, byteArrayOf(0xD0.toByte(), 55))
        val second = EdifierFrameCodec.encode(EdifierFrame.Header.ACK, byteArrayOf(0x09, 1))
        val joined = byteArrayOf(0x00, 0x7F) + first + second

        for (split in joined.indices) {
            val decoder = EdifierFrameDecoder()
            val decoded = decoder.feed(joined.copyOfRange(0, split)) + decoder.feed(joined.copyOfRange(split, joined.size))
            assertEquals("split at $split", listOfNotNull(EdifierFrameCodec.parse(first), EdifierFrameCodec.parse(second)), decoded)
        }
    }

    @Test fun damagedChecksumResynchronizesToFollowingFrame() {
        val bad = EdifierFrameCodec.encode(EdifierFrame.Header.STATE, byteArrayOf(0xD0.toByte(), 40)).also { it[it.lastIndex] = 0 }
        val good = EdifierFrameCodec.encode(EdifierFrame.Header.STATE, byteArrayOf(0xD0.toByte(), 61))
        val decoder = EdifierFrameDecoder()

        val frames = decoder.feed(bad + good)

        assertEquals(listOfNotNull(EdifierFrameCodec.parse(good)), frames)
    }

    @Test fun parserRejectsTruncationUnknownHeaderAndBadChecksum() {
        val good = EdifierFrameCodec.encodeHost(byteArrayOf(0xD0.toByte()))
        assertNull(EdifierFrameCodec.parse(good.copyOf(good.size - 1)))
        assertNull(EdifierFrameCodec.parse(good.copyOf().also { it[0] = 0xAB.toByte() }))
        assertNull(EdifierFrameCodec.parse(good.copyOf().also { it[it.lastIndex] = 0 }))
    }

    @Test fun encoderRejectsPayloadOverOneByteLength() {
        try {
            EdifierFrameCodec.encodeHost(ByteArray(256))
            throw AssertionError("oversized payload accepted")
        } catch (_: IllegalArgumentException) { }
    }

    @Test fun decoderResetDropsPartialFrame() {
        val frame = EdifierFrameCodec.encode(EdifierFrame.Header.STATE, byteArrayOf(0xD0.toByte(), 55))
        val decoder = EdifierFrameDecoder()
        assertTrue(decoder.feed(frame.copyOfRange(0, frame.size - 1)).isEmpty())
        decoder.reset()
        assertTrue(decoder.feed(frame.copyOfRange(frame.size - 1, frame.size)).isEmpty())
    }
}
