package com.spizganed.quickbuds.protocol.edifier

/** BLE wire settings selected by Edifier Connect from the advertisement manufacturer record. */
data class EdifierBleProtocol(
    val version: Int = 1,
    val encryptionKey: Int = 0,
    val appCode: Int = DEFAULT_APP_CODE,
) {
    init {
        require(version in 1..2)
        require(encryptionKey in 0..0xFF)
        require(appCode in 0..0xFF)
    }

    companion object {
        const val EDIFIER_MANUFACTURER_ID = 0x07E0
        const val DEFAULT_APP_CODE = 0xEC
        private const val ENCRYPTION_10_KEY = 16
        private const val ENCRYPTION_10_XOR = 0xA5

        /**
         * Connect 8.4.28 reads the final two bytes for Edifier's manufacturer ID as
         * `[BLE version, encryption key]`; other manufacturer IDs use BLE v1 without encryption.
         */
        fun fromAdvertisement(manufacturerId: Int?, manufacturerData: ByteArray?): EdifierBleProtocol {
            if (manufacturerId != EDIFIER_MANUFACTURER_ID || manufacturerData == null || manufacturerData.size < 2) {
                return EdifierBleProtocol()
            }
            val advertisedVersion = manufacturerData[manufacturerData.lastIndex - 1].toInt() and 0xFF
            val version = if (advertisedVersion > 1) 2 else 1
            val encryption = manufacturerData.last().toInt() and 0xFF
            val xorKey = if (encryption == ENCRYPTION_10_KEY) ENCRYPTION_10_XOR else 0
            return EdifierBleProtocol(version, xorKey)
        }
    }

    fun transformPayload(payload: ByteArray): ByteArray =
        if (encryptionKey == 0) payload.copyOf() else ByteArray(payload.size) { index -> (payload[index].toInt() xor encryptionKey).toByte() }
}
