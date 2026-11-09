package com.spizganed.quickbuds.bluetooth

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class EdifierPromptLevelTest {
    @Test fun acceptsOnlyVendorPromptLevels() {
        assertEquals(0, decodeEdifierPromptLevel(byteArrayOf(0)))
        assertEquals(13, decodeEdifierPromptLevel(byteArrayOf(13)))
        assertEquals(15, decodeEdifierPromptLevel(byteArrayOf(15)))
        assertNull(decodeEdifierPromptLevel(byteArrayOf(16)))
        assertNull(decodeEdifierPromptLevel(byteArrayOf()))
    }
}
