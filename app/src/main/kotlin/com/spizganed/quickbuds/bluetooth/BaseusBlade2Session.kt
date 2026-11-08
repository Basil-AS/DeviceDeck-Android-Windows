package com.spizganed.quickbuds.bluetooth

import android.bluetooth.BluetoothDevice
import android.content.Context
import android.os.Handler
import android.os.Looper
import com.spizganed.quickbuds.protocol.baseus.BaseusMobilePowerCodec
import java.util.ArrayDeque

/** Blade 2 power-device session inside DeviceDeck's shared accessory hub. */
class BaseusBlade2Session(context: Context, private val device: BluetoothDevice, private val listener: Listener) {
    data class State(val values: Map<Int, BaseusMobilePowerCodec.Status> = emptyMap())

    interface Listener {
        fun onState(state: State)
        fun onStatus(text: String)
        fun onReady()
        fun onClosed(reason: String)
    }

    private data class Operation(val packet: ByteArray, val responseCode: Int?, val done: (BaseusMobilePowerCodec.Status?) -> Unit)
    private val handler = Handler(Looper.getMainLooper())
    private val decoder = BaseusMobilePowerCodec.Decoder()
    private val queue = ArrayDeque<Operation>()
    private var pending: Operation? = null
    private var ready = false
    private var state = State()
    private val profile = DeviceGattProfiles.baseusBlade2
    private val isBlade2_140 = try {
        device.name?.contains("140", ignoreCase = true) == true
    } catch (_: SecurityException) {
        // If Android withholds BLUETOOTH_CONNECT, don't infer the 140-only feature set.
        false
    }
    private val link = BleGattLink.connect(context, device, profile, object : BleGattLink.Listener {
        override fun onReady(mtu: Int) {
            ready = true
            listener.onReady()
            listener.onStatus("Connected · BLE MTU $mtu")
            refresh()
        }

        override fun onBytes(bytes: ByteArray) {
            decoder.feed(bytes).forEach { value ->
                val operation = pending ?: return@forEach
                if (value.code != operation.responseCode) return@forEach
                pending = null
                handler.removeCallbacks(operationTimeout)
                operation.done(value)
                pump()
            }
        }

        override fun onWriteComplete(bytes: ByteArray, status: Int) {
            val operation = pending ?: return
            if (operation.responseCode != null || !operation.packet.contentEquals(bytes)) return
            if (status != android.bluetooth.BluetoothGatt.GATT_SUCCESS) {
                closed("GATT write failed (status $status)")
                return
            }
            // Command writes are followed by a separate readback when the app exposes one.
            finishWriteAfterDelay(operation)
        }

        override fun onDisconnected(status: Int) = closed("Disconnected")
        override fun onError(message: String) = closed(message)
    })

    private val operationTimeout = Runnable {
        close()
        listener.onClosed("Blade 2 did not answer. Reconnect and try again.")
    }

    private val writeSettle = Runnable {
        val operation = pending ?: return@Runnable
        if (operation.responseCode == null) {
            pending = null
            handler.removeCallbacks(operationTimeout)
            operation.done(null)
            pump()
        }
    }

    fun refresh() {
        if (!ready) return
        val codes = mutableListOf<Int>()
        codes += 0x0001..0x000D
        codes += listOf(0x0010, 0x0011, 0x0012, 0x0013, 0x0014, 0x0015, 0x0016, 0x001E, 0x001F, 0x0020, 0x0022, 0x0023)
        // The app requests these only on its Blade 2-140 branch; don't probe them
        // on the regular Blade 2 profile by inference.
        if (isBlade2_140) {
            codes += listOf(0x0024, 0x0025, 0x0026)
            codes += 0x0028..0x002D
            codes += 0x0031..0x0036
        }
        codes.forEach(::read)
    }

    fun setTheme(index: Int) {
        if (index !in 0..2) { listener.onStatus("Theme must be one of the three vendor presets"); return }
        writeAndRead(0x0020, index)
    }

    fun setScreenSaver(enabled: Boolean) = writeAndRead(0x0022, if (enabled) 1 else 0)

    fun setCountdown(code: Int, minutes: Int) {
        if (code !in setOf(0x001E, 0x001F) || minutes !in 0..1440) {
            listener.onStatus("Timer must be from 0 to 1440 minutes")
            return
        }
        writeAndRead(code, minutes)
    }

    fun setScreenText(text: String) {
        if (!ready) return
        val packet = runCatching { BaseusMobilePowerCodec.writeText(text) }.getOrElse {
            listener.onStatus(it.message ?: "Invalid screen text")
            return
        }
        enqueue(Operation(packet, null) {
            listener.onStatus("Text command sent; the vendor protocol provides no confirmed text readback here")
        })
    }

    private fun writeAndRead(code: Int, value: Int) {
        if (!ready) return
        enqueue(Operation(BaseusMobilePowerCodec.writeU16(code, value), null) {
            read(code) { result ->
                if (result?.value == value) listener.onStatus("Setting confirmed by device readback")
                else listener.onStatus("Device readback did not confirm the requested value")
            }
        })
    }

    private fun read(code: Int, result: ((BaseusMobilePowerCodec.Status?) -> Unit)? = null) {
        if (!ready) return
        enqueue(Operation(BaseusMobilePowerCodec.read(code), code) { value ->
            if (value != null) {
                state = state.copy(values = state.values + (code to value))
                listener.onState(state)
            }
            result?.invoke(value)
        })
    }

    private fun enqueue(operation: Operation) {
        queue.addLast(operation)
        pump()
    }

    private fun pump() {
        if (!ready || pending != null || queue.isEmpty()) return
        val operation = queue.removeFirst()
        pending = operation
        handler.postDelayed(operationTimeout, RESPONSE_TIMEOUT_MS)
        if (!link.send(operation.packet)) {
            pending = null
            handler.removeCallbacks(operationTimeout)
            listener.onStatus("Could not send a command to Blade 2")
            pump()
        } else if (operation.responseCode == null) {
            // The module may acknowledge over GATT or remain silent; the timer keeps
            // command ordering deterministic and the UI reports text as unconfirmed.
            handler.postDelayed(writeSettle, WRITE_SETTLE_MS)
        }
    }

    private fun finishWriteAfterDelay(operation: Operation) {
        handler.removeCallbacks(writeSettle)
        handler.removeCallbacks(operationTimeout)
        pending = null
        operation.done(null)
        pump()
    }

    private fun closed(reason: String) {
        val wasActive = ready || pending != null
        ready = false
        pending = null
        queue.clear()
        handler.removeCallbacks(operationTimeout)
        handler.removeCallbacks(writeSettle)
        decoder.reset()
        if (wasActive) listener.onClosed(reason)
    }

    fun close() {
        ready = false
        pending = null
        queue.clear()
        handler.removeCallbacks(operationTimeout)
        handler.removeCallbacks(writeSettle)
        decoder.reset()
        link.close()
    }

    private companion object {
        const val RESPONSE_TIMEOUT_MS = 4_000L
        const val WRITE_SETTLE_MS = 450L
    }
}
