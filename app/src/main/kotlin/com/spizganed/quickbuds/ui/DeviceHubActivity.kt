package com.spizganed.quickbuds.ui

import android.annotation.SuppressLint
import android.app.AlertDialog
import android.app.Activity
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.EditText
import android.widget.ImageView
import android.widget.SeekBar
import android.widget.Toast
import android.text.InputType
import android.text.InputFilter
import com.spizganed.quickbuds.R
import com.spizganed.quickbuds.bluetooth.BaseusBowie30Session
import com.spizganed.quickbuds.bluetooth.BaseusBlade2Session
import com.spizganed.quickbuds.bluetooth.BaseusSppSession
import com.spizganed.quickbuds.bluetooth.BleGattScanner
import com.spizganed.quickbuds.bluetooth.EdifierW820NbPlusSession
import com.spizganed.quickbuds.bluetooth.DeviceGattProfiles
import com.spizganed.quickbuds.protocol.baseus.BaseusMobilePowerCodec

/** One in-app home for cross-brand accessory sessions. More device profiles are added here. */
class DeviceHubActivity : Activity() {
    private enum class DeviceKind { BASEUS_BOWIE_30, BASEUS_BLADE_2, EDIFIER_W820NB_PLUS, BASEUS_EP10_PRO, BASEUS_BOWIE_MF1 }
    private val handler = Handler(Looper.getMainLooper())
    private var scanner: BleGattScanner? = null
    private var baseusSession: BaseusBowie30Session? = null
    private var edifierSession: EdifierW820NbPlusSession? = null
    private var blade2Session: BaseusBlade2Session? = null
    private var baseusSppSession: BaseusSppSession? = null
    private var sppState = BaseusSppSession.State()
    private var selectedDevice = DeviceKind.BASEUS_BOWIE_30
    private var foundDevice: BluetoothDevice? = null
    private var ready = false
    private var currentState = BaseusBowie30Session.State()
    private var edifierState = EdifierW820NbPlusSession.State()
    private var blade2State = BaseusBlade2Session.State()
    private lateinit var deviceName: TextView
    private lateinit var deviceIllustration: ImageView
    private lateinit var status: TextView
    private lateinit var battery: TextView
    private lateinit var noise: TextView
    private lateinit var controls: LinearLayout
    private lateinit var sppGestureControls: LinearLayout
    private lateinit var edifierEqControls: LinearLayout
    private lateinit var edifierTimerControls: LinearLayout
    private lateinit var edifierGameModeControl: TextView
    private lateinit var edifierAutoPowerOffControl: TextView
    private lateinit var edifierNameControl: TextView
    private lateinit var edifierTouchSensitivityControl: TextView
    private lateinit var edifierTapControls: LinearLayout
    private lateinit var scanButton: TextView
    private lateinit var deviceRow: TextView
    private lateinit var powerControls: LinearLayout
    private lateinit var powerMetrics: TextView
    private lateinit var bladeTheme: TextView
    private lateinit var bladeSaver: TextView
    private lateinit var bladePort1: TextView
    private lateinit var bladePort2: TextView
    private lateinit var bladeTextInput: EditText
    private lateinit var bladeReminderInput: EditText
    private lateinit var bladeShutdownInput: EditText
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
        deviceIllustration = ImageView(this).apply {
            setImageResource(R.drawable.ic_device_earbuds)
            contentDescription = getString(R.string.device_hub_earbuds_illustration)
            scaleType = ImageView.ScaleType.FIT_CENTER
        }
        card.addView(deviceIllustration, LinearLayout.LayoutParams(dp(72f), dp(60f)).apply { gravity = Gravity.CENTER_HORIZONTAL; bottomMargin = dp(6f) })
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

        powerControls = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            visibility = View.GONE
            setPadding(0, dp(8f), 0, 0)
        }
        powerControls.addView(ImageView(this).apply {
            setImageResource(R.drawable.ic_baseus_blade2)
            contentDescription = "Baseus Blade 2 power bank"
            scaleType = ImageView.ScaleType.FIT_CENTER
        }, LinearLayout.LayoutParams(dp(76f), dp(90f)).apply { gravity = Gravity.CENTER_HORIZONTAL })
        powerMetrics = TextView(this).apply {
            textSize = 13f
            setTextColor(palette.text)
            typeface = android.graphics.Typeface.MONOSPACE
            setText(R.string.device_hub_powerbank_waiting)
            setPadding(0, dp(8f), 0, dp(8f))
        }
        powerControls.addView(powerMetrics)
        bladePort1 = accessoryAction(getString(R.string.device_hub_powerbank_port1), palette, dp) {
            chooseBladePortMode(1)
        }
        bladePort2 = accessoryAction(getString(R.string.device_hub_powerbank_port2), palette, dp) {
            chooseBladePortMode(2)
        }
        powerControls.addView(bladePort1)
        powerControls.addView(bladePort2)
        bladeTheme = TextView(this).apply {
            setText(R.string.device_hub_powerbank_theme)
            styleAccessoryAction(this, palette, dp)
            setOnClickListener { blade2Session?.setTheme(((blade2State.values[0x0020]?.value ?: -1) + 1).mod(3)) }
        }
        powerControls.addView(bladeTheme)
        bladeSaver = TextView(this).apply {
            setText(R.string.device_hub_powerbank_saver)
            styleAccessoryAction(this, palette, dp)
            setOnClickListener {
                val enabled = blade2State.values[0x0022]?.value ?: 0
                blade2Session?.setScreenSaver(enabled == 0)
            }
        }
        powerControls.addView(bladeSaver)
        bladeTextInput = EditText(this).apply {
            hint = getString(R.string.device_hub_powerbank_text_hint)
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
            maxLines = 1
        }
        powerControls.addView(bladeTextInput)
        powerControls.addView(accessoryAction(getString(R.string.device_hub_powerbank_set_text), palette, dp) {
            blade2Session?.setScreenText(bladeTextInput.text.toString())
        })
        bladeReminderInput = timerInput(R.string.device_hub_powerbank_reminder_hint)
        powerControls.addView(bladeReminderInput)
        powerControls.addView(accessoryAction(getString(R.string.device_hub_powerbank_set_reminder), palette, dp) {
            blade2Session?.setCountdown(0x001E, bladeReminderInput.text.toString().toIntOrNull() ?: -1)
        })
        bladeShutdownInput = timerInput(R.string.device_hub_powerbank_shutdown_hint)
        powerControls.addView(bladeShutdownInput)
        powerControls.addView(accessoryAction(getString(R.string.device_hub_powerbank_set_shutdown), palette, dp) {
            blade2Session?.setCountdown(0x001F, bladeShutdownInput.text.toString().toIntOrNull() ?: -1)
        })
        card.addView(powerControls)
        card.addView(TextView(this).apply {
            setText(R.string.device_hub_verified_note)
            textSize = 12f
            setTextColor(palette.textSecondary)
            setPadding(0, dp(12f), 0, 0)
        })
        root.addView(card)

        val devicePicker = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        listOf(DeviceKind.BASEUS_BOWIE_30 to "Bowie 30", DeviceKind.BASEUS_BLADE_2 to "Blade 2",
            DeviceKind.EDIFIER_W820NB_PLUS to "W820NB Plus", DeviceKind.BASEUS_EP10_PRO to "EP10 Pro",
            DeviceKind.BASEUS_BOWIE_MF1 to "Bowie MF1").chunked(3).forEachIndexed { rowIndex, entries ->
            val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
            entries.forEachIndexed { index, (kind, label) ->
                row.addView(TextView(this).apply {
                    text = label; textSize = 14f; gravity = Gravity.CENTER
                    setPadding(dp(8f), dp(12f), dp(8f), dp(12f)); setTextColor(palette.text)
                    background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.card(this@DeviceHubActivity, 14f))
                    isClickable = true; isFocusable = true
                    setOnClickListener { selectDevice(kind) }
                }, LinearLayout.LayoutParams(0, -2, 1f).apply { if (index > 0) marginStart = dp(8f) })
            }
            devicePicker.addView(row, LinearLayout.LayoutParams(-1, -2).apply { if (rowIndex > 0) topMargin = dp(8f) })
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

    @SuppressLint("MissingPermission") // BLUETOOTH_CONNECT is requested by the parent app before opening this hub.
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
        powerControls.visibility = View.GONE
        status.setText(R.string.device_hub_scanning)
        scanButton.isEnabled = false
        scanButton.setText(R.string.device_hub_scanning)
        if (selectedDevice == DeviceKind.BASEUS_EP10_PRO || selectedDevice == DeviceKind.BASEUS_BOWIE_MF1) {
            val expected = if (selectedDevice == DeviceKind.BASEUS_EP10_PRO)
                listOf("baseus bass ep10 pro", "bass ep10 pro", "baseus ep10 pro", "ep10 pro")
            else listOf("baseus bowie mf1", "bowie mf1")
            val adapter = getSystemService(BluetoothManager::class.java)?.adapter
            val paired = runCatching { adapter?.bondedDevices.orEmpty().firstOrNull { device ->
                val name = runCatching { device.name }.getOrNull()?.trim()?.lowercase() ?: return@firstOrNull false
                expected.any { name == it }
            } }.getOrNull()
            scanButton.isEnabled = true; scanButton.setText(R.string.device_hub_scan)
            if (paired == null) {
                status.setText(R.string.device_hub_spp_pair_hint)
            } else {
                seenAny = true; foundDevice = paired
                deviceRow.text = "${runCatching { paired.name }.getOrNull() ?: deviceName.text}  ·  ${getString(R.string.conn_action_connect)}"
                deviceRow.visibility = View.VISIBLE
                status.setText(R.string.device_hub_spp_found)
            }
            return
        }
        val profile = when (selectedDevice) {
            DeviceKind.BASEUS_BOWIE_30 -> DeviceGattProfiles.baseusShared.copy(
                displayName = "Baseus Bowie 30", acceptedNames = setOf("Baseus Bowie 30", "Bowie 30"))
            DeviceKind.BASEUS_BLADE_2 -> DeviceGattProfiles.baseusBlade2
            DeviceKind.EDIFIER_W820NB_PLUS -> DeviceGattProfiles.edifierW820NbPlus
            DeviceKind.BASEUS_EP10_PRO, DeviceKind.BASEUS_BOWIE_MF1 -> error("SPP devices do not use BLE discovery")
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
        when (selectedDevice) {
            DeviceKind.BASEUS_BOWIE_30 -> connectBaseus(device)
            DeviceKind.BASEUS_BLADE_2 -> connectBlade2(device)
            DeviceKind.EDIFIER_W820NB_PLUS -> connectEdifier(device)
            DeviceKind.BASEUS_EP10_PRO -> connectBaseusSpp(device, BaseusSppSession.Model.EP10_PRO)
            DeviceKind.BASEUS_BOWIE_MF1 -> connectBaseusSpp(device, BaseusSppSession.Model.BOWIE_MF1)
        }
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

    private fun connectBlade2(device: BluetoothDevice) {
        blade2Session = BaseusBlade2Session(this, device, object : BaseusBlade2Session.Listener {
            override fun onState(state: BaseusBlade2Session.State) {
                blade2State = state
                renderBlade2(state)
            }
            override fun onStatus(text: String) { status.text = text }
            override fun onReady() = connected()
            override fun onClosed(reason: String) = disconnected(reason)
        })
    }

    private fun connectBaseusSpp(device: BluetoothDevice, model: BaseusSppSession.Model) {
        baseusSppSession = BaseusSppSession(device, model, object : BaseusSppSession.Listener {
            override fun onState(state: BaseusSppSession.State) {
                sppState = state
                battery.text = "${getString(R.string.device_hub_battery)}  ·  L ${state.leftBattery ?: "—"}%   R ${state.rightBattery ?: "—"}%"
                battery.visibility = if (state.leftBattery == null) View.GONE else View.VISIBLE
                noise.text = "Noise control  ·  ${state.noise?.let(::sppNoiseLabel) ?: "—"}"
                noise.visibility = if (state.noise == null) View.GONE else View.VISIBLE
                renderModes()
            }
            override fun onStatus(text: String) { status.text = text }
            override fun onReady() = connected()
            override fun onClosed(reason: String) = disconnected(reason)
        }).also { it.connect() }
    }

    private fun sppNoiseLabel(mode: BaseusSppSession.Noise): String = when (mode) {
        BaseusSppSession.Noise.OFF -> getString(R.string.device_hub_mode_off)
        BaseusSppSession.Noise.ANC -> "ANC"
        BaseusSppSession.Noise.TRANSPARENCY -> getString(R.string.device_hub_mode_transparency)
    }

    private fun connected() {
        ready = true
        controls.visibility = if (selectedDevice == DeviceKind.BASEUS_BLADE_2 || selectedDevice == DeviceKind.BASEUS_BOWIE_MF1) View.GONE else View.VISIBLE
        powerControls.visibility = if (selectedDevice == DeviceKind.BASEUS_BLADE_2) View.VISIBLE else View.GONE
        status.setText(R.string.device_hub_ready)
        scanButton.isEnabled = true
        scanButton.setText(R.string.device_hub_refresh)
        scanButton.setOnClickListener { when (selectedDevice) {
            DeviceKind.BASEUS_BOWIE_30 -> baseusSession?.refresh()
            DeviceKind.BASEUS_BLADE_2 -> blade2Session?.refresh()
            DeviceKind.EDIFIER_W820NB_PLUS -> edifierSession?.refresh()
            DeviceKind.BASEUS_EP10_PRO, DeviceKind.BASEUS_BOWIE_MF1 -> baseusSppSession?.refresh()
        } }
        renderModes()
    }

    private fun disconnected(reason: String) {
        ready = false
        controls.visibility = View.GONE
        powerControls.visibility = View.GONE
        status.text = reason
        scanButton.isEnabled = true
        scanButton.setText(R.string.device_hub_scan)
        scanButton.setOnClickListener { startScan() }
        renderModes()
    }

    private fun closeSessions() {
        baseusSession?.close(); baseusSession = null
        edifierSession?.close(); edifierSession = null
        blade2Session?.close(); blade2Session = null
        baseusSppSession?.close(); baseusSppSession = null
    }

    private fun selectDevice(kind: DeviceKind) {
        if (selectedDevice == kind) return
        scanner?.stop(); closeSessions()
        selectedDevice = kind
        foundDevice = null; ready = false
        currentState = BaseusBowie30Session.State(); edifierState = EdifierW820NbPlusSession.State()
        blade2State = BaseusBlade2Session.State()
        sppState = BaseusSppSession.State()
        deviceName.text = when (kind) {
            DeviceKind.BASEUS_BOWIE_30 -> "Baseus Bowie 30"
            DeviceKind.BASEUS_BLADE_2 -> "Baseus Blade 2"
            DeviceKind.EDIFIER_W820NB_PLUS -> "EDIFIER W820NB Plus"
            DeviceKind.BASEUS_EP10_PRO -> "Baseus EP10 Pro"
            DeviceKind.BASEUS_BOWIE_MF1 -> "Baseus Bowie MF1"
        }
        val illustration = when (kind) {
            DeviceKind.BASEUS_BOWIE_30, DeviceKind.BASEUS_EP10_PRO -> R.drawable.ic_device_earbuds
            DeviceKind.BASEUS_BOWIE_MF1, DeviceKind.EDIFIER_W820NB_PLUS -> R.drawable.ic_device_headphones
            DeviceKind.BASEUS_BLADE_2 -> R.drawable.ic_baseus_blade2
        }
        deviceIllustration.setImageResource(illustration)
        deviceIllustration.contentDescription = when (kind) {
            DeviceKind.BASEUS_BOWIE_30, DeviceKind.BASEUS_EP10_PRO -> getString(R.string.device_hub_earbuds_illustration)
            DeviceKind.BASEUS_BOWIE_MF1, DeviceKind.EDIFIER_W820NB_PLUS -> getString(R.string.device_hub_headphones_illustration)
            DeviceKind.BASEUS_BLADE_2 -> getString(R.string.device_hub_powerbank_illustration)
        }
        deviceRow.visibility = View.GONE; controls.removeAllViews(); controls.visibility = View.GONE
        if (kind == DeviceKind.BASEUS_BOWIE_30) addModes(controls, BaseusBowie30Session.Noise.entries)
        else if (kind == DeviceKind.EDIFIER_W820NB_PLUS) addEdifierModes()
        else if (kind == DeviceKind.BASEUS_EP10_PRO) addSppModes()
        powerControls.visibility = View.GONE
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
        edifierEqControls = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, ThemeRes.dp(this@DeviceHubActivity, 10f), 0, 0)
        }
        controls.addView(edifierEqControls)
        edifierTimerControls = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, ThemeRes.dp(this@DeviceHubActivity, 10f), 0, 0)
        }
        controls.addView(edifierTimerControls)
        edifierGameModeControl = TextView(this).apply {
            textSize = 13f; gravity = Gravity.CENTER
            setPadding(ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f), ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f))
            isClickable = true; isFocusable = true
            background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.card(this@DeviceHubActivity, 14f))
            setOnClickListener { edifierState.gameModeEnabled?.let { edifierSession?.setGameMode(!it) } }
        }
        controls.addView(edifierGameModeControl)
        edifierTouchSensitivityControl = TextView(this).apply {
            textSize = 13f; gravity = Gravity.CENTER
            setPadding(ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f), ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f))
            isClickable = true; isFocusable = true
            background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.card(this@DeviceHubActivity, 14f))
            setOnClickListener {
                val index = edifierState.touchSensitivityIndex ?: return@setOnClickListener
                val current = edifierState.touchSensitivityValue ?: return@setOnClickListener
                val max = if (index == 1) 31 else if (index == 2) 15 else return@setOnClickListener
                val slider = SeekBar(this@DeviceHubActivity).apply { this.max = max; progress = current }
                AlertDialog.Builder(this@DeviceHubActivity).setTitle(R.string.device_hub_touch_sensitivity).setView(slider)
                    .setNegativeButton(android.R.string.cancel, null)
                    .setPositiveButton(R.string.device_hub_save) { _, _ -> edifierSession?.setTouchSensitivity(slider.progress) }
                    .show()
            }
        }
        controls.addView(edifierTouchSensitivityControl)
        edifierTapControls = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, ThemeRes.dp(this@DeviceHubActivity, 10f), 0, 0)
        }
        controls.addView(edifierTapControls)
        edifierAutoPowerOffControl = TextView(this).apply {
            textSize = 13f; gravity = Gravity.CENTER
            setPadding(ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f), ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f))
            isClickable = true; isFocusable = true
            background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.card(this@DeviceHubActivity, 14f))
            setOnClickListener {
                edifierState.autoPowerOffEnabled?.let { enabled ->
                    if (enabled) edifierSession?.setAutoPowerOff(false)
                    else AlertDialog.Builder(this@DeviceHubActivity)
                        .setTitle(R.string.device_hub_auto_power_off)
                        .setMessage(R.string.device_hub_auto_power_off_confirm)
                        .setNegativeButton(android.R.string.cancel, null)
                        .setPositiveButton(R.string.device_hub_mode_on) { _, _ -> edifierSession?.setAutoPowerOff(true) }
                        .show()
                }
            }
        }
        controls.addView(edifierAutoPowerOffControl)
        edifierNameControl = TextView(this).apply {
            textSize = 13f; gravity = Gravity.CENTER
            setPadding(ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f), ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f))
            isClickable = true; isFocusable = true
            background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.card(this@DeviceHubActivity, 14f))
            setOnClickListener {
                val limit = edifierState.bluetoothNameLimitBytes
                if (!ready || !edifierState.bluetoothNameSupported || limit <= 0) return@setOnClickListener
                val input = EditText(this@DeviceHubActivity).apply {
                    inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
                    setSingleLine(true)
                    setText(edifierState.bluetoothName.orEmpty())
                    filters = arrayOf(InputFilter { source, start, end, dest, dstart, dend ->
                        val candidate = dest.toString().replaceRange(dstart, dend, source.subSequence(start, end).toString())
                        if (candidate.toByteArray(Charsets.UTF_8).size > limit) "" else null
                    })
                }
                AlertDialog.Builder(this@DeviceHubActivity)
                    .setTitle(R.string.device_hub_bluetooth_name)
                    .setMessage(getString(R.string.device_hub_bluetooth_name_limit, limit))
                    .setView(input)
                    .setNegativeButton(android.R.string.cancel, null)
                    .setPositiveButton(R.string.device_hub_save) { _, _ -> edifierSession?.setBluetoothName(input.text.toString()) }
                    .show()
            }
        }
        controls.addView(edifierNameControl)
    }

    private fun chooseEdifierNoiseCycle(mapping: EdifierW820NbPlusSession.TouchControl) {
        val labels = arrayOf(
            getString(R.string.device_hub_touch_anc_high),
            getString(R.string.device_hub_touch_anc_medium),
            getString(R.string.device_hub_touch_ambient),
            getString(R.string.device_hub_touch_normal),
            getString(R.string.device_hub_touch_wind),
        )
        val selected = BooleanArray(labels.size) { index -> (mapping.noiseMask ?: 0) and (1 shl index) != 0 }
        AlertDialog.Builder(this)
            .setTitle(R.string.device_hub_touch_cycle)
            .setMultiChoiceItems(labels, selected) { _, which, checked -> selected[which] = checked }
            .setNegativeButton(android.R.string.cancel, null)
            .setPositiveButton(R.string.device_hub_save) { _, _ ->
                val modes = selected.indices.filter { selected[it] }
                if (modes.size !in 2..3) Toast.makeText(this, R.string.device_hub_touch_choose_two_three, Toast.LENGTH_SHORT).show()
                else edifierSession?.setTouchControl(mapping.index, 6, modes)
            }
            .show()
    }

    private fun addSppModes() {
        val palette = ThemeRes.palette(this)
        BaseusSppSession.Noise.entries.forEachIndexed { index, mode ->
            controls.addView(TextView(this).apply {
                text = sppNoiseLabel(mode); textSize = 13f; gravity = Gravity.CENTER
                setPadding(ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f), ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f))
                setTextColor(palette.text); isClickable = true; isFocusable = true
                background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.card(this@DeviceHubActivity, 14f))
                setOnClickListener { baseusSppSession?.setNoise(mode) }
            }, LinearLayout.LayoutParams(-1, -2).apply { if (index > 0) topMargin = ThemeRes.dp(this@DeviceHubActivity, 6f) })
        }
        sppGestureControls = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        controls.addView(sppGestureControls, LinearLayout.LayoutParams(-1, -2).apply {
            topMargin = ThemeRes.dp(this@DeviceHubActivity, 12f)
        })
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
        if (selectedDevice == DeviceKind.BASEUS_BLADE_2) return
        if (selectedDevice == DeviceKind.EDIFIER_W820NB_PLUS) {
            for (index in EdifierW820NbPlusSession.Noise.entries.indices) {
                val chip = controls.getChildAt(index) as? TextView ?: continue
                val mode = EdifierW820NbPlusSession.Noise.entries.getOrNull(index) ?: continue
                val palette = ThemeRes.palette(this)
                val selected = edifierState.noise == mode
                chip.setTextColor(if (selected) ThemeRes.onFill(this) else palette.text)
                chip.background = ThemeRes.ripple(this, ThemeRes.shape(this, if (selected) palette.accent else palette.card,
                    if (selected) null else palette.outline, 14f))
                chip.isEnabled = ready && edifierState.noise != null
            }
            if (::edifierEqControls.isInitialized) {
                edifierEqControls.removeAllViews()
                if (edifierState.equalizerPresets.isNotEmpty()) {
                    edifierEqControls.addView(TextView(this).apply {
                        text = getString(R.string.device_hub_equalizer)
                        textSize = 12f
                        setTextColor(ThemeRes.palette(this@DeviceHubActivity).textSecondary)
                    })
                    edifierState.equalizerPresets.forEachIndexed { index, preset ->
                        edifierEqControls.addView(TextView(this).apply {
                            text = if (edifierState.selectedEqualizer == preset.id) "✓  ${preset.name}" else preset.name
                            textSize = 13f
                            gravity = Gravity.CENTER
                            setPadding(ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f), ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f))
                            val palette = ThemeRes.palette(this@DeviceHubActivity)
                            setTextColor(if (edifierState.selectedEqualizer == preset.id) ThemeRes.onFill(this@DeviceHubActivity) else palette.text)
                            isClickable = true; isFocusable = true
                            background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.shape(this@DeviceHubActivity,
                                if (edifierState.selectedEqualizer == preset.id) palette.accent else palette.card,
                                if (edifierState.selectedEqualizer == preset.id) null else palette.outline, 14f))
                            isEnabled = ready && edifierState.selectedEqualizer != null
                            setOnClickListener { edifierSession?.setEqualizer(preset.id) }
                        }, LinearLayout.LayoutParams(-1, -2).apply { if (index > 0) topMargin = ThemeRes.dp(this@DeviceHubActivity, 6f) })
                    }
                }
            }
            if (::edifierTimerControls.isInitialized) {
                edifierTimerControls.removeAllViews()
                if (edifierState.sleepTimerOptions.isNotEmpty()) {
                    edifierTimerControls.addView(TextView(this).apply {
                        text = getString(R.string.device_hub_sleep_timer)
                        textSize = 12f
                        setTextColor(ThemeRes.palette(this@DeviceHubActivity).textSecondary)
                    })
                    edifierState.sleepTimerOptions.forEachIndexed { index, minutes ->
                        val selected = edifierState.sleepTimerMinutes == minutes
                        edifierTimerControls.addView(TextView(this).apply {
                            text = (if (selected) "✓  " else "") + if (minutes == 0) getString(R.string.device_hub_mode_off) else getString(R.string.device_hub_minutes, minutes)
                            textSize = 13f; gravity = Gravity.CENTER
                            setPadding(ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f), ThemeRes.dp(this@DeviceHubActivity, 8f), ThemeRes.dp(this@DeviceHubActivity, 12f))
                            val palette = ThemeRes.palette(this@DeviceHubActivity)
                            setTextColor(if (selected) ThemeRes.onFill(this@DeviceHubActivity) else palette.text)
                            isClickable = true; isFocusable = true
                            background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.shape(this@DeviceHubActivity,
                                if (selected) palette.accent else palette.card, if (selected) null else palette.outline, 14f))
                            isEnabled = ready
                            setOnClickListener { edifierSession?.setSleepTimer(minutes) }
                        }, LinearLayout.LayoutParams(-1, -2).apply { if (index > 0) topMargin = ThemeRes.dp(this@DeviceHubActivity, 6f) })
                    }
                }
            }
            if (::edifierGameModeControl.isInitialized) {
                val supported = edifierState.gameModeSupported && edifierState.gameModeEnabled != null
                edifierGameModeControl.visibility = if (supported) View.VISIBLE else View.GONE
                edifierGameModeControl.text = getString(R.string.device_hub_game_mode) + " · " +
                    getString(if (edifierState.gameModeEnabled == true) R.string.device_hub_mode_on else R.string.device_hub_mode_off)
                val palette = ThemeRes.palette(this)
                val enabled = edifierState.gameModeEnabled == true
                edifierGameModeControl.setTextColor(if (enabled) ThemeRes.onFill(this) else palette.text)
                edifierGameModeControl.background = ThemeRes.ripple(this, ThemeRes.shape(this,
                    if (enabled) palette.accent else palette.card, if (enabled) null else palette.outline, 14f))
                edifierGameModeControl.isEnabled = ready && supported
            }
            if (::edifierAutoPowerOffControl.isInitialized) {
                val supported = edifierState.autoPowerOffSupported && edifierState.autoPowerOffEnabled != null
                edifierAutoPowerOffControl.visibility = if (supported) View.VISIBLE else View.GONE
                edifierAutoPowerOffControl.text = getString(R.string.device_hub_auto_power_off) + " · " +
                    getString(if (edifierState.autoPowerOffEnabled == true) R.string.device_hub_mode_on else R.string.device_hub_mode_off)
                val palette = ThemeRes.palette(this)
                val enabled = edifierState.autoPowerOffEnabled == true
                edifierAutoPowerOffControl.setTextColor(if (enabled) ThemeRes.onFill(this) else palette.text)
                edifierAutoPowerOffControl.background = ThemeRes.ripple(this, ThemeRes.shape(this,
                    if (enabled) palette.accent else palette.card, if (enabled) null else palette.outline, 14f))
                edifierAutoPowerOffControl.isEnabled = ready && supported
            }
            if (::edifierTouchSensitivityControl.isInitialized) {
                val value = edifierState.touchSensitivityValue
                val supported = edifierState.touchSensitivitySupported && value != null
                edifierTouchSensitivityControl.visibility = if (supported) View.VISIBLE else View.GONE
                edifierTouchSensitivityControl.text = getString(R.string.device_hub_touch_sensitivity) + " · " + (value?.toString() ?: "–")
                val palette = ThemeRes.palette(this)
                edifierTouchSensitivityControl.setTextColor(palette.text)
                edifierTouchSensitivityControl.background = ThemeRes.ripple(this, ThemeRes.shape(this, palette.card, palette.outline, 14f))
                edifierTouchSensitivityControl.isEnabled = ready && supported
            }
            if (::edifierTapControls.isInitialized) {
                val supported = edifierState.touchControlsSupported && edifierState.touchControls.isNotEmpty()
                edifierTapControls.visibility = if (supported) View.VISIBLE else View.GONE
                edifierTapControls.removeAllViews()
                if (supported) {
                    val labels = listOf(
                        R.string.device_hub_touch_left_tap, R.string.device_hub_touch_right_tap,
                        R.string.device_hub_touch_left_hold, R.string.device_hub_touch_right_hold,
                    )
                    edifierState.touchControls.sortedBy { it.index }.forEach { mapping ->
                        val actionNames = if (mapping.index == 1 || mapping.index == 3)
                            listOf(R.string.device_hub_touch_play_pause, R.string.device_hub_touch_volume_up,
                                R.string.device_hub_touch_previous, R.string.device_hub_touch_assistant,
                                R.string.device_hub_touch_game, R.string.device_hub_touch_cycle)
                        else listOf(R.string.device_hub_touch_play_pause, R.string.device_hub_touch_volume_down,
                            R.string.device_hub_touch_next, R.string.device_hub_touch_assistant,
                            R.string.device_hub_touch_sound_mode, R.string.device_hub_touch_cycle)
                        val selected = when (mapping.action) {
                            6 -> getString(R.string.device_hub_touch_cycle)
                            in 1..5 -> getString(actionNames[mapping.action - 1])
                            else -> getString(R.string.device_hub_unknown)
                        }
                        edifierTapControls.addView(TextView(this).apply {
                            text = getString(labels[mapping.index - 1]) + " · " + selected
                            textSize = 13f; gravity = Gravity.CENTER_VERTICAL
                            setPadding(ThemeRes.dp(this@DeviceHubActivity, 14f), ThemeRes.dp(this@DeviceHubActivity, 12f), ThemeRes.dp(this@DeviceHubActivity, 14f), ThemeRes.dp(this@DeviceHubActivity, 12f))
                            setTextColor(ThemeRes.palette(this@DeviceHubActivity).text)
                            background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.card(this@DeviceHubActivity, 14f))
                            isEnabled = ready
                            setOnClickListener {
                                AlertDialog.Builder(this@DeviceHubActivity)
                                    .setTitle(labels[mapping.index - 1])
                                    .setItems(actionNames.map { getString(it) }.toTypedArray()) { _, which ->
                                        val action = which + 1
                                        if (action == 6) chooseEdifierNoiseCycle(mapping)
                                        else edifierSession?.setTouchControl(mapping.index, action)
                                    }
                                    .setNegativeButton(android.R.string.cancel, null)
                                    .show()
                            }
                        }, LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = ThemeRes.dp(this@DeviceHubActivity, 6f) })
                    }
                }
            }
            if (::edifierNameControl.isInitialized) {
                val supported = edifierState.bluetoothNameSupported && edifierState.bluetoothNameLimitBytes > 0
                edifierNameControl.visibility = if (supported) View.VISIBLE else View.GONE
                edifierNameControl.text = getString(R.string.device_hub_bluetooth_name) + " · " +
                    (edifierState.bluetoothName ?: getString(R.string.device_hub_unknown))
                val palette = ThemeRes.palette(this)
                edifierNameControl.setTextColor(palette.text)
                edifierNameControl.background = ThemeRes.ripple(this, ThemeRes.shape(this, palette.card, palette.outline, 14f))
                edifierNameControl.isEnabled = ready && supported
            }
            return
        }
        if (selectedDevice == DeviceKind.BASEUS_EP10_PRO) {
            for (index in BaseusSppSession.Noise.entries.indices) {
                val chip = controls.getChildAt(index) as? TextView ?: continue
                val mode = BaseusSppSession.Noise.entries.getOrNull(index) ?: continue
                val palette = ThemeRes.palette(this)
                val selected = sppState.noise == mode
                chip.setTextColor(if (selected) ThemeRes.onFill(this) else palette.text)
                chip.background = ThemeRes.ripple(this, ThemeRes.shape(this, if (selected) palette.accent else palette.card,
                    if (selected) null else palette.outline, 14f))
                chip.isEnabled = ready && sppState.noise != null
            }
            renderSppGestures()
            return
        }
        if (selectedDevice == DeviceKind.BASEUS_BOWIE_MF1) return
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

    private fun renderSppGestures() {
        if (!::sppGestureControls.isInitialized) return
        sppGestureControls.removeAllViews()
        if (sppState.gestures.isEmpty()) return
        val secondary = ThemeRes.palette(this).textSecondary
        sppGestureControls.addView(TextView(this).apply {
            text = if (resources.configuration.locales[0].language == "ru") "Жесты" else "Gestures"
            textSize = 12f; setTextColor(secondary)
        })
        sppState.gestures.sortedWith(compareBy({ it.button }, { it.layout })).forEachIndexed { index, gesture ->
            val label = when (gesture.layout) { 3 -> "Single"; 0 -> "Double"; 1 -> "Triple"; 2 -> "Hold"; else -> "Gesture ${gesture.layout}" }
            val side = if (gesture.button == 0) "Left" else "Right"
            val action = baseusActionLabel(gesture.action)
            sppGestureControls.addView(TextView(this).apply {
                text = "$side · $label\n$action"
                textSize = 13f; gravity = Gravity.CENTER_VERTICAL
                setPadding(ThemeRes.dp(this@DeviceHubActivity, 14f), ThemeRes.dp(this@DeviceHubActivity, 10f), ThemeRes.dp(this@DeviceHubActivity, 14f), ThemeRes.dp(this@DeviceHubActivity, 10f))
                setTextColor(ThemeRes.palette(this@DeviceHubActivity).text)
                isClickable = true; isFocusable = true
                background = ThemeRes.ripple(this@DeviceHubActivity, ThemeRes.card(this@DeviceHubActivity, 14f))
                isEnabled = ready
                setOnClickListener { chooseBaseusGesture(gesture) }
            }, LinearLayout.LayoutParams(-1, -2).apply { topMargin = ThemeRes.dp(this@DeviceHubActivity, if (index == 0) 6f else 4f) })
            if (gesture.action == 6) {
                val key = "${gesture.button}:${gesture.layout}"
                val selected = sppState.gestureChildren[key].orEmpty()
                val modes = listOf(1, 2, 0)
                val labels = modes.map { when (it) {
                    1 -> if (resources.configuration.locales[0].language == "ru") "Шумоподавление" else "Noise cancelling"
                    2 -> getString(R.string.device_hub_mode_transparency)
                    else -> if (resources.configuration.locales[0].language == "ru") "Обычный режим" else "Normal"
                } }
                sppGestureControls.addView(TextView(this).apply {
                    text = if (selected.isEmpty()) {
                        if (resources.configuration.locales[0].language == "ru") "Режимы цикла не прочитаны" else "Cycle choices unavailable"
                    } else labels.filterIndexed { i, _ -> modes[i] in selected }.joinToString(" · ")
                    textSize = 12f; gravity = Gravity.CENTER_VERTICAL
                    setPadding(ThemeRes.dp(this@DeviceHubActivity, 14f), ThemeRes.dp(this@DeviceHubActivity, 6f), ThemeRes.dp(this@DeviceHubActivity, 14f), ThemeRes.dp(this@DeviceHubActivity, 8f))
                    setTextColor(ThemeRes.palette(this@DeviceHubActivity).textSecondary)
                    isClickable = selected.isNotEmpty() && ready; isFocusable = isClickable
                    setOnClickListener { chooseBaseusGestureChildren(gesture, modes, labels, selected) }
                })
            }
        }
    }

    private fun chooseBaseusGestureChildren(
        gesture: BaseusSppSession.Gesture,
        modes: List<Int>,
        labels: List<String>,
        selected: List<Int>,
    ) {
        val checked = BooleanArray(modes.size) { modes[it] in selected }
        val title = if (resources.configuration.locales[0].language == "ru") "Режимы цикла шумоподавления" else "Noise-cycle modes"
        AlertDialog.Builder(this)
            .setTitle(title)
            .setMultiChoiceItems(labels.toTypedArray(), checked) { _, which, isChecked -> checked[which] = isChecked }
            .setNegativeButton(android.R.string.cancel, null)
            .setPositiveButton(if (resources.configuration.locales[0].language == "ru") "Сохранить" else "Save") { _, _ ->
                val values = modes.filterIndexed { index, _ -> checked[index] }
                if (values.size < 2) {
                    status.setText(if (resources.configuration.locales[0].language == "ru") "Выберите минимум два режима" else "Select at least two modes")
                } else baseusSppSession?.setGestureChildren(gesture, values)
            }
            .show()
    }

    private fun chooseBaseusGesture(gesture: BaseusSppSession.Gesture) {
        val actions = if (gesture.layout == 3) listOf(0, 1) else listOf(0, 1, 2, 3, 4, 6, 11, 12)
        AlertDialog.Builder(this)
            .setTitle("${if (gesture.button == 0) "Left" else "Right"} · ${when (gesture.layout) { 3 -> "Single"; 0 -> "Double"; 1 -> "Triple"; else -> "Hold" }}")
            .setItems(actions.map(::baseusActionLabel).toTypedArray()) { _, which -> baseusSppSession?.setGesture(gesture, actions[which]) }
            .show()
    }

    private fun baseusActionLabel(action: Int): String = when (action) {
        0 -> getString(R.string.gesture_action_none)
        1 -> getString(R.string.gesture_action_play_pause)
        2 -> getString(R.string.gesture_action_prev)
        3 -> getString(R.string.gesture_action_next)
        4 -> getString(R.string.gesture_action_assistant)
        6 -> if (resources.configuration.locales[0].language == "ru") "Переключение режимов шумоподавления" else "Cycle noise modes"
        11 -> getString(R.string.gesture_action_volume_up)
        12 -> getString(R.string.gesture_action_volume_down)
        else -> "Action $action"
    }

    private fun renderBlade2(state: BaseusBlade2Session.State) {
        if (!::powerMetrics.isInitialized) return
        val entries = state.values.toSortedMap().mapNotNull { (code, value) ->
            val label = powerMetricLabel(code) ?: return@mapNotNull null
            val shown = when (code) {
                0x000D, 0x0025 -> "${value.value - 100} °C"
                0x0010 -> "0x${value.value.toString(16).uppercase().padStart(4, '0')}"
                0x0012 -> "#${value.value}"
                0x0023, 0x0024 -> "${value.value}"
                else -> value.scaled()?.let { "${"%.2f".format(java.util.Locale.ROOT, it)} ${powerMetricUnit(code)}" } ?: "${value.value} (raw)"
            }
            "$label  ·  $shown"
        }
        powerMetrics.text = entries.joinToString("\n").ifEmpty { getString(R.string.device_hub_powerbank_waiting) }
        val theme = state.values[0x0020]?.value
        bladeTheme.text = getString(R.string.device_hub_powerbank_theme) + "  ·  " + (theme?.let { "${it + 1}/3" } ?: "—")
        val saver = state.values[0x0022]?.value
        bladeSaver.text = getString(R.string.device_hub_powerbank_saver) + "  ·  " + when (saver) {
            0 -> getString(R.string.device_hub_mode_off)
            1 -> getString(R.string.device_hub_ready)
            else -> "—"
        }
        state.values[0x001E]?.let { if (!bladeReminderInput.hasFocus()) bladeReminderInput.setText(it.value.toString()) }
        state.values[0x001F]?.let { if (!bladeShutdownInput.hasFocus()) bladeShutdownInput.setText(it.value.toString()) }
        val portStatus = state.values[0x0010]?.value
        bladePort1.text = portLabel(1, portStatus)
        bladePort2.text = portLabel(2, portStatus)
    }

    private fun powerMetricLabel(code: Int): String? = when (code) {
        in 0x0001..0x000C -> {
            val port = if (code in 1..3 || code in 7..9) "USB-C 1" else "USB-C 2"
            val direction = if (code in 1..6) "OUT" else "IN"
            val metric = when ((code - 1) % 3) { 0 -> "voltage"; 1 -> "current"; else -> "power" }
            "$port $direction $metric"
        }
        0x000D -> "Temperature"
        0x0010 -> "Power status flags"
        0x0011 -> "Error code"
        0x0012 -> "Negotiated protocol"
        0x0013 -> "Module status"
        0x0014 -> "Energy (app-scaled)"
        0x0015 -> "Remaining time"
        0x0016 -> "Charge time"
        0x001E -> getString(R.string.device_hub_powerbank_reminder)
        0x001F -> getString(R.string.device_hub_powerbank_shutdown)
        0x0023 -> "Module version"
        0x0024 -> "DC module version"
        0x0025 -> "Cell temperature"
        0x0026 -> "Cell cycles"
        in 0x0028..0x002D -> "Cell ${code - 0x0027} voltage"
        in 0x0031..0x0036 -> "Cell ${code - 0x0030} current"
        else -> null
    }

    private fun powerMetricUnit(code: Int): String = when (code) {
        in 0x0001..0x000C -> when ((code - 1) % 3) { 0 -> "V"; 1 -> "A"; else -> "W" }
        0x0015, 0x0016, 0x001E, 0x001F -> "min"
        in 0x0028..0x002D -> "mV"
        in 0x0031..0x0036 -> "mA"
        else -> "raw/scale"
    }

    private fun timerInput(hintRes: Int): EditText = EditText(this).apply {
        hint = getString(hintRes)
        inputType = InputType.TYPE_CLASS_NUMBER
        maxLines = 1
    }

    private fun accessoryAction(label: String, palette: Palette, dp: (Float) -> Int, action: () -> Unit): TextView = TextView(this).apply {
        text = label
        styleAccessoryAction(this, palette, dp)
        setOnClickListener { action() }
    }

    private fun styleAccessoryAction(view: TextView, palette: Palette, dp: (Float) -> Int) {
        view.textSize = 13f
        view.gravity = Gravity.CENTER
        view.setTextColor(palette.text)
        view.setPadding(dp(10f), dp(11f), dp(10f), dp(11f))
        view.background = ThemeRes.ripple(this, ThemeRes.card(this, 14f))
        view.isClickable = true
        view.isFocusable = true
    }

    override fun onDestroy() {
        handler.removeCallbacks(scanTimeout)
        scanner?.stop()
        closeSessions()
        super.onDestroy()
    }

    private fun chooseBladePortMode(port: Int) {
        val status = blade2State.values[0x0010]?.value?.toInt() ?: run {
            blade2Session?.refresh()
            return
        }
        val current = (status shr ((port - 1) * 2)) and 0x03
        val title = getString(if (port == 1) R.string.device_hub_powerbank_port1 else R.string.device_hub_powerbank_port2)
        val options = arrayOf(
            getString(R.string.device_hub_powerbank_port_off),
            getString(R.string.device_hub_powerbank_port_output),
            getString(R.string.device_hub_powerbank_port_input),
        )
        AlertDialog.Builder(this)
            .setTitle(title)
            .setSingleChoiceItems(options, current.takeIf { it in 0..2 } ?: -1) { dialog, selected ->
                dialog.dismiss()
                blade2Session?.setPortMode(port, selected)
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun portLabel(port: Int, status: Int?): String {
        val title = getString(if (port == 1) R.string.device_hub_powerbank_port1 else R.string.device_hub_powerbank_port2)
        val mode = status?.let { (it shr ((port - 1) * 2)) and 0x03 } ?: -1
        val value = when (mode) {
            0 -> R.string.device_hub_powerbank_port_off
            1 -> R.string.device_hub_powerbank_port_output
            2 -> R.string.device_hub_powerbank_port_input
            else -> R.string.device_hub_powerbank_port_unknown
        }
        return "$title  ·  ${getString(value)}"
    }
}
