package com.spizganed.quickbuds.ui

import android.app.Activity
import android.bluetooth.BluetoothDevice
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import com.spizganed.quickbuds.R
import com.spizganed.quickbuds.bluetooth.BaseusBowie30Session
import com.spizganed.quickbuds.bluetooth.BleGattScanner
import com.spizganed.quickbuds.bluetooth.EdifierW820NbPlusSession
import com.spizganed.quickbuds.bluetooth.DeviceGattProfiles

/** One in-app home for cross-brand accessory sessions. More device profiles are added here. */
class DeviceHubActivity : Activity() {
    private enum class DeviceKind { BASEUS_BOWIE_30, EDIFIER_W820NB_PLUS }
    private val handler = Handler(Looper.getMainLooper())
    private var scanner: BleGattScanner? = null
    private var baseusSession: BaseusBowie30Session? = null
    private var edifierSession: EdifierW820NbPlusSession? = null
    private var selectedDevice = DeviceKind.BASEUS_BOWIE_30
    private var foundDevice: BluetoothDevice? = null
    private var ready = false
    private var currentState = BaseusBowie30Session.State()
    private var edifierState = EdifierW820NbPlusSession.State()
    private lateinit var deviceName: TextView
    private lateinit var status: TextView
    private lateinit var battery: TextView
    private lateinit var noise: TextView
    private lateinit var controls: LinearLayout
    private lateinit var scanButton: TextView
    private lateinit var deviceRow: TextView
    private var seenAny = false

    private val scanTimeout = Runnable {
        if (!seenAny && baseusSession == null && edifierSession == null) status.setText(R.string.device_hub_none)
        scanButton.isEnabled = true
        scanButton.setText(R.string.device_hub_scan)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        ThemeRes.select(this)
        super.onCreate(savedInstanceState)
        val dp = { value: Float -> ThemeRes.dp(this, value) }
        val palette = ThemeRes.palette(this)
        val root = SettingRowFactory.screen(this)
        root.addView(SettingRowFactory.title(this, R.string.device_hub_title))
        root.addView(TextView(this).apply {
            setText(R.string.device_hub_scan_hint)
            textSize = 14f
            setTextColor(palette.textSecondary)
            setPadding(dp(4f), dp(6f), dp(4f), dp(14f))
        })

        val card = SettingRowFactory.card(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(16f), dp(14f), dp(16f), dp(14f))
        }
        deviceName = TextView(this).apply {
            text = "Baseus Bowie 30"
            textSize = 18f
            typeface = ThemeRes.medium(this@DeviceHubActivity)
            setTextColor(palette.text)
        }
        card.addView(deviceName)
        status = TextView(this).apply {
            setText(R.string.device_hub_scan)
            textSize = 14f
            setTextColor(palette.textSecondary)
            setPadding(0, dp(5f), 0, dp(10f))
        }
        card.addView(status)
        battery = TextView(this).apply {
            setText(R.string.device_hub_battery)
            textSize = 15f
            setTextColor(palette.text)
            visibility = View.GONE
        }
        noise = TextView(this).apply {
            textSize = 14f
            setTextColor(palette.textSecondary)
            setPadding(0, dp(6f), 0, dp(8f))
            visibility = View.GONE
        }
        card.addView(battery)
        card.addView(noise)

        controls = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            visibility = View.GONE
        }
        addModes(controls, listOf(BaseusBowie30Session.Noise.OFF, BaseusBowie30Session.Noise.COMMUTE,
            BaseusBowie30Session.Noise.INDOOR, BaseusBowie30Session.Noise.OUTDOOR, BaseusBowie30Session.Noise.TRANSPARENCY))
        card.addView(controls)
        card.addView(TextView(this).apply {
            setText(R.string.device_hub_verified_note)
            textSize = 12f
            setTextColor(palette.textSecondary)
            setPadding(0, dp(12f), 0, 0)
        })
        root.addView(card)

        val devicePicker = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        listOf(DeviceKind.BASEUS_BOWIE_30 to "Bowie 30", DeviceKind.EDIFIER_W820NB_PLUS to "W820NB Plus").forEachIndexed { index, (kind, label) ->
            devicePicker.addView(TextView(this).apply {
                text = label
                textSize = 14f
                gravity = Gravity.CENTER
                setPadding(dp(12f), dp(12f), dp(12f), dp(12f))
                setTextColor(palette.text)
                background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.card(this@DeviceHubActivity, 14f))
                isClickable = true
                isFocusable = true
                setOnClickListener { selectDevice(kind) }
            }, LinearLayout.LayoutParams(0, -2, 1f).apply { if (index > 0) marginStart = dp(8f) })
        }
        root.addView(devicePicker, LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(10f) })

        deviceRow = TextView(this).apply {
            textSize = 15f
            setTextColor(palette.text)
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(16f), dp(14f), dp(16f), dp(14f))
            background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.card(this@DeviceHubActivity, 18f))
            isClickable = true
            isFocusable = true
            visibility = View.GONE
            setOnClickListener { foundDevice?.let(::connect) }
        }
        root.addView(deviceRow, LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(10f) })

        scanButton = TextView(this).apply {
            setText(R.string.device_hub_scan)
            textSize = 15f
            typeface = ThemeRes.medium(this@DeviceHubActivity)
            gravity = Gravity.CENTER
            setTextColor(ThemeRes.onFill(this@DeviceHubActivity))
            background = ThemeRes.ripple(this@DeviceHubActivity,
                ThemeRes.pill(this@DeviceHubActivity, palette.accent, 18f))
            isClickable = true
            isFocusable = true
            setPadding(dp(16f), dp(14f), dp(16f), dp(14f))
            setOnClickListener { startScan() }
        }
        root.addView(scanButton, LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(12f) })

        val scroll = ScrollView(this).apply {
            setBackgroundColor(ThemeRes.color(this@DeviceHubActivity, R.attr.appColorBg))
            addView(root)
        }
        setContentView(scroll)
    }

    private fun addModes(parent: LinearLayout, modes: List<BaseusBowie30Session.Noise>) {
        val dp = { value: Float -> ThemeRes.dp(this, value) }
        modes.chunked(3).forEachIndexed { rowIndex, rowModes ->
            val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
            rowModes.forEachIndexed { index, mode ->
                val label = when (mode) {
                    BaseusBowie30Session.Noise.OFF -> getString(R.string.device_hub_mode_off)
                    BaseusBowie30Session.Noise.COMMUTE -> getString(R.string.device_hub_mode_commute)
                    BaseusBowie30Session.Noise.INDOOR -> getString(R.string.device_hub_mode_indoor)
                    BaseusBowie30Session.Noise.OUTDOOR -> getString(R.string.device_hub_mode_outdoor)
                    BaseusBowie30Session.Noise.TRANSPARENCY -> getString(R.string.device_hub_mode_transparency)
                }
                val p = ThemeRes.palette(this)
                val active = currentState.noise == mode
                row.addView(TextView(this).apply {
                    text = label
                    textSize = 12f
                    typeface = ThemeRes.medium(this@DeviceHubActivity)
                    gravity = Gravity.CENTER
                    setTextColor(if (active) ThemeRes.onFill(this@DeviceHubActivity) else p.text)
                    background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.shape(this@DeviceHubActivity,
                        if (active) p.accent else p.card, if (active) null else p.outline, 14f))
                    isClickable = true
                    isFocusable = true
                    isEnabled = ready && currentState.noise != null && selectedDevice == DeviceKind.BASEUS_BOWIE_30
                    setOnClickListener { baseusSession?.setNoise(mode) }
                }, LinearLayout.LayoutParams(0, dp(44f), 1f).apply {
                    if (index > 0) marginStart = dp(5f)
                    if (rowIndex > 0) topMargin = dp(6f)
                })
            }
            parent.addView(row)
        }
    }

    private fun startScan() {
        scanner?.stop()
        closeSessions()
        foundDevice = null
        ready = false
        seenAny = false
        deviceRow.visibility = View.GONE
        controls.visibility = View.GONE
        battery.visibility = View.GONE
        noise.visibility = View.GONE
        status.setText(R.string.device_hub_scanning)
        scanButton.isEnabled = false
        scanButton.setText(R.string.device_hub_scanning)
        val profile = when (selectedDevice) {
            DeviceKind.BASEUS_BOWIE_30 -> DeviceGattProfiles.baseusShared.copy(
                displayName = "Baseus Bowie 30", acceptedNames = setOf("Baseus Bowie 30", "Bowie 30"))
            DeviceKind.EDIFIER_W820NB_PLUS -> DeviceGattProfiles.edifierW820NbPlus
        }
        scanner = BleGattScanner.create(this, profile, object : BleGattScanner.Listener {
            override fun onDevice(device: BluetoothDevice, profile: com.spizganed.quickbuds.bluetooth.DeviceGattProfile) {
                if (foundDevice != null) return
                seenAny = true
                foundDevice = device
                deviceRow.text = "${profile.displayName}  ·  ${getString(R.string.conn_action_connect)}"
                deviceRow.visibility = View.VISIBLE
                status.text = "Device found · tap its row to connect"
                scanner?.stop()
                scanButton.isEnabled = true
                scanButton.setText(R.string.device_hub_scan)
                handler.removeCallbacks(scanTimeout)
            }

            override fun onScanError(message: String) {
                status.text = message
                scanButton.isEnabled = true
                scanButton.setText(R.string.device_hub_scan)
            }
        })
        if (scanner?.start() == true) handler.postDelayed(scanTimeout, 12_500L)
        else scanButton.isEnabled = true
    }

    private fun connect(device: BluetoothDevice) {
        scanner?.stop()
        closeSessions()
        ready = false
        deviceRow.visibility = View.GONE
        status.text = "Connecting…"
        scanButton.isEnabled = false
        scanButton.setText(R.string.device_hub_scanning)
        if (selectedDevice == DeviceKind.BASEUS_BOWIE_30) connectBaseus(device) else connectEdifier(device)
    }

    private fun connectBaseus(device: BluetoothDevice) {
        baseusSession = BaseusBowie30Session(this, device, object : BaseusBowie30Session.Listener {
            override fun onState(state: BaseusBowie30Session.State) {
                currentState = state
                battery.text = state.leftBattery?.let { "${getString(R.string.device_hub_battery)}  ·  L $it%   R ${state.rightBattery ?: "—"}%" }
                    ?: getString(R.string.device_hub_battery)
                battery.visibility = if (state.leftBattery == null) View.GONE else View.VISIBLE
                noise.text = "Noise control  ·  ${state.noise?.let(::noiseLabel) ?: "—"}"
                noise.visibility = if (state.noise == null) View.GONE else View.VISIBLE
                renderModes()
            }
            override fun onStatus(text: String) { status.text = text }
            override fun onReady() = connected()
            override fun onClosed(reason: String) = disconnected(reason)
        })
    }

    private fun connectEdifier(device: BluetoothDevice) {
        edifierSession = EdifierW820NbPlusSession(this, device, object : EdifierW820NbPlusSession.Listener {
            override fun onState(state: EdifierW820NbPlusSession.State) {
                edifierState = state
                battery.text = state.batteryPercent?.let { "${getString(R.string.device_hub_battery)}  ·  $it%" } ?: getString(R.string.device_hub_battery)
                battery.visibility = if (state.batteryPercent == null) View.GONE else View.VISIBLE
                noise.text = "Noise control  ·  ${state.noise?.title ?: "—"}"
                noise.visibility = if (state.noise == null) View.GONE else View.VISIBLE
                renderModes()
            }
            override fun onStatus(text: String) { status.text = text }
            override fun onReady() = connected()
            override fun onClosed(reason: String) = disconnected(reason)
        })
    }

    private fun connected() {
        ready = true
        controls.visibility = View.VISIBLE
        status.setText(R.string.device_hub_ready)
        scanButton.isEnabled = true
        scanButton.setText(R.string.device_hub_refresh)
        scanButton.setOnClickListener { baseusSession?.refresh() ?: edifierSession?.refresh() }
        renderModes()
    }

    private fun disconnected(reason: String) {
        ready = false
        controls.visibility = View.GONE
        status.text = reason
        scanButton.isEnabled = true
        scanButton.setText(R.string.device_hub_scan)
        scanButton.setOnClickListener { startScan() }
        renderModes()
    }

    private fun closeSessions() {
        baseusSession?.close(); baseusSession = null
        edifierSession?.close(); edifierSession = null
    }

    private fun selectDevice(kind: DeviceKind) {
        if (selectedDevice == kind) return
        scanner?.stop(); closeSessions()
        selectedDevice = kind
        foundDevice = null; ready = false
        currentState = BaseusBowie30Session.State(); edifierState = EdifierW820NbPlusSession.State()
        deviceName.text = if (kind == DeviceKind.BASEUS_BOWIE_30) "Baseus Bowie 30" else "EDIFIER W820NB Plus"
        deviceRow.visibility = View.GONE; controls.removeAllViews(); controls.visibility = View.GONE
        if (kind == DeviceKind.BASEUS_BOWIE_30) addModes(controls, BaseusBowie30Session.Noise.entries)
        else addEdifierModes()
        battery.visibility = View.GONE; noise.visibility = View.GONE
        status.setText(R.string.device_hub_scan)
        scanButton.setText(R.string.device_hub_scan); scanButton.setOnClickListener { startScan() }
        renderModes()
    }

    private fun addEdifierModes() {
        val p = ThemeRes.palette(this)
        EdifierW820NbPlusSession.Noise.entries.forEachIndexed { index, mode ->
            controls.addView(TextView(this).apply {
                text = mode.title; textSize = 13f; gravity = Gravity.CENTER
                setPadding(ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f), ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f))
                setTextColor(p.text); isClickable = true; isFocusable = true
                background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.card(this@DeviceHubActivity, 14f))
                setOnClickListener { edifierSession?.setNoise(mode) }
            }, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, -2).apply {
                if (index > 0) topMargin = ThemeRes.dp(this@DeviceHubActivity, 6f)
            })
        }
    }

    private fun noiseLabel(mode: BaseusBowie30Session.Noise): String = when (mode) {
        BaseusBowie30Session.Noise.OFF -> getString(R.string.device_hub_mode_off)
        BaseusBowie30Session.Noise.COMMUTE -> getString(R.string.device_hub_mode_commute)
        BaseusBowie30Session.Noise.INDOOR -> getString(R.string.device_hub_mode_indoor)
        BaseusBowie30Session.Noise.OUTDOOR -> getString(R.string.device_hub_mode_outdoor)
        BaseusBowie30Session.Noise.TRANSPARENCY -> getString(R.string.device_hub_mode_transparency)
    }

    private fun renderModes() {
        if (!::controls.isInitialized) return
        if (selectedDevice == DeviceKind.EDIFIER_W820NB_PLUS) {
            for (index in 0 until controls.childCount) {
                val chip = controls.getChildAt(index) as? TextView ?: continue
                val mode = EdifierW820NbPlusSession.Noise.entries.getOrNull(index) ?: continue
                val palette = ThemeRes.palette(this)
                val selected = edifierState.noise == mode
                chip.setTextColor(if (selected) ThemeRes.onFill(this) else palette.text)
                chip.background = ThemeRes.ripple(this, ThemeRes.shape(this, if (selected) palette.accent else palette.card,
                    if (selected) null else palette.outline, 14f))
                chip.isEnabled = ready && edifierState.noise != null
            }
            return
        }
        for (row in 0 until controls.childCount) {
            val rowView = controls.getChildAt(row) as? LinearLayout ?: continue
            for (column in 0 until rowView.childCount) {
                val chip = rowView.getChildAt(column) as? TextView ?: continue
                val p = ThemeRes.palette(this)
                val modes = BaseusBowie30Session.Noise.entries
                val mode = modes.getOrNull(row * 3 + column) ?: continue
                val selected = currentState.noise == mode
                chip.setTextColor(if (selected) ThemeRes.onFill(this) else p.text)
                chip.background = ThemeRes.ripple(this, ThemeRes.shape(this,
                    if (selected) p.accent else p.card, if (selected) null else p.outline, 14f))
                chip.isEnabled = ready && currentState.noise != null
            }
        }
    }

    override fun onDestroy() {
        handler.removeCallbacks(scanTimeout)
        scanner?.stop()
        closeSessions()
        super.onDestroy()
    }
}
