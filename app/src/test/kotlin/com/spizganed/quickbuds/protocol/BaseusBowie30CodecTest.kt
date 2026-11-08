package com.spizganed.quickbuds.protocol

import com.spizganed.quickbuds.protocol.baseus.BaseusBowie30Codec
import com.spizganed.quickbuds.protocol.baseus.BaseusFrame
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

class BaseusBowie30CodecTest {
    @Test fun encodesBareBaseusCommands() {
        assertArrayEquals(byteArrayOf(0xBA.toByte(), 0x02), BaseusBowie30Codec.request(0x02))
        assertArrayEquals(byteArrayOf(0xBA.toByte(), 0x34, 1, 8.toByte()), BaseusBowie30Codec.request(0x34, 1, 8))
    }

    @Test fun decodesSplitAndCoalescedReplies() {
        val decoder = BaseusBowie30Codec.Decoder()
        assertEquals(emptyList<BaseusFrame>(), decoder.feed(byteArrayOf(0x55, 0xAA.toByte(), 0x02, 80)))
        val frames = decoder.feed(byteArrayOf(0, 64, 1, 0xAA.toByte(), 0x34, 1))
        assertEquals(listOf(0x02, 0x34), frames.map { it.command })
        assertEquals(listOf(80, 0, 64, 1), (0..3).map(frames[0]::u8))
        assertEquals(1, frames[1].u8(0))
    }
}
