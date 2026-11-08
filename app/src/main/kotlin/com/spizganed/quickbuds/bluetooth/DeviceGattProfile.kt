package com.spizganed.quickbuds.bluetooth

import java.util.UUID

/** Exact GATT endpoints and local-name allowlist for one DeviceDeck device profile. */
data class DeviceGattProfile(
    val id: String,
    val displayName: String,
    val service: UUID,
    val write: UUID,
    val notify: UUID,
    val acceptedNames: Set<String>,
    val discoveryService: UUID = service,
) {
    fun matchesName(name: String?): Boolean = name != null &&
        acceptedNames.any { it.equals(name.trim(), ignoreCase = true) }
}

/** Static profiles are discovery hints, not proof that a model accepts every command. */
object DeviceGattProfiles {
    val edifierW820NbPlus = DeviceGattProfile(
        id = "edifier:w820nb-plus",
        displayName = "EDIFIER W820NB Plus",
        service = UUID.fromString("48093801-1A48-11E9-AB14-D663BD873D93"),
        write = UUID.fromString("48093803-1A48-11E9-AB14-D663BD873D93"),
        notify = UUID.fromString("48093802-1A48-11E9-AB14-D663BD873D93"),
        acceptedNames = setOf("EDIFIER W820NB Plus", "W820NB Plus", "W820NB PLUS"),
        // Edifier Connect uses this model-search UUID in advertisements; it is
        // distinct from the GATT service and endpoints above.
        discoveryService = UUID.fromString("0000A500-0000-1000-8000-00805F9B34FB"),
    )

    // Baseus Intelligent EU app_device_ble_config group 4. The models share
    // transport UUIDs but do not share a command protocol or feature set.
    val baseusShared = DeviceGattProfile(
        id = "baseus:shared-ble",
        displayName = "Baseus BLE device",
        service = UUID.fromString("53527AA4-29F7-AE11-4E74-997334782568"),
        write = UUID.fromString("EE684B1A-1E9B-ED3E-EE55-F894667E92AC"),
        notify = UUID.fromString("654B749C-E37F-AE1F-EBAB-40CA133E3690"),
        acceptedNames = setOf("Baseus Bowie 30", "Bowie 30", "Baseus Blade 2", "Blade 2", "Blade 2-140"),
    )

    val baseusBlade2 = baseusShared.copy(
        id = "baseus:blade-2",
        displayName = "Baseus Blade 2",
        acceptedNames = setOf("Baseus Blade 2", "Blade 2", "Blade 2-140", "Baseus Blade 2-140"),
    )
}
