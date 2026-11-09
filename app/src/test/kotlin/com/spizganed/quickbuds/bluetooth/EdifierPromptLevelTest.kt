package com.spizganed.quickbuds.bluetooth

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class EdifierPromptLevelTest {
    @Test fun acceptsOnlyVendorPromptLevels() {
        assertEquals(0, decodeEdifierPromptLevel(byteArrayOf(0)))
        assertEquals(12, decodeEdifierPromptLevel(byteArrayOf(12)))
        assertNull(decodeEdifierPromptLevel(byteArrayOf(13)))
        assertNull(decodeEdifierPromptLevel(byteArrayOf()))
    }
}
