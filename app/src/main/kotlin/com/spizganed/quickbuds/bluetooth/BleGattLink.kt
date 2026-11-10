package com.spizganed.quickbuds.bluetooth

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.BluetoothStatusCodes
import android.bluetooth.le.BluetoothLeScanner
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import java.util.ArrayDeque
import java.util.UUID

/**
 * Small GATT byte link for device-specific sessions. This deliberately owns no
 * command semantics: profiles and protocol drivers decide what bytes mean.
 * Writes are serialized and notifications are delivered only after the CCCD is
 * enabled successfully.
 */
@SuppressLint("MissingPermission")
class BleGattLink private constructor(
    context: Context,
    private val device: BluetoothDevice,
    private val profile: DeviceGattProfile,
    private val listener: Listener,
) : BluetoothGattCallback() {
    interface Listener {
        fun onReady(mtu: Int)
        fun onBytes(bytes: ByteArray)
        fun onWriteComplete(bytes: ByteArray, status: Int) {}
        fun onDisconnected(status: Int) {}
        fun onError(message: String)
    }

    private enum class State { CONNECTING, DISCOVERING, ENABLING_NOTIFICATIONS, READY, CLOSED }
    private val appContext = context.applicationContext
    private val main = Handler(Looper.getMainLooper())
    private val writeQueue = ArrayDeque<ByteArray>()
    @Volatile private var state = State.CONNECTING
    private var gatt: BluetoothGatt? = null
    private var writeCharacteristic: BluetoothGattCharacteristic? = null
    private var notifyCharacteristic: BluetoothGattCharacteristic? = null
    private var mtu = 23
    private var writePending = false
    private var discoveryStarted = false
    private var connectTimeout: Runnable? = null
    private var mtuFallback: Runnable? = null

    init {
        connectTimeout = Runnable { fail("GATT connection timed out") }
        main.postDelayed(connectTimeout!!, CONNECT_TIMEOUT_MS)
        try {
            gatt = device.connectGatt(appContext, false, this, BluetoothDevice.TRANSPORT_LE, BluetoothDevice.PHY_LE_1M_MASK, main)
            if (gatt == null) fail("Could not create GATT connection")
        } catch (error: SecurityException) {
            fail("Bluetooth permission denied: ${error.message.orEmpty()}")
        } catch (error: RuntimeException) {
            fail("Could not connect over Bluetooth LE: ${error.message.orEmpty()}")
        }
    }

    /** Enqueue one complete protocol packet. GATT writes are never split implicitly. */
    fun send(bytes: ByteArray): Boolean {
        if (state != State.READY) return false
        if (bytes.isEmpty() || bytes.size > mtu - ATT_OVERHEAD) {
            postError("GATT packet must contain 1..${mtu - ATT_OVERHEAD} bytes")
            return false
        }
        val packet = bytes.copyOf()
        main.post {
            if (state != State.READY) postError("GATT link closed before the packet could be written")
            else { writeQueue.addLast(packet); writeNext() }
        }
        return true
    }

    fun close() {
        if (Looper.myLooper() != Looper.getMainLooper()) { main.post { close() }; return }
        if (state == State.CLOSED) return
        state = State.CLOSED
        cancelTimers()
        writeQueue.clear()
        writePending = false
        val link = gatt
        gatt = null
        writeCharacteristic = null
        notifyCharacteristic = null
        runCatching { link?.disconnect() }
        runCatching { link?.close() }
    }

    override fun onConnectionStateChange(gatt: BluetoothGatt, status: Int, newState: Int) {
        if (this.gatt !== gatt || state == State.CLOSED) return
        if (status != BluetoothGatt.GATT_SUCCESS) {
            fail("GATT connection failed (status $status)")
            return
        }
        when (newState) {
            BluetoothProfile.STATE_CONNECTED -> {
                state = State.DISCOVERING
                requestMtuOrDiscover(gatt)
            }
            BluetoothProfile.STATE_DISCONNECTED -> {
                cancelTimers()
                state = State.CLOSED
                runCatching { gatt.close() }
                this.gatt = null
                post { listener.onDisconnected(status) }
            }
        }
    }

    override fun onMtuChanged(gatt: BluetoothGatt, mtu: Int, status: Int) {
        if (this.gatt !== gatt || state != State.DISCOVERING || discoveryStarted) return
        if (status == BluetoothGatt.GATT_SUCCESS && mtu >= MIN_MTU) this.mtu = mtu
        discover(gatt)
    }

    override fun onServicesDiscovered(gatt: BluetoothGatt, status: Int) {
        if (this.gatt !== gatt || state != State.DISCOVERING) return
        if (status != BluetoothGatt.GATT_SUCCESS) {
            fail("GATT service discovery failed (status $status)")
            return
        }
        val service = gatt.getService(profile.service)
        val writer = service?.getCharacteristic(profile.write)
        val notifier = service?.getCharacteristic(profile.notify)
        if (writer == null || notifier == null) {
            fail("The device did not expose the expected control characteristics")
            return
        }
        val canWrite = writer.properties and (BluetoothGattCharacteristic.PROPERTY_WRITE or BluetoothGattCharacteristic.PROPERTY_WRITE_NO_RESPONSE) != 0
        val canNotify = notifier.properties and (BluetoothGattCharacteristic.PROPERTY_NOTIFY or BluetoothGattCharacteristic.PROPERTY_INDICATE) != 0
        if (!canWrite || !canNotify) {
            fail("The control characteristics have incompatible GATT properties")
            return
        }
        writeCharacteristic = writer
        notifyCharacteristic = notifier
        if (!gatt.setCharacteristicNotification(notifier, true)) {
            fail("Could not enable local GATT notifications")
            return
        }
        val cccd = notifier.getDescriptor(CCCD_UUID)
        if (cccd == null) {
            fail("The notification characteristic has no CCCD descriptor")
            return
        }
        val value = if (notifier.properties and BluetoothGattCharacteristic.PROPERTY_NOTIFY != 0)
            BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE else BluetoothGattDescriptor.ENABLE_INDICATION_VALUE
        state = State.ENABLING_NOTIFICATIONS
        val started = if (Build.VERSION.SDK_INT >= 33) {
            gatt.writeDescriptor(cccd, value) == BluetoothStatusCodes.SUCCESS
        } else {
            @Suppress("DEPRECATION")
            cccd.value = value
            @Suppress("DEPRECATION")
            gatt.writeDescriptor(cccd)
        }
        if (!started) fail("Could not start GATT notification setup")
    }

    override fun onDescriptorWrite(gatt: BluetoothGatt, descriptor: BluetoothGattDescriptor, status: Int) {
        if (this.gatt !== gatt || state != State.ENABLING_NOTIFICATIONS || descriptor.uuid != CCCD_UUID) return
        if (status != BluetoothGatt.GATT_SUCCESS) {
            fail("GATT notification setup failed (status $status)")
            return
        }
        state = State.READY
        cancelTimers()
        post { listener.onReady(mtu) }
    }

    @Deprecated("Use the value overload on API 33 and newer")
    override fun onCharacteristicChanged(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic) {
        @Suppress("DEPRECATION")
        val value = characteristic.value?.copyOf() ?: return
        receive(gatt, characteristic, value)
    }

    override fun onCharacteristicChanged(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic, value: ByteArray) {
        receive(gatt, characteristic, value.copyOf())
    }

    private fun receive(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic, bytes: ByteArray) {
        if (this.gatt !== gatt || state != State.READY || characteristic.uuid != profile.notify || bytes.isEmpty()) return
        post { listener.onBytes(bytes) }
    }

    override fun onCharacteristicWrite(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic, status: Int) {
        if (this.gatt !== gatt || state != State.READY || characteristic.uuid != profile.write || !writePending) return
        writePending = false
        val completed = writeQueue.pollFirst()
        if (completed != null) post { listener.onWriteComplete(completed, status) }
        if (status != BluetoothGatt.GATT_SUCCESS) {
            fail("GATT write failed (status $status)")
            return
        }
        writeNext()
    }

    private fun requestMtuOrDiscover(gatt: BluetoothGatt) {
        mtuFallback = Runnable { discover(gatt) }
        main.postDelayed(mtuFallback!!, MTU_TIMEOUT_MS)
        val requested = runCatching { gatt.requestMtu(REQUESTED_MTU) }.getOrDefault(false)
        if (!requested) discover(gatt)
    }

    private fun discover(gatt: BluetoothGatt) {
        if (this.gatt !== gatt || state != State.DISCOVERING || discoveryStarted) return
        discoveryStarted = true
        mtuFallback?.let(main::removeCallbacks)
        mtuFallback = null
        if (!gatt.discoverServices()) fail("Could not start GATT service discovery")
    }

    private fun writeNext() {
        if (state != State.READY || writePending || writeQueue.isEmpty()) return
        val link = gatt ?: return
        val characteristic = writeCharacteristic ?: return
        val bytes = writeQueue.first()
        val writeType = if (characteristic.properties and BluetoothGattCharacteristic.PROPERTY_WRITE != 0)
            BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT else BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE
        writePending = true
        val started = if (Build.VERSION.SDK_INT >= 33) {
            link.writeCharacteristic(characteristic, bytes, writeType) == BluetoothStatusCodes.SUCCESS
        } else {
            @Suppress("DEPRECATION")
            characteristic.writeType = writeType
            @Suppress("DEPRECATION")
            characteristic.value = bytes
            @Suppress("DEPRECATION")
            link.writeCharacteristic(characteristic)
        }
        if (!started) {
            writePending = false
            fail("Could not start GATT write")
        }
    }

    private fun fail(message: String) {
        if (state == State.CLOSED) return
        state = State.CLOSED
        cancelTimers()
        writeQueue.clear()
        writePending = false
        val link = gatt
        gatt = null
        writeCharacteristic = null
        notifyCharacteristic = null
        runCatching { link?.disconnect() }
        runCatching { link?.close() }
        post { listener.onError(message) }
    }

    private fun cancelTimers() {
        connectTimeout?.let(main::removeCallbacks)
        mtuFallback?.let(main::removeCallbacks)
        connectTimeout = null
        mtuFallback = null
    }

    private fun postError(message: String) = post { listener.onError(message) }
    private fun post(callback: () -> Unit) { if (Looper.myLooper() == Looper.getMainLooper()) callback() else main.post(callback) }

    companion object {
        private val CCCD_UUID: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
        private const val CONNECT_TIMEOUT_MS = 15_000L
        private const val MTU_TIMEOUT_MS = 1_500L
        private const val REQUESTED_MTU = 247
        private const val MIN_MTU = 23
        private const val ATT_OVERHEAD = 3

        fun connect(context: Context, device: BluetoothDevice, profile: DeviceGattProfile, listener: Listener): BleGattLink =
            BleGattLink(context, device, profile, listener)
    }
}

/** Service-UUID filtered discovery for DeviceDeck profiles. */
@SuppressLint("MissingPermission")
class BleGattScanner private constructor(
    context: Context,
    private val profile: DeviceGattProfile,
    private val listener: Listener,
) {
    interface Listener {
        fun onDevice(device: BluetoothDevice, profile: DeviceGattProfile)
        fun onScanError(message: String)
    }

    private val appContext = context.applicationContext
    private val handler = Handler(Looper.getMainLooper())
    private val seen = HashSet<String>()
    private var scanner: BluetoothLeScanner? = null
    private var running = false
    private val timeout = Runnable { stop() }
    private val callback = object : ScanCallback() {
        override fun onScanResult(callbackType: Int, result: ScanResult) {
            val device = result.device ?: return
            val advertisedName = result.scanRecord?.deviceName ?: runCatching { device.name }.getOrNull()
            if (!profile.matchesName(advertisedName)) return
            handler.post {
                if (running && seen.add(device.address)) listener.onDevice(device, profile)
            }
        }

        override fun onScanFailed(errorCode: Int) {
            handler.post {
                stop()
                listener.onScanError("Bluetooth LE scan failed (code $errorCode)")
            }
        }
    }

    fun start(): Boolean {
        if (running) return true
        val adapter = appContext.getSystemService(BluetoothManager::class.java)?.adapter
        val bleScanner = adapter?.bluetoothLeScanner
        if (adapter?.isEnabled != true || bleScanner == null) {
            listener.onScanError("Bluetooth LE is unavailable or turned off")
            return false
        }
        return try {
            scanner = bleScanner
            seen.clear()
            val filters = profile.discoveryService?.let { uuid ->
                listOf(ScanFilter.Builder().setServiceUuid(android.os.ParcelUuid(uuid)).build())
            }
            bleScanner.startScan(
                filters,
                ScanSettings.Builder().setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY).build(),
                callback,
            )
            running = true
            handler.postDelayed(timeout, SCAN_TIMEOUT_MS)
            true
        } catch (error: SecurityException) {
            listener.onScanError("Bluetooth scan permission denied: ${error.message.orEmpty()}")
            false
        } catch (error: RuntimeException) {
            listener.onScanError("Could not start Bluetooth scan: ${error.message.orEmpty()}")
            false
        }
    }

    fun stop() {
        if (!running) return
        running = false
        handler.removeCallbacks(timeout)
        val current = scanner
        scanner = null
        runCatching { current?.stopScan(callback) }
    }

    companion object {
        private const val SCAN_TIMEOUT_MS = 12_000L
        fun create(context: Context, profile: DeviceGattProfile, listener: Listener) =
            BleGattScanner(context, profile, listener)
    }
}
