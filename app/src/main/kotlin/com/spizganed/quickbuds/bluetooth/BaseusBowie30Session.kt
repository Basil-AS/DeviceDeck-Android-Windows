package com.spizganed.quickbuds.bluetooth

import android.bluetooth.BluetoothDevice
import android.content.Context
import android.os.Handler
import android.os.Looper
import com.spizganed.quickbuds.protocol.baseus.BaseusBowie30Codec
import com.spizganed.quickbuds.protocol.baseus.BaseusFrame
import java.util.ArrayDeque

/** Bowie 30 control session over the shared DeviceDeck GATT link. */
class BaseusBowie30Session(
    context: Context,
    device: BluetoothDevice,
    private val listener: Listener,
) {
    data class Gesture(val button: Int, val layout: Int, val action: Int)
    data class State(val leftBattery: Int? = null, val rightBattery: Int? = null, val noise: Noise? = null, val gestures: List<Gesture> = emptyList())
    enum class Noise(val wireMode: Int, val wireType: Int, val title: String) {
        OFF(0, 0xFF, "Off"),
        COMMUTE(1, 8, "Commute"),
        INDOOR(1, 10, "Indoor"),
        OUTDOOR(1, 9, "Outdoor"),
        TRANSPARENCY(2, 0xFF, "Transparency"),
    }

    interface Listener {
        fun onState(state: State)
        fun onStatus(text: String)
        fun onReady()
        fun onClosed(reason: String)
    }

    private data class Operation(val packet: ByteArray, val responseCommand: Int, val done: (BaseusFrame) -> Unit)
    private val handler = Handler(Looper.getMainLooper())
    private val decoder = BaseusBowie30Codec.Decoder()
    private val queue = ArrayDeque<Operation>()
    private var pending: Operation? = null
    private var ready = false
    private var state = State()
    private val responseTimeout = Runnable {
        close()
        listener.onClosed("The headset did not answer. Reconnect and try again.")
    }
    private val link = BleGattLink.connect(context, device, DeviceGattProfiles.baseusShared, object : BleGattLink.Listener {
        override fun onReady(mtu: Int) {
            ready = true
            listener.onReady()
            listener.onStatus("Connected · BLE MTU $mtu")
            refresh()
        }

        override fun onBytes(bytes: ByteArray) {
            decoder.feed(bytes).forEach { frame ->
                if (frame.command != pending?.responseCommand) return@forEach
                val operation = pending ?: return@forEach
                pending = null
                handler.removeCallbacks(responseTimeout)
                operation.done(frame)
                pump()
            }
        }

        override fun onDisconnected(status: Int) = closed("Disconnected")
        override fun onError(message: String) = closed(message)
    })

    fun refresh() {
        if (!ready) return
        enqueue(0x02, response = 0x02) { battery ->
            if (battery.payload.size != 4 || battery.u8(0) > 100 || battery.u8(1) != 0 || battery.u8(2) > 100 || battery.u8(3) != 1) {
                listener.onStatus("Battery response was not recognized")
                return@enqueue
            }
            state = state.copy(leftBattery = battery.u8(0), rightBattery = battery.u8(2))
            listener.onState(state)
            readNoise { readLegacyGestures() }
        }
    }

    fun setNoise(mode: Noise) {
        if (!ready || state.noise == null) {
            listener.onStatus("Read current noise mode before changing it")
            return
        }
        enqueue(0x34, mode.wireMode, mode.wireType, response = 0x34) { acknowledgement ->
            if (acknowledgement.payload.size != 1 || acknowledgement.u8(0) != 1) {
                listener.onStatus("The headset rejected the noise mode")
                return@enqueue
            }
            readNoise(expected = mode)
        }
    }

    private fun readNoise(expected: Noise? = null, done: () -> Unit = {}) {
        enqueue(0x33, response = 0x33) { frame ->
            if (frame.payload.size != 4) {
                listener.onStatus("Noise response was not recognized")
                return@enqueue
            }
            val value = when (frame.u8(0)) {
                0 -> Noise.OFF
                1 -> when (frame.u8(1)) {
                    8 -> Noise.COMMUTE
                    10 -> Noise.INDOOR
                    9 -> Noise.OUTDOOR
                    else -> null
                }
                2 -> Noise.TRANSPARENCY
                else -> null
            }
            if (value == null) {
                listener.onStatus("The headset returned an unknown noise mode")
                return@enqueue
            }
            state = state.copy(noise = value)
            listener.onState(state)
            if (expected != null && value != expected) listener.onStatus("Mode was not confirmed by device readback")
            done()
        }
    }

    private fun readLegacyGestures() {
        listOf(0, 2, 1).forEach { layout ->
            enqueue(0x21, layout, response = 0x21) { frame ->
                if (frame.payload.size != 3 || frame.u8(0) != layout) {
                    listener.onStatus("Gesture response was not recognized")
                    return@enqueue
                }
                val rows = state.gestures.filterNot { it.layout == layout } + listOf(
                    Gesture(0, layout, frame.u8(1)), Gesture(1, layout, frame.u8(2)),
                )
                state = state.copy(gestures = rows)
                listener.onState(state)
            }
        }
    }

    fun setGesture(gesture: Gesture, action: Int) {
        if (!ready || gesture !in state.gestures || action !in listOf(0, 1, 2, 3, 4, 6)) return
        val current = state.gestures.filter { it.layout == gesture.layout }
        val left = current.firstOrNull { it.button == 0 }?.action ?: return
        val right = current.firstOrNull { it.button == 1 }?.action ?: return
        val layout = gesture.layout
        enqueue(0x22, layout, if (gesture.button == 0) action else left,
            if (gesture.button == 1) action else right, response = 0x22) { ack ->
            if (ack.payload.size != 1 || ack.u8(0) != 1) {
                listener.onStatus("The headset rejected the gesture")
                return@enqueue
            }
            enqueue(0x21, layout, response = 0x21) { frame ->
                if (frame.payload.size != 3 || frame.u8(0) != layout ||
                    frame.u8(gesture.button + 1) != action) {
                    listener.onStatus("Gesture change was not confirmed by device readback")
                    return@enqueue
                }
                val rows = state.gestures.filterNot { it.layout == layout } + listOf(
                    Gesture(0, layout, frame.u8(1)), Gesture(1, layout, frame.u8(2)),
                )
                state = state.copy(gestures = rows)
                listener.onState(state)
            }
        }
    }

    private fun enqueue(command: Int, vararg args: Int, response: Int, done: (BaseusFrame) -> Unit) {
        queue.addLast(Operation(BaseusBowie30Codec.request(command, *args), response, done))
        pump()
    }

    private fun pump() {
        if (!ready || pending != null || queue.isEmpty()) return
        val operation = queue.removeFirst()
        pending = operation
        handler.postDelayed(responseTimeout, RESPONSE_TIMEOUT_MS)
        if (!link.send(operation.packet)) {
            pending = null
            handler.removeCallbacks(responseTimeout)
            listener.onStatus("Could not send a command to the headset")
            pump()
        }
    }

    private fun closed(reason: String) {
        if (!ready && pending == null) {
            listener.onClosed(reason)
            return
        }
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
