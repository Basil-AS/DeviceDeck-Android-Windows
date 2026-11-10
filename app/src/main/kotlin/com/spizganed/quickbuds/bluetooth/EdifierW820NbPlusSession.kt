package com.spizganed.quickbuds.bluetooth

import android.bluetooth.BluetoothDevice
import android.content.Context
import android.os.Handler
import android.os.Looper
import com.spizganed.quickbuds.protocol.edifier.EdifierFrame
import com.spizganed.quickbuds.protocol.edifier.EdifierFrameCodec
import com.spizganed.quickbuds.protocol.edifier.EdifierFrameDecoder
import com.spizganed.quickbuds.protocol.edifier.EdifierBleProtocol
import com.spizganed.quickbuds.protocol.edifier.EdifierEqualizer
import java.util.ArrayDeque

internal fun decodeEdifierPromptLevel(parameters: ByteArray): Int? =
    parameters.firstOrNull()?.toInt()?.and(0xFF)?.takeIf { it in 0..15 }

/** W820NB Plus command session over DeviceDeck's shared GATT link. */
class EdifierW820NbPlusSession(
    context: Context,
    device: BluetoothDevice,
    protocol: EdifierBleProtocol = EdifierBleProtocol(),
    private val listener: Listener,
) {
    enum class Noise(val wireValue: Int, val title: String) {
        OFF(1, "Off"), ANC(2, "Noise cancelling"), TRANSPARENCY(3, "Ambient sound"),
    }

    data class State(
        val batteryPercent: Int? = null,
        val noise: Noise? = null,
        val equalizerPresets: List<EdifierEqualizer.Preset> = emptyList(),
        val selectedEqualizer: Int? = null,
        val sleepTimerOptions: List<Int> = emptyList(),
        val sleepTimerMinutes: Int? = null,
        val gameModeSupported: Boolean = false,
        val gameModeEnabled: Boolean? = null,
        val autoPowerOffSupported: Boolean = false,
        val autoPowerOffEnabled: Boolean? = null,
        val promptToneSwitchSupported: Boolean = false,
        val promptToneEnabled: Boolean? = null,
        val promptVolumeSupported: Boolean = false,
        val promptVolumeLevel: Int? = null,
        val bluetoothNameSupported: Boolean = false,
        val bluetoothNameLimitBytes: Int = 0,
        val bluetoothName: String? = null,
        val firmwareVersionSupported: Boolean = false,
        val firmwareVersion: String? = null,
        val bluetoothAddressSupported: Boolean = false,
        val bluetoothAddress: String? = null,
        val audioDisconnectSupported: Boolean = false,
        val pairingRepairSupported: Boolean = false,
        val devicePowerOffSupported: Boolean = false,
        val touchSensitivitySupported: Boolean = false,
        val touchSensitivityIndex: Int? = null,
        val touchSensitivityValue: Int? = null,
        val touchControlsSupported: Boolean = false,
        val touchControls: List<TouchControl> = emptyList(),
    )

    /** W820NB Plus control slots exposed by Connect's D8 tap profile 18. */
    data class TouchControl(val index: Int, val action: Int, val noiseMask: Int? = null, val selectedNoise: Int? = null)

    interface Listener {
        fun onState(state: State)
        fun onStatus(text: String)
        fun onReady()
        fun onClosed(reason: String)
    }

    private data class Operation(val command: Int, val packet: ByteArray, val done: (EdifierFrame) -> Unit)

    private val handler = Handler(Looper.getMainLooper())
    private var activeProtocol = protocol
    private var decoder = EdifierFrameDecoder(activeProtocol)
    private val protocolFallbacks = ArrayDeque(
        listOf(EdifierBleProtocol(2, 0xA5), EdifierBleProtocol(2, 0), EdifierBleProtocol(1, 0))
            .filter { it != protocol }
    )
    private val queue = ArrayDeque<Operation>()
    private var pending: Operation? = null
    private var ready = false
    private var state = State()
    private val responseTimeout = Runnable {
        val timedOut = pending
        pending = null
        if (timedOut?.command == 0xD8) {
            // A bonded-device fallback may connect without exposing the manufacturer
            // advertisement that selects v1/v2 and Encryption10. Probe the known wire
            // modes before giving up on capability discovery.
            val fallback = protocolFallbacks.pollFirst()
            if (fallback != null) {
                activeProtocol = fallback
                decoder = EdifierFrameDecoder(activeProtocol)
                listener.onStatus("No capability reply; retrying BLE v${activeProtocol.version} (${if (activeProtocol.encryptionKey == 0) "plain" else "Encryption10"})")
                enqueue(0xD8) { frame -> handleCapabilities(frame) }
                pump()
            } else {
                listener.onStatus("Capability probe was not answered; trying basic controls")
                refresh()
                pump()
            }
        } else if (timedOut?.command == 0xCE) {
            listener.onStatus("Power-off command sent; the headphones may disconnect without acknowledging it")
            pump()
        } else if (timedOut?.command == 0x05 || timedOut?.command == 0x06 || timedOut?.command == 0xC6 || timedOut?.command == 0xC8 || timedOut?.command == 0xD3 || timedOut?.command == 0x08 || timedOut?.command == 0xD7 || timedOut?.command == 0xC9 || timedOut?.command == 0xFD || timedOut?.command == 0xFE || timedOut?.command == 0xF0 || timedOut?.command == 0xF1 || timedOut?.command == 0xF9 || timedOut?.command == 0xFA) {
            listener.onStatus("Optional Edifier status was not answered; other controls remain available")
            pump()
        } else {
            close()
            listener.onClosed("The headphones did not answer. Reconnect and try again.")
        }
    }
    private val link = BleGattLink.connect(context, device, DeviceGattProfiles.edifierW820NbPlus, object : BleGattLink.Listener {
        override fun onReady(mtu: Int) {
            ready = true
            val encryption = if (activeProtocol.encryptionKey == 0) "no encryption" else "Encryption10"
            listener.onStatus("Connected · BLE v${activeProtocol.version}, $encryption · MTU $mtu")
            // D8 is a capability probe. Its layout is firmware-sensitive; retain it as
            // evidence but do not enable commands from an unvalidated interpretation.
            enqueue(0xD8) { frame -> handleCapabilities(frame) }
            pump()
            listener.onReady()
        }

        override fun onBytes(bytes: ByteArray) {
            decoder.feed(bytes).forEach { frame ->
                val operation = pending ?: return@forEach
                if (frame.commandId() != operation.command) return@forEach
                pending = null
                handler.removeCallbacks(responseTimeout)
                operation.done(frame)
                pump()
            }
        }

        override fun onDisconnected(status: Int) = closed("Disconnected")
        override fun onError(message: String) = closed(message)
    })

    private fun handleCapabilities(frame: EdifierFrame) {
                val payload = frame.parameters()
                val presets = if (payload.size >= 14) EdifierEqualizer.presets(payload[13].toInt() and 0xFF) else emptyList()
                val timerProfile = payload.getOrNull(9)?.toInt()?.and(0xFF) ?: 0
                val gameModeSupported = (payload.getOrNull(17)?.toInt()?.and(0x40) ?: 0) != 0
                val touchSensitivitySupported = (payload.getOrNull(17)?.toInt()?.and(0x08) ?: 0) != 0
                val touchControlsSupported = payload.getOrNull(16)?.toInt()?.and(0xFF) == 18
                val autoPowerOffSupported = payload.getOrNull(8)?.toInt()?.and(0xFF) == 1
                val promptToneSwitchSupported = (payload.getOrNull(17)?.toInt()?.and(0x02) ?: 0) != 0
                val promptVolumeSupported = (payload.getOrNull(17)?.toInt()?.and(0x10) ?: 0) != 0
                val nameSupported = payload.getOrNull(2)?.toInt()?.and(0xFF) == 1
                val bluetoothAddressSupported = payload.getOrNull(4)?.toInt()?.and(0xFF) == 1
                val firmwareVersionSupported = payload.getOrNull(5)?.toInt()?.and(0xFF) == 1
                val audioDisconnectSupported = payload.getOrNull(6)?.toInt()?.and(0xFF) == 1
                val pairingRepairSupported = payload.getOrNull(7)?.toInt()?.and(0xFF) == 1
                val devicePowerOffSupported = payload.getOrNull(10)?.toInt()?.and(0xFF) == 1
                val nameLimit = when (payload.getOrNull(3)?.toInt()?.and(0xFF)) {
                    1, 6 -> 24; 2, 5 -> 30; 3 -> 29; 4 -> 35; else -> 0
                }
                val timerOptions = when (timerProfile) {
                    1 -> listOf(0, 10, 20, 30, 60, 90)
                    2 -> listOf(0, 5, 15, 30, 60, 180)
                    else -> emptyList()
                }
                state = state.copy(equalizerPresets = presets, sleepTimerOptions = timerOptions,
                    gameModeSupported = gameModeSupported, autoPowerOffSupported = autoPowerOffSupported,
                    promptToneSwitchSupported = promptToneSwitchSupported,
                    promptVolumeSupported = promptVolumeSupported,
                    bluetoothNameSupported = nameSupported, bluetoothNameLimitBytes = nameLimit,
                    firmwareVersionSupported = firmwareVersionSupported,
                    bluetoothAddressSupported = bluetoothAddressSupported,
                    audioDisconnectSupported = audioDisconnectSupported,
                    pairingRepairSupported = pairingRepairSupported,
                    devicePowerOffSupported = devicePowerOffSupported,
                    touchSensitivitySupported = touchSensitivitySupported,
                    touchControlsSupported = touchControlsSupported)
                listener.onState(state)
                listener.onStatus("Capabilities received · BLE v${activeProtocol.version}${if (activeProtocol.encryptionKey == 0) "" else " · Encryption10"}" + if (presets.isEmpty()) " · no supported EQ profile" else " · ${presets.size} EQ presets")
                refresh()
    }

    fun refresh() {
        if (!ready) return
        enqueue(0xD0) { frame ->
            val value = frame.parameters().firstOrNull()?.toInt()?.and(0xFF)
            if (value == null || value > 100) {
                listener.onStatus("Battery response was not recognized")
                return@enqueue
            }
            state = state.copy(batteryPercent = value)
            listener.onState(state)
            readNoise()
            if (state.equalizerPresets.isNotEmpty()) readEqualizer()
            if (state.sleepTimerOptions.isNotEmpty()) readSleepTimer()
            if (state.gameModeSupported) readGameMode()
            if (state.autoPowerOffSupported) readAutoPowerOff()
            if (state.promptToneSwitchSupported) readPromptToneSwitch()
            if (state.promptVolumeSupported) readPromptVolume()
            if (state.bluetoothNameSupported) readBluetoothName()
            if (state.firmwareVersionSupported) readFirmwareVersion()
            if (state.bluetoothAddressSupported) readBluetoothAddress()
            if (state.touchSensitivitySupported) readTouchSensitivity()
            if (state.touchControlsSupported) readTouchControls()
        }
        pump()
    }

    fun setNoise(mode: Noise) {
        if (!ready || state.noise == null) {
            listener.onStatus("Read the current noise mode before changing it")
            return
        }
        enqueue(0xC1, byteArrayOf(mode.wireValue.toByte())) { acknowledgement ->
            if (acknowledgement.header != EdifierFrame.Header.ACK) {
                listener.onStatus("The headphones did not acknowledge the noise mode")
                return@enqueue
            }
            readNoise(expected = mode)
        }
        pump()
    }

    fun setEqualizer(id: Int) {
        if (!ready || state.selectedEqualizer == null || state.equalizerPresets.none { it.id == id }) {
            listener.onStatus("Read a supported EQ profile before changing it")
            return
        }
        enqueue(0xC4, byteArrayOf(id.toByte())) { ack ->
            if (ack.header != EdifierFrame.Header.ACK) {
                listener.onStatus("The headphones did not acknowledge the EQ preset")
                return@enqueue
            }
            readEqualizer(expected = id)
        }
        pump()
    }

    fun setSleepTimer(minutes: Int) {
        if (!ready || minutes !in state.sleepTimerOptions) {
            listener.onStatus("Sleep timer value is not supported by this firmware")
            return
        }
        val command = if (minutes == 0) 0xD2 else 0xD1
        val params = if (minutes == 0) byteArrayOf() else byteArrayOf((minutes shr 8).toByte(), minutes.toByte())
        enqueue(command, params) { ack ->
            if (ack.header != EdifierFrame.Header.ACK) {
                listener.onStatus("The headphones did not acknowledge the sleep timer")
                return@enqueue
            }
            readSleepTimer(expected = minutes)
        }
        pump()
    }

    fun setGameMode(enabled: Boolean) {
        if (!ready || !state.gameModeSupported || state.gameModeEnabled == null) {
            listener.onStatus("Game mode is not available in the reported device profile")
            return
        }
        enqueue(0x09, byteArrayOf(if (enabled) 1 else 0)) { ack ->
            if (ack.header != EdifierFrame.Header.ACK) {
                listener.onStatus("The headphones did not acknowledge game mode")
                return@enqueue
            }
            readGameMode(expected = enabled)
        }
        pump()
    }

    fun setAutoPowerOff(enabled: Boolean) {
        if (!ready || !state.autoPowerOffSupported || state.autoPowerOffEnabled == null) {
            listener.onStatus("Automatic power-off is not available in the reported device profile")
            return
        }
        enqueue(0xD6, byteArrayOf(if (enabled) 1 else 0)) { ack ->
            if (ack.header != EdifierFrame.Header.ACK) {
                listener.onStatus("The headphones did not acknowledge automatic power-off")
                return@enqueue
            }
            readAutoPowerOff(expected = enabled)
        }
        pump()
    }

    fun setPromptToneSwitch(enabled: Boolean) {
        if (!ready || !state.promptToneSwitchSupported || state.promptToneEnabled == null) {
            listener.onStatus("Prompt tone control is unavailable in the reported device profile")
            return
        }
        enqueue(0xFA, byteArrayOf(if (enabled) 1 else 0)) { ack ->
            if (ack.header != EdifierFrame.Header.ACK) {
                listener.onStatus("The headphones did not acknowledge the prompt tone setting")
                return@enqueue
            }
            readPromptToneSwitch(expected = enabled)
        }
        pump()
    }

    fun setPromptVolume(level: Int) {
        if (!ready || !state.promptVolumeSupported || state.promptVolumeLevel == null || level !in 0..15) {
            listener.onStatus("Voice prompt volume is not available in the reported device profile")
            return
        }
        enqueue(0x06, byteArrayOf(level.toByte())) { ack ->
            if (ack.header != EdifierFrame.Header.ACK) {
                listener.onStatus("The headphones did not acknowledge voice prompt volume")
                return@enqueue
            }
            readPromptVolume(expected = level)
        }
        pump()
    }

    fun setBluetoothName(name: String) {
        val bytes = name.toByteArray(Charsets.UTF_8)
        if (!ready || !state.bluetoothNameSupported || state.bluetoothNameLimitBytes <= 0 ||
            name.isBlank() || bytes.size > state.bluetoothNameLimitBytes || name.any { Character.isISOControl(it) }) {
            listener.onStatus("Name is not supported or exceeds the reported UTF-8 byte limit")
            return
        }
        enqueue(0xCA, bytes) { ack ->
            if (ack.header != EdifierFrame.Header.ACK) {
                listener.onStatus("The headphones did not acknowledge the name change")
                return@enqueue
            }
            readBluetoothName(expected = name)
        }
        pump()
    }

    /** D8 byte 6 advertises the vendor's 0xCD audio-disconnect operation. */
    fun disconnectAudio() = sendBluetoothAction(0xCD, state.audioDisconnectSupported,
        "Audio disconnect is not reported as supported by these headphones")

    /** D8 byte 7 advertises the vendor's 0xCF Bluetooth repair/pairing operation. */
    fun enterPairingMode() = sendBluetoothAction(0xCF, state.pairingRepairSupported,
        "Bluetooth repair is not reported as supported by these headphones")

    /** D8 byte 10 advertises the vendor's 0xCE device power-off operation. */
    fun powerOffDevice() {
        if (!ready || !state.devicePowerOffSupported) {
            listener.onStatus("Device power-off is not reported as supported by these headphones")
            return
        }
        enqueue(0xCE) { ack ->
            listener.onStatus(if (ack.header == EdifierFrame.Header.ACK) {
                "Power-off command acknowledged; the headphones may disconnect"
            } else "Power-off command sent; the headphones may disconnect without acknowledging it")
        }
        pump()
    }

    private fun sendBluetoothAction(command: Int, supported: Boolean, unsupportedMessage: String) {
        if (!ready || !supported) {
            listener.onStatus(unsupportedMessage)
            return
        }
        enqueue(command) { ack ->
            listener.onStatus(if (ack.header == EdifierFrame.Header.ACK) {
                if (command == 0xCF) "Pairing mode command acknowledged; reconnect from Android Bluetooth settings"
                else "Audio disconnect command acknowledged"
            } else "The headphones did not acknowledge the Bluetooth action")
        }
        pump()
    }

    fun setTouchSensitivity(value: Int) {
        val index = state.touchSensitivityIndex
        val maximum = when (index) { 1 -> 31; 2 -> 15; else -> -1 }
        if (!ready || !state.touchSensitivitySupported || index == null || value !in 0..maximum || state.touchSensitivityValue == null) {
            listener.onStatus("Touch sensitivity is unavailable in the reported device profile")
            return
        }
        enqueue(0xFE, byteArrayOf(index.toByte(), value.toByte())) { ack ->
            if (ack.header != EdifierFrame.Header.ACK) {
                listener.onStatus("The headphones did not acknowledge touch sensitivity")
                return@enqueue
            }
            readTouchSensitivity(expected = value)
        }
        pump()
    }

    fun setTouchControl(index: Int, action: Int, selectedNoiseModes: List<Int> = emptyList()) {
        if (!ready || !state.touchControlsSupported || index !in 1..4 || action !in 1..6) return
        val payload = if (action == 6) {
            val unique = selectedNoiseModes.distinct()
            if (unique.size !in 2..3 || unique.any { it !in 0..4 }) {
                listener.onStatus("Choose two or three supported noise modes")
                return
            }
            val mask = unique.fold(0) { bits, value -> bits or (1 shl value) }
            byteArrayOf(index.toByte(), mask.toByte(), unique.last().toByte())
        } else byteArrayOf(index.toByte(), action.toByte())
        enqueue(0xF1, payload) { ack ->
            if (ack.header != EdifierFrame.Header.ACK) {
                listener.onStatus("The headphones did not acknowledge the touch control")
                return@enqueue
            }
            readTouchControl(index, expectedAction = action, expectedModes = selectedNoiseModes)
        }
        pump()
    }

    private fun readTouchControls() {
        (1..4).forEach { readTouchControl(it) }
    }

    private fun readTouchControl(index: Int, expectedAction: Int? = null, expectedModes: List<Int> = emptyList()) {
        enqueue(0xF0, byteArrayOf(index.toByte())) { frame ->
            val values = frame.parameters().map { it.toInt() and 0xFF }
            if (values.size !in 2..3 || values[0] != index || values[1] !in 1..6 ||
                (values[1] == 6 && values.size != 3) || (values[1] != 6 && values.size != 2)) {
                listener.onStatus("Touch-control response was not recognized")
                return@enqueue
            }
            val control = if (values[1] == 6) {
                if (values[2] and 0xE0 != 0 || Integer.bitCount(values[2]) !in 2..3) {
                    listener.onStatus("Touch-control noise-mode mask was invalid")
                    return@enqueue
                }
                TouchControl(index, 6, values[2], null)
            } else TouchControl(index, values[1])
            state = state.copy(touchControls = state.touchControls.filterNot { it.index == index } + control)
            listener.onState(state)
            if (expectedAction != null && control.action != expectedAction) {
                listener.onStatus("Touch control was not confirmed by device readback")
            } else if (expectedAction == 6 && control.noiseMask != null) {
                val actual = (0..4).filter { control.noiseMask and (1 shl it) != 0 }
                if (actual != expectedModes.sorted()) listener.onStatus("Noise-mode choices were not confirmed by device readback")
            }
        }
        pump()
    }

    private fun readTouchSensitivity(expected: Int? = null) {
        enqueue(0xFD) { frame ->
            val p = frame.parameters()
            if (p.size < 2) { listener.onStatus("Touch sensitivity response was not recognized"); return@enqueue }
            val index = p[0].toInt() and 0xFF
            val value = p[1].toInt() and 0xFF
            val maximum = when (index) { 1 -> 31; 2 -> 15; else -> -1 }
            if (maximum < 0 || value !in 0..maximum) {
                listener.onStatus("Touch sensitivity response is outside the reported profile")
                return@enqueue
            }
            state = state.copy(touchSensitivityIndex = index, touchSensitivityValue = value)
            listener.onState(state)
            if (expected != null && expected != value) listener.onStatus("Touch sensitivity was not confirmed by device readback")
        }
        pump()
    }

    private fun readBluetoothName(expected: String? = null) {
        enqueue(0xC9) { frame ->
            val name = frame.parameters().toString(Charsets.UTF_8).trimEnd('\u0000').takeIf { it.isNotBlank() }
            if (name == null) {
                listener.onStatus("Bluetooth name response was not recognized")
                return@enqueue
            }
            state = state.copy(bluetoothName = name)
            listener.onState(state)
            if (expected != null && expected != name) listener.onStatus("Bluetooth name was not confirmed by device readback")
        }
        pump()
    }

    private fun readFirmwareVersion() {
        enqueue(0xC6) { frame ->
            val versionBytes = frame.parameters()
            if (versionBytes.isEmpty()) {
                listener.onStatus("Firmware version response was empty")
                return@enqueue
            }
            val version = versionBytes.joinToString(".") { (it.toInt() and 0xFF).toString() }
            state = state.copy(firmwareVersion = version)
            listener.onState(state)
        }
        pump()
    }

    private fun readBluetoothAddress() {
        enqueue(0xC8) { frame ->
            val addressBytes = frame.parameters()
            if (addressBytes.size != 6) {
                listener.onStatus("Bluetooth address response was not recognized")
                return@enqueue
            }
            val address = addressBytes.joinToString(":") { "%02X".format(it.toInt() and 0xFF) }
            state = state.copy(bluetoothAddress = address)
            listener.onState(state)
        }
        pump()
    }

    private fun readAutoPowerOff(expected: Boolean? = null) {
        enqueue(0xD7) { frame ->
            val enabled = when (frame.parameters().firstOrNull()?.toInt()?.and(0xFF)) {
                0 -> false
                1 -> true
                else -> null
            }
            if (enabled == null) {
                listener.onStatus("Automatic power-off response was not recognized")
                return@enqueue
            }
            state = state.copy(autoPowerOffEnabled = enabled)
            listener.onState(state)
            if (expected != null && expected != enabled) listener.onStatus("Automatic power-off was not confirmed by device readback")
        }
        pump()
    }

    private fun readPromptToneSwitch(expected: Boolean? = null) {
        enqueue(0xF9) { frame ->
            val value = frame.parameters().firstOrNull()?.toInt()?.and(0xFF)?.let { when (it) { 0 -> false; 1 -> true; else -> null } }
            state = state.copy(promptToneEnabled = value)
            listener.onState(state)
            if (expected != null && value != expected) listener.onStatus("Prompt tone setting did not match device readback")
        }
        pump()
    }

    private fun readPromptVolume(expected: Int? = null) {
        enqueue(0x05) { frame ->
            val level = decodeEdifierPromptLevel(frame.parameters())
            if (level == null) {
                listener.onStatus("Voice prompt volume response was not recognized")
                return@enqueue
            }
            state = state.copy(promptVolumeLevel = level)
            listener.onState(state)
            if (expected != null && expected != level) listener.onStatus("Voice prompt volume did not match device readback")
        }
        pump()
    }

    private fun readGameMode(expected: Boolean? = null) {
        enqueue(0x08) { frame ->
            val enabled = when (frame.parameters().firstOrNull()?.toInt()?.and(0xFF)) {
                0 -> false
                1 -> true
                else -> null
            }
            if (enabled == null) {
                listener.onStatus("Game mode response was not recognized")
                return@enqueue
            }
            state = state.copy(gameModeEnabled = enabled)
            listener.onState(state)
            if (expected != null && expected != enabled) listener.onStatus("Game mode was not confirmed by device readback")
        }
        pump()
    }

    private fun readSleepTimer(expected: Int? = null) {
        enqueue(0xD3) { frame ->
            val bytes = frame.parameters()
            if (bytes.size < 2) {
                listener.onStatus("Sleep timer response was not recognized")
                return@enqueue
            }
            val minutes = ((bytes[0].toInt() and 0xFF) shl 8) or (bytes[1].toInt() and 0xFF)
            if (minutes !in state.sleepTimerOptions) {
                listener.onStatus("Sleep timer readback is outside the firmware options")
                return@enqueue
            }
            state = state.copy(sleepTimerMinutes = minutes)
            listener.onState(state)
            if (expected != null && expected != minutes) listener.onStatus("Sleep timer was not confirmed by device readback")
        }
        pump()
    }

    private fun readEqualizer(expected: Int? = null) {
        enqueue(0xD5) { frame ->
            val selected = frame.parameters().firstOrNull()?.toInt()?.and(0xFF)
            if (selected == null || state.equalizerPresets.none { it.id == selected }) {
                listener.onStatus("EQ response did not match the reported profile")
                return@enqueue
            }
            state = state.copy(selectedEqualizer = selected)
            listener.onState(state)
            if (expected != null && expected != selected) listener.onStatus("EQ preset was not confirmed by device readback")
        }
        pump()
    }

    private fun readNoise(expected: Noise? = null) {
        enqueue(0xCC) { frame ->
            val value = when (frame.parameters().firstOrNull()?.toInt()?.and(0xFF)) {
                1 -> Noise.OFF
                2 -> Noise.ANC
                3 -> Noise.TRANSPARENCY
                else -> null
            }
            if (value == null) {
                listener.onStatus("Noise response was not recognized")
                return@enqueue
            }
            state = state.copy(noise = value)
            listener.onState(state)
            if (expected != null && expected != value) listener.onStatus("Mode was not confirmed by device readback")
        }
        pump()
    }

    private fun enqueue(command: Int, parameters: ByteArray = byteArrayOf(), done: (EdifierFrame) -> Unit = {}) {
        val payload = byteArrayOf(command.toByte()) + parameters
        queue.addLast(Operation(command, EdifierFrameCodec.encodeHost(payload, activeProtocol), done))
    }

    private fun pump() {
        if (!ready || pending != null || queue.isEmpty()) return
        val operation = queue.removeFirst()
        pending = operation
        handler.postDelayed(responseTimeout, RESPONSE_TIMEOUT_MS)
        if (!link.send(operation.packet)) {
            pending = null
            handler.removeCallbacks(responseTimeout)
            listener.onStatus("Could not send a command to the headphones")
            pump()
        }
    }

    private fun closed(reason: String) {
        ready = false
        pending = null
        queue.clear()
        handler.removeCallbacks(responseTimeout)
        listener.onClosed(reason)
    }

    fun close() {
        ready = false
        queue.clear()
        pending = null
        handler.removeCallbacks(responseTimeout)
        link.close()
    }

    private companion object { const val RESPONSE_TIMEOUT_MS = 2_500L }
}
