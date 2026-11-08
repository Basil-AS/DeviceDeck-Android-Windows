package com.spizganed.quickbuds.bluetooth

import android.bluetooth.BluetoothDevice
import android.content.Context
import android.os.Handler
import android.os.Looper
import com.spizganed.quickbuds.protocol.edifier.EdifierFrame
import com.spizganed.quickbuds.protocol.edifier.EdifierFrameCodec
import com.spizganed.quickbuds.protocol.edifier.EdifierFrameDecoder
import java.util.ArrayDeque

/** W820NB Plus command session over DeviceDeck's shared GATT link. */
class EdifierW820NbPlusSession(
    context: Context,
    device: BluetoothDevice,
    private val listener: Listener,
) {
    enum class Noise(val wireValue: Int, val title: String) {
        OFF(1, "Off"), ANC(2, "Noise cancelling"), TRANSPARENCY(3, "Ambient sound"),
    }

    data class State(val batteryPercent: Int? = null, val noise: Noise? = null)

    interface Listener {
        fun onState(state: State)
        fun onStatus(text: String)
        fun onReady()
        fun onClosed(reason: String)
    }

    private data class Operation(val command: Int, val packet: ByteArray, val done: (EdifierFrame) -> Unit)

    private val handler = Handler(Looper.getMainLooper())
    private val decoder = EdifierFrameDecoder()
    private val queue = ArrayDeque<Operation>()
    private var pending: Operation? = null
    private var ready = false
    private var state = State()
    private val responseTimeout = Runnable {
        val timedOut = pending
        pending = null
        if (timedOut?.command == 0xD8) {
            // D8 is optional on older firmware. Continue with independently known
            // battery/noise queries instead of making the whole session unusable.
            listener.onStatus("Capability probe was not answered; trying basic controls")
            refresh()
            pump()
        } else {
            close()
            listener.onClosed("The headphones did not answer. Reconnect and try again.")
        }
    }
    private val link = BleGattLink.connect(context, device, DeviceGattProfiles.edifierW820NbPlus, object : BleGattLink.Listener {
        override fun onReady(mtu: Int) {
            ready = true
            listener.onStatus("Connected · BLE MTU $mtu")
            // D8 is a capability probe. Its layout is firmware-sensitive; retain it as
            // evidence but do not enable commands from an unvalidated interpretation.
            enqueue(0xD8) { frame ->
                listener.onStatus("Capabilities received · ${frame.parameters().size} bytes")
                refresh()
            }
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
        queue.addLast(Operation(command, EdifierFrameCodec.encodeHost(payload), done))
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
