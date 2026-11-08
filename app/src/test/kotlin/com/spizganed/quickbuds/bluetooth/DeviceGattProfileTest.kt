package com.spizganed.quickbuds.bluetooth

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class DeviceGattProfileTest {
    @Test fun edifierAdvertisementSearchIdIsNotMistakenForGattService() {
        val profile = DeviceGattProfiles.edifierW820NbPlus
        assertEquals("0000a500-0000-1000-8000-00805f9b34fb", profile.discoveryService.toString())
        assertNotEquals(profile.discoveryService, profile.service)
        assertTrue(profile.matchesName("EDIFIER W820NB Plus"))
        assertTrue(profile.matchesName("W820NB PLUS"))
        assertFalse(profile.matchesName("W820NB Plus Pro"))
    }

    @Test fun baseusSharedGattProfileRequiresAnAllowlistedModelName() {
        val profile = DeviceGattProfiles.baseusShared
        assertEquals("53527aa4-29f7-ae11-4e74-997334782568", profile.service.toString())
        assertEquals("ee684b1a-1e9b-ed3e-ee55-f894667e92ac", profile.write.toString())
        assertEquals("654b749c-e37f-ae1f-ebab-40ca133e3690", profile.notify.toString())
        assertTrue(profile.matchesName("Baseus Bowie 30"))
        assertTrue(profile.matchesName("Blade 2-140"))
        assertFalse(profile.matchesName("Baseus Power Bank"))
    }

    @Test fun blade2ProfileUsesTheSharedTransportButOnlyBlade2Names() {
        val profile = DeviceGattProfiles.baseusBlade2
        assertEquals(DeviceGattProfiles.baseusShared.service, profile.service)
        assertEquals(DeviceGattProfiles.baseusShared.write, profile.write)
        assertEquals(DeviceGattProfiles.baseusShared.notify, profile.notify)
        assertTrue(profile.matchesName("Baseus Blade 2-140"))
        assertTrue(profile.matchesName("Blade 2"))
        assertFalse(profile.matchesName("Baseus Bowie 30"))
    }
}
