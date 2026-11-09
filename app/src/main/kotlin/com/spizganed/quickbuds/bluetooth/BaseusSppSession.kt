package com.spizganed.quickbuds.bluetooth

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.os.Handler
import android.os.Looper
import com.spizganed.quickbuds.protocol.baseus.BaseusBowie30Codec
import com.spizganed.quickbuds.protocol.baseus.BaseusFrame
import java.io.InputStream
import java.io.OutputStream
import java.util.UUID
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/** Baseus EP10 Pro / Bowie MF1 RFCOMM profile inside the shared DeviceDeck hub. */
@SuppressLint("MissingPermission")
class BaseusSppSession(
    private val device: BluetoothDevice,
    private val model: Model,
    private val listener: Listener,
) {
    enum class Model { EP10_PRO, BOWIE_MF1 }
    enum class Noise(val wireMode: Int, val wireType: Int) { OFF(0, 0xFF), ANC(1, 101), TRANSPARENCY(2, 0xFF) }
    data class Gesture(val button: Int, val layout: Int, val action: Int)
    data class State(val leftBattery: Int? = null, val rightBattery: Int? = null, val noise: Noise? = null, val gestures: List<Gesture> = emptyList())
    interface Listener {
        fun onState(state: State)
        fun onStatus(text: String)
        fun onReady()
        fun onClosed(reason: String)
    }

    private val main = Handler(Looper.getMainLooper())
    private val io = Executors.newCachedThreadPool()
    private val requests = Executors.newSingleThreadExecutor()
    private val frames = ArrayBlockingQueue<BaseusFrame>(32)
    private val decoder = BaseusBowie30Codec.Decoder()
    private val closed = AtomicBoolean(false)
    @Volatile private var socket: android.bluetooth.BluetoothSocket? = null
    @Volatile private var input: InputStream? = null
    @Volatile private var output: OutputStream? = null
    @Volatile private var ready = false
    @Volatile private var state = State()

    fun connect() {
        io.execute {
            try {
                val link = device.createRfcommSocketToServiceRecord(SPP_UUID)
                socket = link
                link.connect()
                if (closed.get()) { link.close(); return@execute }
                input = link.inputStream
                output = link.outputStream
                ready = true
                main.post { if (!closed.get()) { listener.onStatus("Connected · Bluetooth SPP"); listener.onReady() } }
                io.execute { readLoop() }
                refresh()
            } catch (error: Exception) {
                fail("Could not connect to the paired headset: ${error.message ?: "Bluetooth SPP error"}")
            }
        }
    }

    fun refresh() {
        if (!ready) return
        requests.execute {
            try {
                val battery = request(0x02)
                if (battery.payload.size != 4 || battery.u8(0) !in 0..100 || battery.u8(1) != 0 || battery.u8(2) !in 0..100 || battery.u8(3) != 1)
                    throw IllegalStateException("Unrecognized battery response")
                state = state.copy(leftBattery = battery.u8(0), rightBattery = battery.u8(2))
                publishState()
                if (model == Model.EP10_PRO) {
                    readNoise()
                    runCatching { readGestures() }.onFailure { publishStatus("Gesture settings unavailable: ${it.message ?: "no response"}") }
                }
            } catch (error: Exception) { fail(error.message ?: "Could not read headset status") }
        }
    }

    fun setNoise(mode: Noise) {
        if (!ready || model != Model.EP10_PRO || state.noise == null) return
        requests.execute {
            try {
                val ack = request(0x34, mode.wireMode, mode.wireType)
                if (ack.payload.size != 1 || ack.u8(0) != 1) throw IllegalStateException("The headset rejected the mode")
                readNoise()
                if (state.noise != mode) throw IllegalStateException("Mode did not match device readback")
            } catch (error: Exception) { publishStatus(error.message ?: "Could not change noise mode") }
        }
    }

    fun setGesture(gesture: Gesture, action: Int) {
        if (!ready || model != Model.EP10_PRO || gesture !in state.gestures || action !in allowedActions(gesture.layout)) return
        requests.execute {
            try {
                val ack = request(0x8D, gesture.button, gesture.layout, action)
                if (ack.payload.size != 1 || ack.u8(0) != 1) throw IllegalStateException("The headset rejected the gesture")
                readGestures()
                if (state.gestures.none { it.button == gesture.button && it.layout == gesture.layout && it.action == action })
                    throw IllegalStateException("Gesture did not match device readback")
            } catch (error: Exception) { publishStatus(error.message ?: "Could not change gesture") }
        }
    }

    private fun readGestures() {
        var rows = state.gestures
        for (button in 0..1) {
            val frame = request(0x8C, button, 0xFF)
            if (frame.payload.size != 9 || frame.u8(0) != button) throw IllegalStateException("Unrecognized gesture response")
            rows = rows.filter { it.button != button } + (0 until 4).map { index ->
                val layout = frame.u8(1 + index * 2)
                if (layout !in 0..3) throw IllegalStateException("Unknown gesture layout")
                Gesture(button, layout, frame.u8(2 + index * 2))
            }
        }
        state = state.copy(gestures = rows)
        publishState()
    }

    private fun allowedActions(layout: Int): List<Int> = if (layout == 3) listOf(0, 1) else listOf(0, 1, 2, 3, 4, 6, 11, 12)

    private fun readNoise() {
        val frame = request(0x33)
        if (frame.payload.size != 4) throw IllegalStateException("Unrecognized noise response")
        val mode = when (frame.u8(0)) {
            0 -> Noise.OFF
            1 -> Noise.ANC
            2 -> Noise.TRANSPARENCY
            else -> throw IllegalStateException("Unknown noise mode")
        }
        state = state.copy(noise = mode)
        publishState()
    }

    private fun request(command: Int, vararg payload: Int): BaseusFrame {
        check(ready && !closed.get()) { "Headset disconnected" }
        frames.clear()
        val stream = output ?: error("Bluetooth output is closed")
        stream.write(BaseusBowie30Codec.request(command, *payload))
        stream.flush()
        val deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(RESPONSE_TIMEOUT_MS)
        while (!closed.get()) {
            val remaining = deadline - System.nanoTime()
            if (remaining <= 0) break
            val frame = frames.poll(remaining, TimeUnit.NANOSECONDS) ?: break
            if (frame.command == command) return frame
        }
        throw IllegalStateException("The headset did not answer command 0x${command.toString(16)}")
    }

    private fun readLoop() {
        val buffer = ByteArray(128)
        try {
            while (ready && !closed.get()) {
                val count = input?.read(buffer) ?: -1
                if (count < 0) break
                synchronized(decoder) {
                    decoder.feed(buffer.copyOf(count)).forEach { frames.offer(it) }
                }
            }
            if (!closed.get()) fail("Bluetooth headset disconnected")
        } catch (error: Exception) {
            if (!closed.get()) fail("Bluetooth SPP error: ${error.message ?: "connection closed"}")
        }
    }

    private fun publishState() { val value = state; main.post { if (!closed.get()) listener.onState(value) } }
    private fun publishStatus(text: String) { main.post { if (!closed.get()) listener.onStatus(text) } }

    private fun fail(reason: String) {
        if (!closed.compareAndSet(false, true)) return
        ready = false
        runCatching { socket?.close() }
        requests.shutdownNow(); io.shutdownNow()
        main.post { listener.onClosed(reason) }
    }

    fun close() {
        if (!closed.compareAndSet(false, true)) return
        ready = false
        runCatching { socket?.close() }
        requests.shutdownNow(); io.shutdownNow()
    }

    private fun BaseusFrame.u8(index: Int): Int = payload[index].toInt() and 0xFF

    private companion object {
        val SPP_UUID: UUID = UUID.fromString("00001101-0000-1000-8000-00805F9B34FB")
        const val RESPONSE_TIMEOUT_MS = 2_500L
    }
}
