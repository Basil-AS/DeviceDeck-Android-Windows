//! Cross-brand Bluetooth device hub for the existing DeviceDeck desktop app.
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Manager, Peripheral};
use futures::StreamExt;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager as TauriManager, State};
use uuid::Uuid;

const EDIFIER_DISCOVERY: &str = "0000a500-0000-1000-8000-00805f9b34fb";
const EDIFIER_SERVICE: &str = "4809a501-1a48-11e9-ab14-d663bd873d93";
const EDIFIER_WRITE: &str = "48090002-1a48-11e9-ab14-d663bd873d93";
const EDIFIER_NOTIFY: &str = "48090001-1a48-11e9-ab14-d663bd873d93";
const BASEUS_WRITE: &str = "ee684b1a-1e9b-ed3e-ee55-f894667e92ac";
const BASEUS_NOTIFY: &str = "654b749c-e37f-ae1f-ebab-40ca133e3690";
const MAX_RX_BUFFER: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Model {
    EdifierW820NbPlus,
    BaseusBowie30,
    BaseusBlade2,
    BaseusEp10Pro,
    BaseusBowieMf1,
}

impl Model {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "edifier-w820nb-plus" => Ok(Self::EdifierW820NbPlus),
            "baseus-bowie-30" => Ok(Self::BaseusBowie30),
            "baseus-blade-2" => Ok(Self::BaseusBlade2),
            "baseus-ep10-pro" => Ok(Self::BaseusEp10Pro),
            "baseus-bowie-mf1" => Ok(Self::BaseusBowieMf1),
            _ => Err("Unsupported DeviceDeck device profile".into()),
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::EdifierW820NbPlus => "edifier-w820nb-plus",
            Self::BaseusBowie30 => "baseus-bowie-30",
            Self::BaseusBlade2 => "baseus-blade-2",
            Self::BaseusEp10Pro => "baseus-ep10-pro",
            Self::BaseusBowieMf1 => "baseus-bowie-mf1",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::EdifierW820NbPlus => "EDIFIER W820NB Plus",
            Self::BaseusBowie30 => "Baseus Bowie 30",
            Self::BaseusBlade2 => "Baseus Blade 2",
            Self::BaseusEp10Pro => "Baseus EP10 Pro",
            Self::BaseusBowieMf1 => "Baseus Bowie MF1",
        }
    }
    fn write_uuid(self) -> Uuid {
        Uuid::parse_str(match self {
            Self::EdifierW820NbPlus => EDIFIER_WRITE,
            Self::BaseusBowie30 | Self::BaseusBlade2 => BASEUS_WRITE,
            Self::BaseusEp10Pro | Self::BaseusBowieMf1 => BASEUS_WRITE,
        })
        .unwrap()
    }
    fn notify_uuid(self) -> Uuid {
        Uuid::parse_str(match self {
            Self::EdifierW820NbPlus => EDIFIER_NOTIFY,
            Self::BaseusBowie30 | Self::BaseusBlade2 => BASEUS_NOTIFY,
            Self::BaseusEp10Pro | Self::BaseusBowieMf1 => BASEUS_NOTIFY,
        })
        .unwrap()
    }

    fn service_uuid(self) -> Option<Uuid> {
        match self {
            Self::EdifierW820NbPlus => Some(Uuid::parse_str(EDIFIER_SERVICE).unwrap()),
            Self::BaseusBowie30
            | Self::BaseusBlade2
            | Self::BaseusEp10Pro
            | Self::BaseusBowieMf1 => None,
        }
    }
    fn matches(self, name: &str, services: &[Uuid]) -> bool {
        match self {
            Self::EdifierW820NbPlus => {
                services
                    .iter()
                    .any(|u| u.to_string().eq_ignore_ascii_case(EDIFIER_DISCOVERY))
                    || ["edifier w820nb plus", "w820nb plus"]
                        .iter()
                        .any(|n| name.eq_ignore_ascii_case(n))
            }
            Self::BaseusBowie30 => ["baseus bowie 30", "bowie 30"]
                .iter()
                .any(|n| name.eq_ignore_ascii_case(n)),
            Self::BaseusBlade2 => [
                "baseus blade 2",
                "baseus blade 2-140",
                "blade 2",
                "blade 2-140",
            ]
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n)),
            Self::BaseusEp10Pro | Self::BaseusBowieMf1 => false,
        }
    }

    fn uses_spp(self) -> bool {
        matches!(self, Self::BaseusEp10Pro | Self::BaseusBowieMf1)
    }

    fn matches_spp_name(self, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        match self {
            Self::BaseusEp10Pro => {
                name.contains("baseus") && name.contains("ep10") || name.contains("ep10 pro")
            }
            Self::BaseusBowieMf1 => name.contains("bowie mf1") || name.contains("baseus mf1"),
            _ => false,
        }
    }
}

pub struct AccessoryHub {
    found: Mutex<HashMap<String, (Peripheral, Model, String)>>,
    spp_found: Mutex<HashMap<String, (u64, Model, String)>>,
    session: tokio::sync::Mutex<Option<AccessorySession>>,
    spp_session: tokio::sync::Mutex<Option<BaseusSppSession>>,
}

impl AccessoryHub {
    fn new() -> Self {
        Self {
            found: Mutex::new(HashMap::new()),
            spp_found: Mutex::new(HashMap::new()),
            session: tokio::sync::Mutex::new(None),
            spp_session: tokio::sync::Mutex::new(None),
        }
    }
}

struct BaseusSppSession {
    model: Model,
    link: crate::bt::Link,
    buffer: Vec<u8>,
    battery: Option<(u8, u8)>,
    noise: Option<String>,
    gestures: Vec<(u8, u8, u8)>,
    gesture_children: HashMap<String, Vec<u8>>,
    device_name: String,
}

impl BaseusSppSession {
    fn request(&mut self, command: u8, payload: &[u8]) -> Result<Frame, String> {
        let request = encode_baseus_spp_request(command, payload);
        self.link.write(&request)?;
        let deadline = std::time::Instant::now() + Duration::from_millis(2500);
        let mut bytes = [0u8; 128];
        while std::time::Instant::now() < deadline {
            match self.link.read(&mut bytes)? {
                0 => std::thread::sleep(Duration::from_millis(5)),
                n => {
                    if let Some(frame) = decode_baseus_stream(&mut self.buffer, &bytes[..n])
                        .into_iter()
                        .find(|frame| frame.command == command as u16)
                    {
                        return Ok(frame);
                    }
                }
            }
        }
        Err(format!(
            "Baseus headset did not answer command 0x{command:02X}"
        ))
    }

    fn refresh(&mut self) -> Result<(), String> {
        let battery = self.request(0x02, &[])?;
        if battery.payload.len() != 4
            || battery.payload[0] > 100
            || battery.payload[1] != 0
            || battery.payload[2] > 100
            || battery.payload[3] != 1
        {
            return Err("Unrecognized Baseus battery response".into());
        }
        self.battery = Some((battery.payload[0], battery.payload[2]));
        if self.model == Model::BaseusEp10Pro {
            self.read_noise()?;
            let _ = self.read_gestures();
        } else if self.model == Model::BaseusBowieMf1 {
            let _ = self.read_gestures();
        }
        Ok(())
    }

    fn read_noise(&mut self) -> Result<(), String> {
        let frame = self.request(0x33, &[])?;
        self.noise = Some(
            match frame.payload.first().copied() {
                Some(0) => "off",
                Some(1) => "anc",
                Some(2) => "transparency",
                _ => return Err("Unknown Baseus noise state".into()),
            }
            .into(),
        );
        Ok(())
    }

    fn set_noise(&mut self, mode: &str) -> Result<(), String> {
        if self.model != Model::BaseusEp10Pro {
            return Err("Noise controls are not mapped for this model".into());
        }
        let (wire_mode, wire_type) = match mode {
            "off" => (0, 0xFF),
            "anc" => (1, 101),
            "transparency" => (2, 0xFF),
            _ => return Err("Unsupported Baseus noise mode".into()),
        };
        let ack = self.request(0x34, &[wire_mode, wire_type])?;
        if ack.payload.first() != Some(&1) {
            return Err("Baseus rejected the noise mode".into());
        }
        self.read_noise()?;
        if self.noise.as_deref() != Some(mode) {
            return Err("Baseus noise mode did not match readback".into());
        }
        Ok(())
    }

    fn read_gestures(&mut self) -> Result<(), String> {
        if self.model == Model::BaseusBowieMf1 {
            for layout in [0, 2, 1] {
                let frame = self.request(0x21, &[layout])?;
                if frame.payload.len() != 3 || frame.payload[0] != layout {
                    return Err("Unrecognized Bowie MF1 gesture response".into());
                }
                self.gestures.retain(|(_, existing_layout, _)| *existing_layout != layout);
                self.gestures.push((0, layout, frame.payload[1]));
                self.gestures.push((1, layout, frame.payload[2]));
            }
            return Ok(());
        }
        if self.model != Model::BaseusEp10Pro {
            return Err("Gesture protocol is unavailable for this model".into());
        }
        let mut mappings = self.gestures.clone();
        for button in 0..=1 {
            let frame = self.request(0x8C, &[button, 0xFF])?;
            if frame.payload.len() != 9 || frame.payload[0] != button {
                return Err("Unrecognized Baseus gesture response".into());
            }
            mappings.retain(|(side, _, _)| *side != button);
            for index in 0..4 {
                let layout = frame.payload[1 + index * 2];
                if layout > 3 { return Err("Unknown Baseus gesture layout".into()); }
                mappings.push((button, layout, frame.payload[2 + index * 2]));
            }
        }
        self.gestures = mappings;
        for (button, layout, action) in self.gestures.clone().into_iter().filter(|(_, _, action)| *action == 6) {
            let frame = self.request(0x8E, &[button, layout, 6])?;
            if frame.payload.len() < 4 || frame.payload[0] != button || frame.payload[1] != layout || frame.payload[2] != 6
                || frame.payload[3] < 2 || frame.payload.len() != 4 + frame.payload[3] as usize { return Err("Unrecognized Baseus gesture child response".into()); }
            let children = frame.payload[4..].to_vec();
            if children.iter().any(|id| ![0,1,2].contains(id)) || children.iter().collect::<std::collections::HashSet<_>>().len() != children.len() { return Err("Invalid Baseus gesture child values".into()); }
            self.gesture_children.insert(format!("{button}:{layout}"), children);
        }
        Ok(())
    }

    fn set_gesture_children(&mut self, button: u8, layout: u8, selected: &[u8]) -> Result<(), String> {
        if self.model != Model::BaseusEp10Pro || button > 1 || ![0,1,2,3].contains(&layout) || !(2..=3).contains(&selected.len())
            || selected.iter().any(|id| ![0,1,2].contains(id)) || selected.iter().collect::<std::collections::HashSet<_>>().len() != selected.len()
            || !self.gestures.contains(&(button, layout, 6)) { return Err("Unsupported Baseus gesture child selection".into()); }
        let mut payload = vec![button, layout, 6, selected.len() as u8]; payload.extend_from_slice(selected);
        let ack = self.request(0x8F, &payload)?;
        if ack.payload.first() != Some(&1) { return Err("Baseus rejected gesture child selection".into()); }
        let frame = self.request(0x8E, &[button, layout, 6])?;
        if frame.payload.len() < 4 || frame.payload[0] != button || frame.payload[1] != layout || frame.payload[2] != 6 || frame.payload.len() != 4 + frame.payload[3] as usize || &frame.payload[4..] != selected { return Err("Baseus gesture child readback mismatch".into()); }
        self.gesture_children.insert(format!("{button}:{layout}"), selected.to_vec()); Ok(())
    }

    fn set_gesture(&mut self, button: u8, layout: u8, action: u8) -> Result<(), String> {
        if button > 1 {
            return Err("Unsupported Baseus gesture".into());
        }
        if self.model == Model::BaseusBowieMf1 {
            let allowed: &[u8] = match layout {
                0 | 2 => &[0, 1, 2, 3, 4, 11, 12],
                1 => &[4, 5],
                _ => return Err("Unsupported Bowie MF1 gesture layout".into()),
            };
            if !allowed.contains(&action)
                || !self.gestures.iter().any(|(side, current_layout, _)| *side == button && *current_layout == layout) {
                return Err("Unsupported Bowie MF1 gesture action".into());
            }
            let left = self.gestures.iter().find(|(side, current_layout, _)| *side == 0 && *current_layout == layout).map(|(_, _, value)| *value)
                .ok_or("Missing Bowie MF1 left gesture state")?;
            let right = self.gestures.iter().find(|(side, current_layout, _)| *side == 1 && *current_layout == layout).map(|(_, _, value)| *value)
                .ok_or("Missing Bowie MF1 right gesture state")?;
            let next_left = if button == 0 { action } else { left };
            let next_right = if button == 1 { action } else { right };
            let ack = self.request(0x22, &[layout, next_left, next_right])?;
            if ack.payload.first() != Some(&1) { return Err("Baseus rejected the gesture".into()); }
            self.read_gestures()?;
            if !self.gestures.contains(&(button, layout, action)) { return Err("Bowie MF1 gesture readback mismatch".into()); }
            return Ok(());
        }
        if self.model != Model::BaseusEp10Pro || ![0, 1, 2, 3].contains(&layout) {
            return Err("Unsupported Baseus gesture".into());
        }
        let allowed: &[u8] = if layout == 3 { &[0, 1] } else { &[0, 1, 2, 3, 4, 6, 11, 12] };
        if !allowed.contains(&action) { return Err("Unsupported Baseus gesture action".into()); }
        let ack = self.request(0x8D, &[button, layout, action])?;
        if ack.payload.first() != Some(&1) { return Err("Baseus rejected the gesture".into()); }
        self.read_gestures()?;
        if !self.gestures.contains(&(button, layout, action)) { return Err("Baseus gesture readback mismatch".into()); }
        Ok(())
    }

    fn snapshot(&self, status: &str) -> Value {
        json!({ "status": status, "model": self.model.id(), "name": self.device_name,
            "battery": self.battery.map(|(left, right)| json!({"left": left, "right": right})),
            "noise": self.noise, "gestures": self.gestures.iter().map(|(button, layout, action)| json!({"button":button,"layout":layout,"action":action,"children":self.gesture_children.get(&format!("{button}:{layout}"))})).collect::<Vec<_>>(), "power": {} })
    }
}

fn encode_baseus_spp_request(command: u8, payload: &[u8]) -> Vec<u8> {
    let mut request = vec![0xBA, command];
    request.extend_from_slice(payload);
    request
}

pub fn start(app: &AppHandle) {
    app.manage(AccessoryHub::new());
}

struct AccessorySession {
    model: Model,
    peripheral: Peripheral,
    write: btleplug::api::Characteristic,
    rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    decoder: Decoder,
    edifier_protocol: EdifierBLEProtocol,
    battery: Option<u8>,
    noise: Option<String>,
    gestures: Vec<(u8, u8, u8)>,
    eq_presets: Vec<EdifierEqPreset>,
    selected_eq: Option<u8>,
    sleep_timer_options: Vec<u16>,
    sleep_timer_minutes: Option<u16>,
    game_mode_supported: bool,
    game_mode: Option<bool>,
    prompt_tone_supported: bool,
    prompt_tone_enabled: Option<bool>,
    prompt_volume_supported: bool,
    prompt_volume_level: Option<u8>,
    auto_power_off_supported: bool,
    auto_power_off: Option<bool>,
    disconnect_audio_supported: bool,
    repair_bluetooth_supported: bool,
    power_off_supported: bool,
    bluetooth_name_supported: bool,
    bluetooth_name_limit_bytes: usize,
    bluetooth_name: Option<String>,
    touch_sensitivity_supported: bool,
    touch_sensitivity_index: Option<u8>,
    touch_sensitivity_value: Option<u8>,
    touch_controls_supported: bool,
    touch_controls: Vec<EdifierTouchControl>,
    power: HashMap<u16, PowerStatus>,
    device_name: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EdifierEqPreset { id: u8, name: &'static str }

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EdifierTouchControl { index: u8, action: u8, noise_mask: Option<u8> }

fn edifier_eq_presets(profile: u8) -> Vec<EdifierEqPreset> {
    let names: &[&str] = match profile {
        1 => &["Classic", "Popular", "Classical", "Rock"],
        2 => &["Classic", "Popular", "Classical", "Rock", "Dance", "Blues"],
        3 => &["Classic", "Popular", "Rock"],
        4 => &["Classic", "Popular", "Classical", "Rock", "Custom"],
        6 => &["Standard", "Surround", "Game"],
        7 => &["Classic", "Dynamic", "Custom"],
        8 => &["Classic", "Dynamic", "Spatial", "Custom"],
        9 => &["Classic", "HiFi Camel", "STAX"],
        10 => &["Classic", "Monitor", "Variable", "Vocal", "Custom"],
        _ => &[],
    };
    names.iter().enumerate().map(|(id, name)| EdifierEqPreset { id: id as u8, name }).collect()
}

enum Decoder {
    Edifier(Vec<u8>, EdifierBLEProtocol),
    Baseus(Vec<u8>),
    BaseusPower(Vec<u8>),
}

#[derive(Clone, Copy, Debug)]
struct EdifierBLEProtocol {
    version: u8,
    xor_key: u8,
}

impl Default for EdifierBLEProtocol {
    fn default() -> Self { Self { version: 1, xor_key: 0 } }
}

impl EdifierBLEProtocol {
    fn from_manufacturer_data(data: Option<&HashMap<u16, Vec<u8>>>) -> Self {
        let Some(bytes) = data.and_then(|records| records.get(&0x07E0)) else { return Self::default() };
        if bytes.len() < 2 { return Self::default(); }
        let version = if bytes[bytes.len() - 2] > 1 { 2 } else { 1 };
        let xor_key = if bytes[bytes.len() - 1] == 16 { 0xA5 } else { 0 };
        Self { version, xor_key }
    }
}

#[derive(Clone, Copy, Debug)]
struct PowerStatus {
    multiplier: u8,
    value: u16,
}

#[derive(Debug)]
struct Frame {
    header: u8,
    command: u16,
    payload: Vec<u8>,
}

impl AccessorySession {
    fn encode_edifier(&self, payload: &[u8]) -> Vec<u8> {
        encode_edifier(payload, self.edifier_protocol)
    }

    async fn write(&self, bytes: &[u8]) -> Result<(), String> {
        self.peripheral
            .write(&self.write, bytes, WriteType::WithResponse)
            .await
            .map_err(|e| e.to_string())
    }
    fn feed(&mut self, bytes: &[u8]) -> Vec<Frame> {
        match (&mut self.decoder, self.model) {
            (Decoder::Edifier(buffer, protocol), Model::EdifierW820NbPlus) => {
                decode_edifier_stream(buffer, bytes, *protocol)
            }
            (Decoder::Baseus(buffer), Model::BaseusBowie30) => decode_baseus_stream(buffer, bytes),
            (Decoder::BaseusPower(buffer), Model::BaseusBlade2) => {
                decode_power_stream(buffer, bytes)
            }
            _ => vec![],
        }
    }
    async fn request(&mut self, packet: &[u8], expected: u16) -> Result<Frame, String> {
        self.write(packet).await?;
        tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(bytes) = self.rx.recv().await {
                for frame in self.feed(&bytes) {
                    if frame.command == expected {
                        return Ok(frame);
                    }
                }
            }
            Err("BLE notification stream ended".to_string())
        })
        .await
        .map_err(|_| format!("Device did not answer command 0x{expected:02X}"))?
    }
    async fn refresh(&mut self) -> Result<(), String> {
        match self.model {
            Model::EdifierW820NbPlus => {
                let d8 = [0xD8];
                if let Ok(capabilities) = self.request(&self.encode_edifier(&d8), 0xD8).await {
                    if let Some(profile) = capabilities.payload.get(13).copied() {
                        self.eq_presets = edifier_eq_presets(profile);
                    }
                    self.sleep_timer_options = match capabilities.payload.get(9).copied() {
                        Some(1) => vec![0, 10, 20, 30, 60, 90],
                        Some(2) => vec![0, 5, 15, 30, 60, 180],
                        _ => Vec::new(),
                    };
                    self.game_mode_supported = capabilities.payload.get(17).is_some_and(|value| *value & 0x40 != 0);
                    self.prompt_tone_supported = capabilities.payload.get(17).is_some_and(|value| *value & 0x02 != 0);
                    self.prompt_volume_supported = edifier_prompt_volume_supported(&capabilities.payload);
                    self.touch_sensitivity_supported = capabilities.payload.get(17).is_some_and(|value| *value & 0x08 != 0);
                    self.touch_controls_supported = capabilities.payload.get(16) == Some(&18);
                    self.auto_power_off_supported = capabilities.payload.get(8) == Some(&1);
                    self.disconnect_audio_supported = capabilities.payload.get(6) == Some(&1);
                    self.repair_bluetooth_supported = capabilities.payload.get(7) == Some(&1);
                    self.power_off_supported = capabilities.payload.get(10) == Some(&1);
                    self.bluetooth_name_supported = capabilities.payload.get(2) == Some(&1);
                    self.bluetooth_name_limit_bytes = capabilities.payload.get(3).copied().map(edifier_name_limit).unwrap_or(0);
                }
                if !self.eq_presets.is_empty() {
                    if let Ok(eq) = self.request(&self.encode_edifier(&[0xD5]), 0xD5).await {
                        if let Some(selected) = eq.payload.first().copied().filter(|id| self.eq_presets.iter().any(|p| p.id == *id)) {
                            self.selected_eq = Some(selected);
                        }
                    }
                }
                if !self.sleep_timer_options.is_empty() {
                    if let Ok(timer) = self.request(&self.encode_edifier(&[0xD3]), 0xD3).await {
                        if timer.payload.len() >= 2 {
                            let minutes = u16::from_be_bytes([timer.payload[0], timer.payload[1]]);
                            if self.sleep_timer_options.contains(&minutes) { self.sleep_timer_minutes = Some(minutes); }
                        }
                    }
                }
                if self.game_mode_supported {
                    if let Ok(game) = self.request(&self.encode_edifier(&[0x08]), 0x08).await {
                        self.game_mode = match game.payload.first() { Some(0) => Some(false), Some(1) => Some(true), _ => None };
                    }
                }
                if self.prompt_tone_supported {
                    if let Ok(state) = self.request(&self.encode_edifier(&[0xF9]), 0xF9).await {
                        self.prompt_tone_enabled = match state.payload.first() { Some(0) => Some(false), Some(1) => Some(true), _ => None };
                    }
                }
                if self.prompt_volume_supported {
                    if let Ok(level) = self.request(&self.encode_edifier(&[0x05]), 0x05).await {
                        self.prompt_volume_level = decode_edifier_prompt_level(&level.payload);
                    }
                }
                if self.auto_power_off_supported {
                    if let Ok(state) = self.request(&self.encode_edifier(&[0xD7]), 0xD7).await {
                        self.auto_power_off = match state.payload.first() { Some(0) => Some(false), Some(1) => Some(true), _ => None };
                    }
                }
                if self.bluetooth_name_supported {
                    if let Ok(name) = self.request(&self.encode_edifier(&[0xC9]), 0xC9).await {
                        self.bluetooth_name = decode_edifier_name(&name.payload);
                    }
                }
                if self.touch_sensitivity_supported {
                    if let Ok(sensitivity) = self.request(&self.encode_edifier(&[0xFD]), 0xFD).await {
                        if sensitivity.payload.len() >= 2 {
                            let index = sensitivity.payload[0];
                            let value = sensitivity.payload[1];
                            if edifier_sensitivity_max(index).is_some_and(|max| value <= max) {
                                self.touch_sensitivity_index = Some(index);
                                self.touch_sensitivity_value = Some(value);
                            }
                        }
                    }
                }
                if self.touch_controls_supported {
                    self.touch_controls.clear();
                    for index in 1..=4 {
                        if let Ok(control) = self.read_touch_control(index).await {
                            self.touch_controls.push(control);
                        }
                    }
                }
                let d0 = [0xD0];
                let battery = self.request(&self.encode_edifier(&d0), 0xD0).await?;
                let level = battery
                    .payload
                    .first()
                    .copied()
                    .ok_or("Empty Edifier battery response")?;
                if level > 100 {
                    return Err("Invalid Edifier battery percentage".into());
                }
                self.battery = Some(level);
                let cc = [0xCC];
                let noise = self.request(&self.encode_edifier(&cc), 0xCC).await?;
                self.noise = Some(
                    match noise.payload.first() {
                        Some(1) => "off",
                        Some(2) => "anc",
                        Some(3) => "transparency",
                        _ => return Err("Unrecognized Edifier noise state".into()),
                    }
                    .into(),
                );
            }
            Model::BaseusBowie30 => {
                let battery = self.request(&[0xBA, 0x02], 0x02).await?;
                if battery.payload.len() != 4
                    || battery.payload[0] > 100
                    || battery.payload[1] != 0
                    || battery.payload[2] > 100
                    || battery.payload[3] != 1
                {
                    return Err("Unrecognized Baseus battery response".into());
                }
                self.battery = Some(battery.payload[0]);
                let noise = self.request(&[0xBA, 0x33], 0x33).await?;
                if noise.payload.len() != 4 {
                    return Err("Unrecognized Baseus noise response".into());
                }
                self.noise = Some(
                    match (noise.payload[0], noise.payload[1]) {
                        (0, _) => "off",
                        (1, 8) => "commute",
                        (1, 10) => "indoor",
                        (1, 9) => "outdoor",
                        (2, _) => "transparency",
                        _ => return Err("Unknown Baseus noise state".into()),
                    }
                    .into(),
                );
                let _ = self.read_legacy_gestures().await;
            }
            Model::BaseusBlade2 => {
                let mut codes: Vec<u16> = (0x0001..=0x000D).collect();
                codes.extend([
                    0x0010, 0x0011, 0x0012, 0x0013, 0x0014, 0x0015, 0x0016, 0x001E, 0x001F, 0x0020,
                    0x0022, 0x0023,
                ]);
                if self.device_name.contains("140") {
                    codes.extend([0x0024, 0x0025, 0x0026]);
                    codes.extend(0x0028..=0x002D);
                    codes.extend(0x0031..=0x0036);
                }
                for code in codes {
                    self.read_power(code).await?;
                }
                self.battery = None;
                self.noise = None;
            }
            Model::BaseusEp10Pro | Model::BaseusBowieMf1 => {
                return Err("Baseus SPP profiles use the paired-device session".into());
            }
        }
        Ok(())
    }
    async fn read_legacy_gesture_layout(&mut self, layout: u8) -> Result<(), String> {
        if self.model != Model::BaseusBowie30 || ![0, 1, 2].contains(&layout) {
            return Err("Unsupported Bowie 30 gesture layout".into());
        }
        let frame = self.request(&[0xBA, 0x21, layout], 0x21).await?;
        if frame.payload.len() != 3 || frame.payload[0] != layout {
            return Err("Unrecognized Bowie 30 gesture response".into());
        }
        self.gestures.retain(|(_, current, _)| *current != layout);
        self.gestures.push((0, layout, frame.payload[1]));
        self.gestures.push((1, layout, frame.payload[2]));
        Ok(())
    }
    async fn read_legacy_gestures(&mut self) -> Result<(), String> {
        for layout in [0, 2, 1] {
            self.read_legacy_gesture_layout(layout).await?;
        }
        Ok(())
    }
    async fn set_gesture(&mut self, button: u8, layout: u8, action: u8) -> Result<(), String> {
        if self.model != Model::BaseusBowie30 || button > 1 || ![0, 1, 2].contains(&layout)
            || ![0, 1, 2, 3, 4, 6].contains(&action)
            || !self.gestures.iter().any(|(side, current, _)| *side == button && *current == layout) {
            return Err("Unsupported Bowie 30 gesture".into());
        }
        let left = self.gestures.iter().find(|(side, current, _)| *side == 0 && *current == layout).map(|(_, _, value)| *value)
            .ok_or("Missing Bowie 30 left gesture state")?;
        let right = self.gestures.iter().find(|(side, current, _)| *side == 1 && *current == layout).map(|(_, _, value)| *value)
            .ok_or("Missing Bowie 30 right gesture state")?;
        let ack = self.request(&[0xBA, 0x22, layout, if button == 0 { action } else { left }, if button == 1 { action } else { right }], 0x22).await?;
        if ack.payload.first() != Some(&1) { return Err("Baseus rejected the gesture".into()); }
        self.read_legacy_gesture_layout(layout).await?;
        if !self.gestures.contains(&(button, layout, action)) { return Err("Bowie 30 gesture readback mismatch".into()); }
        Ok(())
    }
    async fn set_noise(&mut self, value: &str) -> Result<(), String> {
        if self.noise.is_none() {
            return Err("Read the current mode before changing it".into());
        }
        match self.model {
            Model::EdifierW820NbPlus => {
                let wire = match value {
                    "off" => 1u8,
                    "anc" => 2,
                    "transparency" => 3,
                    _ => return Err("Unsupported EDIFIER mode".into()),
                };
                let packet = self.encode_edifier(&[0xC1, wire]);
                let ack = self.request(&packet, 0xC1).await?;
                if ack.header != 0xCC {
                    return Err("EDIFIER did not acknowledge the mode".into());
                }
                let cc = [0xCC];
                let state = self.request(&self.encode_edifier(&cc), 0xCC).await?;
                let actual = match state.payload.first() {
                    Some(1) => "off",
                    Some(2) => "anc",
                    Some(3) => "transparency",
                    _ => return Err("Unknown EDIFIER mode after write".into()),
                };
                self.noise = Some(actual.into());
                if actual != value {
                    return Err("EDIFIER mode did not match readback".into());
                }
            }
            Model::BaseusBowie30 => {
                let (mode, ty) = match value {
                    "off" => (0, 0xFF),
                    "commute" => (1, 8),
                    "indoor" => (1, 10),
                    "outdoor" => (1, 9),
                    "transparency" => (2, 0xFF),
                    _ => return Err("Unsupported Baseus mode".into()),
                };
                let ack = self.request(&[0xBA, 0x34, mode, ty], 0x34).await?;
                if ack.payload != [1] {
                    return Err("Baseus rejected the mode".into());
                }
                let noise = self.request(&[0xBA, 0x33], 0x33).await?;
                let actual = match (
                    noise.payload.first().copied(),
                    noise.payload.get(1).copied(),
                ) {
                    (Some(0), _) => "off",
                    (Some(1), Some(8)) => "commute",
                    (Some(1), Some(10)) => "indoor",
                    (Some(1), Some(9)) => "outdoor",
                    (Some(2), _) => "transparency",
                    _ => return Err("Unknown Baseus mode after write".into()),
                };
                self.noise = Some(actual.into());
                if actual != value {
                    return Err("Baseus mode did not match readback".into());
                }
            }
            Model::BaseusBlade2 => {
                return Err("Blade 2 is a power accessory; noise modes are not supported".into())
            }
            Model::BaseusEp10Pro | Model::BaseusBowieMf1 => {
                return Err("Baseus SPP profiles use the paired-device session".into())
            }
        }
        Ok(())
    }
    async fn set_equalizer(&mut self, id: u8) -> Result<(), String> {
        if self.model != Model::EdifierW820NbPlus || self.eq_presets.iter().all(|preset| preset.id != id) || self.selected_eq.is_none() {
            return Err("No supported Edifier EQ profile has been read".into());
        }
        let ack = self.request(&self.encode_edifier(&[0xC4, id]), 0xC4).await?;
        if ack.header != 0xCC { return Err("Edifier did not acknowledge the EQ preset".into()); }
        let state = self.request(&self.encode_edifier(&[0xD5]), 0xD5).await?;
        let actual = state.payload.first().copied().filter(|value| self.eq_presets.iter().any(|preset| preset.id == *value));
        self.selected_eq = actual;
        if actual != Some(id) { return Err("Edifier EQ preset did not match device readback".into()); }
        Ok(())
    }
    async fn set_sleep_timer(&mut self, minutes: u16) -> Result<(), String> {
        if self.model != Model::EdifierW820NbPlus || !self.sleep_timer_options.contains(&minutes) {
            return Err("Sleep timer value is not supported by this firmware".into());
        }
        let packet = if minutes == 0 { vec![0xD2] } else { vec![0xD1, (minutes >> 8) as u8, minutes as u8] };
        let ack = self.request(&self.encode_edifier(&packet), if minutes == 0 { 0xD2 } else { 0xD1 }).await?;
        if ack.header != 0xCC { return Err("Edifier did not acknowledge the sleep timer".into()); }
        let state = self.request(&self.encode_edifier(&[0xD3]), 0xD3).await?;
        if state.payload.len() < 2 { return Err("Invalid sleep timer readback".into()); }
        let actual = u16::from_be_bytes([state.payload[0], state.payload[1]]);
        self.sleep_timer_minutes = Some(actual);
        if actual != minutes { return Err("Sleep timer did not match device readback".into()); }
        Ok(())
    }
    async fn set_game_mode(&mut self, enabled: bool) -> Result<(), String> {
        if self.model != Model::EdifierW820NbPlus || !self.game_mode_supported || self.game_mode.is_none() {
            return Err("Game mode is not available in the reported Edifier profile".into());
        }
        let ack = self.request(&self.encode_edifier(&[0x09, u8::from(enabled)]), 0x09).await?;
        if ack.header != 0xCC { return Err("Edifier did not acknowledge game mode".into()); }
        let state = self.request(&self.encode_edifier(&[0x08]), 0x08).await?;
        self.game_mode = match state.payload.first() { Some(0) => Some(false), Some(1) => Some(true), _ => None };
        if self.game_mode != Some(enabled) { return Err("Edifier game mode did not match device readback".into()); }
        Ok(())
    }

    async fn set_prompt_tone(&mut self, enabled: bool) -> Result<(), String> {
        if self.model != Model::EdifierW820NbPlus || !self.prompt_tone_supported || self.prompt_tone_enabled.is_none() {
            return Err("Prompt tones are not available in the reported Edifier profile".into());
        }
        let ack = self.request(&self.encode_edifier(&[0xFA, u8::from(enabled)]), 0xFA).await?;
        if ack.header != 0xCC { return Err("Edifier did not acknowledge prompt tones".into()); }
        let state = self.request(&self.encode_edifier(&[0xF9]), 0xF9).await?;
        self.prompt_tone_enabled = match state.payload.first() { Some(0) => Some(false), Some(1) => Some(true), _ => None };
        if self.prompt_tone_enabled != Some(enabled) { return Err("Prompt tone setting did not match device readback".into()); }
        Ok(())
    }

    async fn set_prompt_volume(&mut self, level: u8) -> Result<(), String> {
        if self.model != Model::EdifierW820NbPlus || !self.prompt_volume_supported || self.prompt_volume_level.is_none() || level > 15 {
            return Err("Voice prompt volume is not available in the reported Edifier profile".into());
        }
        let ack = self.request(&self.encode_edifier(&[0x06, level]), 0x06).await?;
        if ack.header != 0xCC { return Err("Edifier did not acknowledge voice prompt volume".into()); }
        let state = self.request(&self.encode_edifier(&[0x05]), 0x05).await?;
        self.prompt_volume_level = decode_edifier_prompt_level(&state.payload);
        if self.prompt_volume_level != Some(level) { return Err("Voice prompt volume did not match device readback".into()); }
        Ok(())
    }

    async fn set_touch_sensitivity(&mut self, value: u8) -> Result<(), String> {
        let index = self.touch_sensitivity_index.ok_or("Touch sensitivity has not been read")?;
        let max = edifier_sensitivity_max(index).ok_or("Unsupported Edifier sensitivity profile")?;
        if self.model != Model::EdifierW820NbPlus || !self.touch_sensitivity_supported || self.touch_sensitivity_value.is_none() || value > max {
            return Err("Touch sensitivity is unavailable in the reported Edifier profile".into());
        }
        let ack = self.request(&self.encode_edifier(&[0xFE, index, value]), 0xFE).await?;
        if ack.header != 0xCC { return Err("Edifier did not acknowledge touch sensitivity".into()); }
        let state = self.request(&self.encode_edifier(&[0xFD]), 0xFD).await?;
        if state.payload.len() < 2 || state.payload[0] != index || state.payload[1] != value {
            return Err("Edifier touch sensitivity did not match device readback".into());
        }
        self.touch_sensitivity_value = Some(value);
        Ok(())
    }
    async fn read_touch_control(&mut self, index: u8) -> Result<EdifierTouchControl, String> {
        if self.model != Model::EdifierW820NbPlus || !self.touch_controls_supported || !(1..=4).contains(&index) {
            return Err("Touch control is not supported by the reported W820NB Plus profile".into());
        }
        let frame = self.request(&self.encode_edifier(&[0xF0, index]), 0xF0).await?;
        let p = &frame.payload;
        if p.len() < 2 || p[0] != index || !(1..=6).contains(&p[1])
            || (p[1] == 6 && p.len() != 3) || (p[1] != 6 && p.len() != 2) {
            return Err("Edifier touch-control response was not recognized".into());
        }
        let noise_mask = if p[1] == 6 {
            let mask = p[2];
            if mask & 0xE0 != 0 || mask.count_ones() < 2 || mask.count_ones() > 3 {
                return Err("Edifier touch-control mode mask was invalid".into());
            }
            Some(mask)
        } else { None };
        Ok(EdifierTouchControl { index, action: p[1], noise_mask })
    }
    async fn set_touch_control(&mut self, index: u8, action: u8, modes: &[u8]) -> Result<(), String> {
        if self.model != Model::EdifierW820NbPlus || !self.touch_controls_supported || !(1..=4).contains(&index) || !(1..=6).contains(&action) {
            return Err("Touch control is not supported by the reported W820NB Plus profile".into());
        }
        let mut payload = vec![0xF1, index];
        let expected_mask = if action == 6 {
            if !(2..=3).contains(&modes.len()) || modes.iter().any(|mode| *mode > 4)
                || modes.iter().collect::<std::collections::HashSet<_>>().len() != modes.len() {
                return Err("Choose two or three distinct supported noise modes".into());
            }
            let mask = modes.iter().fold(0u8, |bits, mode| bits | (1 << mode));
            payload.extend_from_slice(&[mask, *modes.last().unwrap()]);
            Some(mask)
        } else {
            if !modes.is_empty() { return Err("Noise-mode choices only apply to the cycle action".into()); }
            payload.push(action);
            None
        };
        let ack = self.request(&self.encode_edifier(&payload), 0xF1).await?;
        if ack.header != 0xCC { return Err("Edifier did not acknowledge the touch control".into()); }
        let actual = self.read_touch_control(index).await?;
        if actual.action != action || actual.noise_mask != expected_mask { return Err("Edifier touch control did not match device readback".into()); }
        self.touch_controls.retain(|control| control.index != index);
        self.touch_controls.push(actual);
        Ok(())
    }
    async fn set_auto_power_off(&mut self, enabled: bool) -> Result<(), String> {
        if self.model != Model::EdifierW820NbPlus || !self.auto_power_off_supported || self.auto_power_off.is_none() {
            return Err("Automatic power-off is not available in the reported Edifier profile".into());
        }
        let ack = self.request(&self.encode_edifier(&[0xD6, u8::from(enabled)]), 0xD6).await?;
        if ack.header != 0xCC { return Err("Edifier did not acknowledge automatic power-off".into()); }
        let state = self.request(&self.encode_edifier(&[0xD7]), 0xD7).await?;
        self.auto_power_off = match state.payload.first() { Some(0) => Some(false), Some(1) => Some(true), _ => None };
        if self.auto_power_off != Some(enabled) { return Err("Automatic power-off did not match device readback".into()); }
        Ok(())
    }
    async fn edifier_bluetooth_action(&self, action: &str) -> Result<u8, String> {
        if self.model != Model::EdifierW820NbPlus {
            return Err("Bluetooth actions are available only for EDIFIER W820NB Plus".into());
        }
        let (command, supported) = match action {
            "disconnectAudio" => (0xCD, self.disconnect_audio_supported),
            "repairBluetooth" => (0xCF, self.repair_bluetooth_supported),
            "powerOff" => (0xCE, self.power_off_supported),
            _ => return Err("Unknown EDIFIER action".into()),
        };
        if !supported {
            return Err("The connected headset did not report support for this action".into());
        }
        self.write(&self.encode_edifier(&[command])).await?;
        Ok(command)
    }
    async fn set_bluetooth_name(&mut self, name: &str) -> Result<(), String> {
        let bytes = name.as_bytes();
        if self.model != Model::EdifierW820NbPlus || !self.bluetooth_name_supported || self.bluetooth_name_limit_bytes == 0
            || name.trim().is_empty() || bytes.len() > self.bluetooth_name_limit_bytes || name.chars().any(char::is_control) {
            return Err("Bluetooth name is not supported or exceeds the reported UTF-8 byte limit".into());
        }
        let mut payload = vec![0xCA];
        payload.extend_from_slice(bytes);
        let ack = self.request(&self.encode_edifier(&payload), 0xCA).await?;
        if ack.header != 0xCC { return Err("Edifier did not acknowledge the Bluetooth name change".into()); }
        let state = self.request(&self.encode_edifier(&[0xC9]), 0xC9).await?;
        self.bluetooth_name = decode_edifier_name(&state.payload);
        if self.bluetooth_name.as_deref() != Some(name) { return Err("Bluetooth name did not match device readback".into()); }
        Ok(())
    }
    async fn read_power(&mut self, code: u16) -> Result<PowerStatus, String> {
        let frame = self.request(&encode_power_read(code), code).await?;
        if frame.payload.len() != 3 {
            return Err(format!("Invalid Blade 2 status response for 0x{code:04X}"));
        }
        let status = PowerStatus {
            multiplier: frame.payload[0],
            value: u16::from_be_bytes([frame.payload[1], frame.payload[2]]),
        };
        self.power.insert(code, status);
        Ok(status)
    }
    async fn set_power(&mut self, code: u16, value: u16) -> Result<(), String> {
        let valid = match code {
            0x0020 => value <= 2,
            0x0022 => value <= 1,
            0x001E | 0x001F => value <= 1440,
            _ => false,
        };
        if !valid {
            return Err("Unsupported Blade 2 setting or value".into());
        }
        self.write(&encode_power_write(code, value)).await?;
        let actual = self.read_power(code).await?;
        if actual.value != value {
            return Err("Blade 2 setting did not match device readback".into());
        }
        Ok(())
    }
    async fn restore_power_defaults(&self) -> Result<(), String> {
        if self.model != Model::BaseusBlade2 {
            return Err("Connected device is not a Blade 2 powerbank".into());
        }
        // Baseus Intelligent's Blade 2 reset path writes 0000 to 000E. The app
        // exposes no reset status/readback, so the caller must report this as sent.
        self.write(&encode_power_write(0x000E, 0)).await
    }
    async fn set_power_port_mode(&mut self, port: u8, mode: u8) -> Result<(), String> {
        if self.model != Model::BaseusBlade2 || !(1..=2).contains(&port) || mode > 2 {
            return Err("Unsupported Blade 2 port mode".into());
        }
        let current = self.read_power(0x0010).await?.value;
        let shift = u32::from((port - 1) * 2);
        let current_mode = ((current >> shift) & 0x03) as u8;
        if current_mode == 3 {
            return Err(format!("Blade 2 reported an unknown C{port} port state"));
        }
        if current_mode != mode {
            let command = |selected: u8| -> Option<u16> {
                match (port, selected) {
                    (1, 1) => Some(0x0018), // C1 output / discharge
                    (1, 2) => Some(0x0019), // C1 input / charge
                    (2, 1) => Some(0x001A), // C2 output / discharge
                    (2, 2) => Some(0x001B), // C2 input / charge
                    _ => None,
                }
            };
            if current_mode != 0 {
                let off = command(current_mode).ok_or("Unsupported Blade 2 port state")?;
                self.write(&encode_power_write(off, 0)).await?;
            }
            if mode != 0 {
                let on = command(mode).ok_or("Unsupported Blade 2 port mode")?;
                self.write(&encode_power_write(on, 1)).await?;
            }
        }
        let actual = self.read_power(0x0010).await?.value;
        if ((actual >> shift) & 0x03) as u8 != mode {
            return Err(format!("Blade 2 C{port} port mode did not match status readback"));
        }
        Ok(())
    }
    async fn set_power_text(&self, text: &str) -> Result<(), String> {
        let bytes = text.as_bytes();
        if text.trim().is_empty() || bytes.len() > 255 {
            return Err("Display text must contain 1–255 UTF-8 bytes".into());
        }
        self.write(&encode_power_text(bytes)).await
    }
    fn snapshot(&self, status: &str) -> Value {
        let power: HashMap<String, Value> = self
            .power
            .iter()
            .map(|(code, v)| {
                (
                    format!("{code:04X}"),
                    json!({"multiplier":v.multiplier,"value":v.value}),
                )
            })
            .collect();
        json!({ "status": status, "model": self.model.id(), "name": self.device_name, "edifierBleVersion": (self.model == Model::EdifierW820NbPlus).then_some(self.edifier_protocol.version), "edifierEncryption": (self.model == Model::EdifierW820NbPlus).then_some(if self.edifier_protocol.xor_key == 0 { "none" } else { "Encryption10" }), "battery": self.battery, "noise": self.noise, "gestures": self.gestures.iter().map(|(button, layout, action)| json!({"button":button,"layout":layout,"action":action})).collect::<Vec<_>>(), "eqPresets": self.eq_presets, "selectedEq": self.selected_eq, "sleepTimerOptions": self.sleep_timer_options, "sleepTimerMinutes": self.sleep_timer_minutes, "gameModeSupported": self.game_mode_supported, "gameMode": self.game_mode, "promptToneSupported": self.prompt_tone_supported, "promptToneEnabled": self.prompt_tone_enabled, "promptVolumeSupported": self.prompt_volume_supported, "promptVolumeLevel": self.prompt_volume_level, "autoPowerOffSupported": self.auto_power_off_supported, "autoPowerOff": self.auto_power_off, "disconnectAudioSupported": self.disconnect_audio_supported, "repairBluetoothSupported": self.repair_bluetooth_supported, "powerOffSupported": self.power_off_supported, "bluetoothNameSupported": self.bluetooth_name_supported, "bluetoothNameLimitBytes": self.bluetooth_name_limit_bytes, "bluetoothName": self.bluetooth_name, "touchSensitivitySupported": self.touch_sensitivity_supported, "touchSensitivityIndex": self.touch_sensitivity_index, "touchSensitivity": self.touch_sensitivity_value, "touchControlsSupported": self.touch_controls_supported, "touchControls": self.touch_controls, "power": power })
    }
}

fn encode_edifier(payload: &[u8], protocol: EdifierBLEProtocol) -> Vec<u8> {
    if protocol.version == 2 {
        let command = payload.first().copied().unwrap_or(0);
        let parameters = payload.get(1..).unwrap_or_default();
        let mut frame = vec![0xAA, 0xEC, command, (parameters.len() >> 8) as u8, parameters.len() as u8];
        frame.extend(parameters.iter().map(|byte| *byte ^ protocol.xor_key));
        let checksum = frame.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte));
        frame.push(checksum);
        frame
    } else {
        encode_edifier_header(0xAA, payload)
    }
}

fn edifier_name_limit(mode: u8) -> usize {
    match mode { 1 | 6 => 24, 2 | 5 => 30, 3 => 29, 4 => 35, _ => 0 }
}

fn edifier_prompt_volume_supported(capabilities: &[u8]) -> bool {
    capabilities.get(17).is_some_and(|value| value & 0x10 != 0)
}

fn decode_edifier_prompt_level(payload: &[u8]) -> Option<u8> {
    payload.first().copied().filter(|level| *level <= 15)
}

fn edifier_sensitivity_max(index: u8) -> Option<u8> {
    match index { 1 => Some(31), 2 => Some(15), _ => None }
}

fn decode_edifier_name(payload: &[u8]) -> Option<String> {
    let end = payload.iter().rposition(|byte| *byte != 0).map_or(0, |index| index + 1);
    String::from_utf8(payload[..end].to_vec()).ok().filter(|name| !name.trim().is_empty())
}

fn encode_edifier_header(header: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![header, payload.len() as u8];
    out.extend_from_slice(payload);
    let sum = out.iter().fold(0x2019u16, |s, b| s.wrapping_add(*b as u16));
    out.extend_from_slice(&sum.to_be_bytes());
    out
}

fn decode_edifier_stream(buffer: &mut Vec<u8>, input: &[u8], protocol: EdifierBLEProtocol) -> Vec<Frame> {
    if buffer.len().saturating_add(input.len()) > MAX_RX_BUFFER {
        buffer.clear();
    }
    buffer.extend_from_slice(input);
    let mut frames = Vec::new();
    while !buffer.is_empty() {
        let Some(start) = buffer.iter().position(|b| matches!(*b, 0xBB | 0xCC)) else {
            buffer.clear();
            break;
        };
        if start > 0 {
            buffer.drain(..start);
        }
        if buffer.len() < 2 {
            break;
        }
        let size = if protocol.version == 2 {
            if buffer.len() < 5 { break; }
            if buffer[1] != 0xEC { buffer.remove(0); continue; }
            u16::from_be_bytes([buffer[3], buffer[4]]) as usize + 6
        } else {
            buffer[1] as usize + 4
        };
        if buffer.len() < size {
            break;
        }
        let raw = &buffer[..size];
        let valid = if protocol.version == 2 {
            raw[..size - 1].iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) == raw[size - 1]
        } else {
            let sum = raw[..size - 2]
                .iter()
                .fold(0x2019u16, |s, b| s.wrapping_add(*b as u16));
            u16::from_be_bytes([raw[size - 2], raw[size - 1]]) == sum && raw[1] >= 1
        };
        if valid {
            frames.push(Frame {
                header: raw[0],
                command: raw[2] as u16,
                payload: if protocol.version == 2 {
                    raw[5..size - 1].iter().map(|byte| *byte ^ protocol.xor_key).collect()
                } else {
                    raw[3..size - 2].to_vec()
                },
            });
            buffer.drain(..size);
        } else {
            buffer.remove(0);
        }
    }
    frames
}

fn decode_baseus_stream(buffer: &mut Vec<u8>, input: &[u8]) -> Vec<Frame> {
    if buffer.len().saturating_add(input.len()) > MAX_RX_BUFFER {
        buffer.clear();
    }
    buffer.extend_from_slice(input);
    let mut frames = Vec::new();
    while buffer.len() >= 2 {
        let Some(start) = buffer.iter().position(|b| *b == 0xAA) else {
            buffer.clear();
            break;
        };
        if start > 0 {
            buffer.drain(..start);
        }
        if buffer.len() < 2 {
            break;
        }
        let command = buffer[1];
        let size = match command {
            0x21 => 5,
            0x22 => 3,
            0x02 | 0x33 => 6,
            0x34 | 0x35 | 0x8B | 0x8D | 0x8F => 3,
            0x8C => 11,
            0x8E => {
                if buffer.len() < 6 { break; }
                6 + buffer[5] as usize
            }
            _ => {
                buffer.remove(0);
                continue;
            }
        };
        if buffer.len() < size {
            break;
        }
        frames.push(Frame {
            header: 0xAA,
            command: command as u16,
            payload: buffer[2..size].to_vec(),
        });
        buffer.drain(..size);
    }
    frames
}

fn encode_power_read(code: u16) -> Vec<u8> {
    power_crc(
        vec![0x9A, 0xAA, 0x03]
            .into_iter()
            .chain(code.to_be_bytes())
            .chain([0, 1])
            .collect(),
    )
}
fn encode_power_write(code: u16, value: u16) -> Vec<u8> {
    power_crc(
        vec![0x9A, 0xAA, 0x10]
            .into_iter()
            .chain(code.to_be_bytes())
            .chain([0, 1, 2, 1])
            .chain(value.to_be_bytes())
            .collect(),
    )
}
fn encode_power_text(text: &[u8]) -> Vec<u8> {
    power_crc(
        vec![0x9A, 0xAA, 0x10, 0, 0x30, 0, 1, text.len() as u8, 1]
            .into_iter()
            .chain(text.iter().copied())
            .collect(),
    )
}
fn power_crc(mut bytes: Vec<u8>) -> Vec<u8> {
    let crc = crc16(&bytes);
    bytes.extend_from_slice(&crc.to_be_bytes());
    bytes
}
fn crc16(bytes: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    for byte in bytes {
        crc ^= *byte as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}
fn decode_power_stream(buffer: &mut Vec<u8>, input: &[u8]) -> Vec<Frame> {
    if buffer.len().saturating_add(input.len()) > MAX_RX_BUFFER {
        buffer.clear();
    }
    buffer.extend_from_slice(input);
    let mut frames = Vec::new();
    while buffer.len() >= 2 {
        let Some(start) = buffer.windows(2).position(|pair| pair == [0x9A, 0xAA]) else {
            let keep = buffer.last() == Some(&0x9A);
            *buffer = if keep { vec![0x9A] } else { Vec::new() };
            break;
        };
        if start > 0 {
            buffer.drain(..start);
        }
        if buffer.len() < 11 {
            break;
        }
        let raw = &buffer[..11];
        let expected = u16::from_be_bytes([raw[9], raw[10]]);
        if raw[2] == 0x03 && crc16(&raw[..9]) == expected && raw[5] == 0 {
            let code = u16::from_be_bytes([raw[3], raw[4]]);
            frames.push(Frame {
                header: 0x9A,
                command: code,
                payload: raw[6..9].to_vec(),
            });
            buffer.drain(..11);
        } else {
            buffer.remove(0);
        }
    }
    frames
}

fn emit(app: &AppHandle, value: Value) {
    let _ = app.emit("accessory", value);
}

#[tauri::command]
pub async fn accessory_scan(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    model: String,
) -> Result<Value, String> {
    let model = Model::parse(&model)?;
    hub.found.lock().unwrap().clear();
    hub.spp_found.lock().unwrap().clear();
    emit(
        &app,
        json!({ "status": "scanning", "model": model.id(), "devices": [] }),
    );
    if model.uses_spp() {
        // RFCOMM services are not advertisements. Query the system's paired-device list,
        // then let the model-specific SPP session validate that the service really answers.
        let paired = tokio::task::spawn_blocking(crate::bt::paired)
            .await
            .map_err(|e| format!("Could not enumerate paired Bluetooth devices: {e}"))?;
        let mut devices = Vec::new();
        for device in paired {
            if !model.matches_spp_name(&device.name) {
                continue;
            }
            let id = format!("{:012X}", device.addr);
            hub.spp_found
                .lock()
                .unwrap()
                .insert(id.clone(), (device.addr, model, device.name.clone()));
            devices.push(json!({ "id": id, "name": device.name }));
        }
        let status = if devices.is_empty() {
            "not-found"
        } else {
            "found"
        };
        let result = json!({ "status": status, "model": model.id(), "devices": devices });
        emit(&app, result.clone());
        return Ok(result);
    }
    let manager = Manager::new()
        .await
        .map_err(|e| format!("Bluetooth unavailable: {e}"))?;
    let adapter = manager
        .adapters()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .next()
        .ok_or("No Bluetooth adapter found")?;
    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|e| format!("Could not start Bluetooth scan: {e}"))?;
    hub.found.lock().unwrap().clear();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(12);
    let mut seen: HashMap<String, Value> = HashMap::new();
    while tokio::time::Instant::now() < deadline {
        for peripheral in adapter.peripherals().await.map_err(|e| e.to_string())? {
            let Ok(Some(properties)) = peripheral.properties().await else {
                continue;
            };
            let advertised_name = properties.local_name.unwrap_or_default();
            if !model.matches(&advertised_name, &properties.services) {
                continue;
            }
            let name = if advertised_name.is_empty() {
                model.name().into()
            } else {
                advertised_name
            };
            let id = peripheral.id().to_string();
            seen.insert(id.clone(), json!({ "id": id, "name": name }));
            hub.found
                .lock()
                .unwrap()
                .insert(id, (peripheral, model, name));
        }
        emit(
            &app,
            json!({ "status": "scanning", "model": model.id(), "devices": seen.values().cloned().collect::<Vec<_>>() }),
        );
        tokio::time::sleep(Duration::from_millis(350)).await;
    }
    adapter.stop_scan().await.ok();
    let devices: Vec<Value> = seen.into_values().collect();
    let status = if devices.is_empty() {
        "not-found"
    } else {
        "found"
    };
    let result = json!({ "status": status, "model": model.id(), "devices": devices });
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_connect(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    id: String,
) -> Result<Value, String> {
    let spp = hub.spp_found.lock().unwrap().get(&id).cloned();
    if let Some((addr, model, name)) = spp {
        emit(
            &app,
            json!({ "status": "connecting", "model": model.id(), "name": name }),
        );
        *hub.session.lock().await = None;
        *hub.spp_session.lock().await = None;
        let link = tokio::task::spawn_blocking(move || {
            crate::bt::connect_service(addr, crate::bt::STANDARD_SPP_UUID)
        })
        .await
        .map_err(|e| format!("Could not start Bluetooth SPP connection: {e}"))??;
        let mut session = BaseusSppSession {
            model,
            link,
            buffer: Vec::new(),
            battery: None,
            noise: None,
            gestures: Vec::new(),
            gesture_children: HashMap::new(),
            device_name: name,
        };
        session.refresh()?;
        let result = session.snapshot("connected");
        *hub.spp_session.lock().await = Some(session);
        emit(&app, result.clone());
        return Ok(result);
    }
    let (peripheral, model, name) = hub
        .found
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or("Device expired; scan again")?;
    emit(
        &app,
        json!({ "status": "connecting", "model": model.id(), "name": name }),
    );
    let mut active = hub.session.lock().await;
    *hub.spp_session.lock().await = None;
    *active = None;
    if !peripheral.is_connected().await.unwrap_or(false) {
        peripheral
            .connect()
            .await
            .map_err(|e| format!("GATT connect failed: {e}"))?;
    }
    peripheral
        .discover_services()
        .await
        .map_err(|e| format!("GATT service discovery failed: {e}"))?;
    let chars = peripheral.characteristics();
    let service_uuid = model.service_uuid();
    let write = chars
        .iter()
        .find(|c| {
            c.uuid == model.write_uuid()
                && service_uuid.map_or(true, |service| c.service_uuid == service)
        })
        .cloned()
        .ok_or("Device control write characteristic was not found")?;
    let notify = chars
        .iter()
        .find(|c| {
            c.uuid == model.notify_uuid()
                && service_uuid.map_or(true, |service| c.service_uuid == service)
        })
        .cloned()
        .ok_or("Device notification characteristic was not found")?;
    peripheral
        .subscribe(&notify)
        .await
        .map_err(|e| format!("Could not subscribe to device notifications: {e}"))?;
    let mut notifications = peripheral
        .notifications()
        .await
        .map_err(|e| e.to_string())?;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tauri::async_runtime::spawn(async move {
        while let Some(n) = notifications.next().await {
            if tx.send(n.value).is_err() {
                break;
            }
        }
    });
    let edifier_protocol = if model == Model::EdifierW820NbPlus {
        let properties = peripheral.properties().await.ok().flatten();
        EdifierBLEProtocol::from_manufacturer_data(properties.as_ref().map(|p| &p.manufacturer_data))
    } else {
        EdifierBLEProtocol::default()
    };
    let mut session = AccessorySession {
        model,
        peripheral,
        write,
        rx,
        decoder: if model == Model::EdifierW820NbPlus {
            Decoder::Edifier(Vec::new(), edifier_protocol)
        } else if model == Model::BaseusBlade2 {
            Decoder::BaseusPower(Vec::new())
        } else {
            Decoder::Baseus(Vec::new())
        },
        edifier_protocol,
        battery: None,
        noise: None,
        gestures: Vec::new(),
        eq_presets: Vec::new(),
        selected_eq: None,
        sleep_timer_options: Vec::new(),
        sleep_timer_minutes: None,
        game_mode_supported: false,
        game_mode: None,
        prompt_tone_supported: false,
        prompt_tone_enabled: None,
        prompt_volume_supported: false,
        prompt_volume_level: None,
        auto_power_off_supported: false,
        auto_power_off: None,
        disconnect_audio_supported: false,
        repair_bluetooth_supported: false,
        power_off_supported: false,
        bluetooth_name_supported: false,
        bluetooth_name_limit_bytes: 0,
        bluetooth_name: None,
        touch_sensitivity_supported: false,
        touch_sensitivity_index: None,
        touch_sensitivity_value: None,
        touch_controls_supported: false,
        touch_controls: Vec::new(),
        power: HashMap::new(),
        device_name: name,
    };
    session.refresh().await?;
    let result = session.snapshot("connected");
    *active = Some(session);
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_refresh(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
) -> Result<Value, String> {
    let mut spp = hub.spp_session.lock().await;
    if let Some(session) = spp.as_mut() {
        session.refresh()?;
        let result = session.snapshot("connected");
        emit(&app, result.clone());
        return Ok(result);
    }
    drop(spp);
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.refresh().await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_noise(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    mode: String,
) -> Result<Value, String> {
    let mut spp = hub.spp_session.lock().await;
    if let Some(session) = spp.as_mut() {
        session.set_noise(&mode)?;
        let result = session.snapshot("connected");
        emit(&app, result.clone());
        return Ok(result);
    }
    drop(spp);
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_noise(&mode).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_gesture(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    button: u8,
    layout: u8,
    action: u8,
) -> Result<Value, String> {
    let mut spp = hub.spp_session.lock().await;
    if let Some(session) = spp.as_mut() {
        session.set_gesture(button, layout, action)?;
        let result = session.snapshot("connected");
        emit(&app, result.clone());
        return Ok(result);
    }
    drop(spp);
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No Baseus BLE headset is connected")?;
    session.set_gesture(button, layout, action).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_gesture_children(app: AppHandle, hub: State<'_, AccessoryHub>, button: u8, layout: u8, selected: Vec<u8>) -> Result<Value, String> {
    let mut spp = hub.spp_session.lock().await;
    let session = spp.as_mut().ok_or("No Baseus SPP headset is connected")?;
    session.set_gesture_children(button, layout, &selected)?;
    let result = session.snapshot("connected"); emit(&app, result.clone()); Ok(result)
}

#[tauri::command]
pub async fn accessory_set_equalizer(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    id: u8,
) -> Result<Value, String> {
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_equalizer(id).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_sleep_timer(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    minutes: u16,
) -> Result<Value, String> {
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_sleep_timer(minutes).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_game_mode(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    enabled: bool,
) -> Result<Value, String> {
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_game_mode(enabled).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_auto_power_off(app: AppHandle, hub: State<'_, AccessoryHub>, enabled: bool) -> Result<Value, String> {
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_auto_power_off(enabled).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_edifier_action(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    action: String,
) -> Result<Value, String> {
    if hub.spp_session.lock().await.is_some() {
        return Err("Connected device uses SPP, not the EDIFIER BLE profile".into());
    }
    let active = hub.session.lock().await;
    let session = active.as_ref().ok_or("No BLE device is connected")?;
    let command = session.edifier_bluetooth_action(&action).await?;
    let mut result = session.snapshot("connected");
    result["message"] = json!(format!("EDIFIER command 0x{command:02X} sent; the device provides no state readback"));
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_prompt_tone(app: AppHandle, hub: State<'_, AccessoryHub>, enabled: bool) -> Result<Value, String> {
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_prompt_tone(enabled).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_prompt_volume(app: AppHandle, hub: State<'_, AccessoryHub>, level: u8) -> Result<Value, String> {
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_prompt_volume(level).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_bluetooth_name(app: AppHandle, hub: State<'_, AccessoryHub>, name: String) -> Result<Value, String> {
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_bluetooth_name(&name).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_touch_sensitivity(app: AppHandle, hub: State<'_, AccessoryHub>, value: u8) -> Result<Value, String> {
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_touch_sensitivity(value).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_touch_control(app: AppHandle, hub: State<'_, AccessoryHub>, index: u8, action: u8, modes: Vec<u8>) -> Result<Value, String> {
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_touch_control(index, action, &modes).await?;
    let result = session.snapshot("connected"); emit(&app, result.clone()); Ok(result)
}

#[tauri::command]
pub async fn accessory_set_power(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    code: u16,
    value: u16,
) -> Result<Value, String> {
    if hub.spp_session.lock().await.is_some() {
        return Err("Connected device is a headset, not a Blade 2 powerbank".into());
    }
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    if session.model != Model::BaseusBlade2 {
        return Err("Connected device is not a Blade 2 powerbank".into());
    }
    session.set_power(code, value).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_reset_power(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
) -> Result<Value, String> {
    if hub.spp_session.lock().await.is_some() {
        return Err("Connected device is a headset, not a Blade 2 powerbank".into());
    }
    let active = hub.session.lock().await;
    let session = active.as_ref().ok_or("No BLE device is connected")?;
    session.restore_power_defaults().await?;
    let mut result = session.snapshot("connected");
    result["message"] = json!("Factory-reset command sent; device acknowledgement is unavailable");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_port_mode(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    port: u8,
    mode: u8,
) -> Result<Value, String> {
    if hub.spp_session.lock().await.is_some() {
        return Err("Connected device is a headset, not a Blade 2 powerbank".into());
    }
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_power_port_mode(port, mode).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_power_text(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    text: String,
) -> Result<Value, String> {
    if hub.spp_session.lock().await.is_some() {
        return Err("Connected device is a headset, not a Blade 2 powerbank".into());
    }
    let active = hub.session.lock().await;
    let session = active.as_ref().ok_or("No BLE device is connected")?;
    if session.model != Model::BaseusBlade2 {
        return Err("Connected device is not a Blade 2 powerbank".into());
    }
    session.set_power_text(&text).await?;
    let mut result = session.snapshot("connected");
    result["message"] = json!("Display text sent; the protocol provides no text readback here");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_disconnect(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
) -> Result<(), String> {
    hub.spp_session.lock().await.take();
    if let Some(session) = hub.session.lock().await.take() {
        session
            .peripheral
            .disconnect()
            .await
            .map_err(|e| e.to_string())?;
    }
    emit(&app, json!({ "status": "disconnected" }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edifier_d8_profiles_only_enable_known_preset_layouts() {
        assert_eq!(edifier_eq_presets(7).iter().map(|p| p.name).collect::<Vec<_>>(), ["Classic", "Dynamic", "Custom"]);
        assert_eq!(edifier_eq_presets(10).last().map(|p| p.id), Some(4));
        assert!(edifier_eq_presets(5).is_empty());
        assert!(edifier_eq_presets(11).is_empty());
    }

    #[test]
    fn edifier_voice_prompt_level_uses_the_reported_b17_bit_and_range_zero_to_twelve() {
        let mut capabilities = vec![0; 18];
        assert!(!edifier_prompt_volume_supported(&capabilities));
        capabilities[17] = 0x10;
        assert!(edifier_prompt_volume_supported(&capabilities));
        assert_eq!(decode_edifier_prompt_level(&[0]), Some(0));
        assert_eq!(decode_edifier_prompt_level(&[13]), Some(13));
        assert_eq!(decode_edifier_prompt_level(&[15]), Some(15));
        assert_eq!(decode_edifier_prompt_level(&[16]), None);
        assert_eq!(decode_edifier_prompt_level(&[]), None);
    }

    #[test]
    fn baseus_spp_profiles_match_paired_model_names() {
        assert!(Model::parse("baseus-ep10-pro")
            .unwrap()
            .matches_spp_name("Baseus Bass EP10 Pro"));
        assert!(Model::parse("baseus-ep10-pro")
            .unwrap()
            .matches_spp_name("EP10 Pro"));
        assert!(Model::parse("baseus-bowie-mf1")
            .unwrap()
            .matches_spp_name("Baseus Bowie MF1"));
        assert!(!Model::parse("baseus-bowie-mf1")
            .unwrap()
            .matches_spp_name("Baseus Bowie 30"));
        assert_eq!(
            encode_baseus_spp_request(0x34, &[1, 101]),
            [0xBA, 0x34, 1, 101]
        );
    }

    #[test]
    fn edifier_decoder_handles_concatenated_valid_frames() {
        let a = encode_edifier_header(0xBB, &[0xD0]);
        let b = encode_edifier_header(0xCC, &[0xCC, 2]);
        let input = [a, b].concat();
        let frames = decode_edifier_stream(&mut Vec::new(), &input, EdifierBLEProtocol::default());
        assert_eq!(
            frames.iter().map(|f| f.command).collect::<Vec<_>>(),
            [0xD0, 0xCC]
        );
    }
    #[test]
    fn baseus_decoder_extracts_battery_and_noise_frames() {
        let frames = decode_baseus_stream(
            &mut Vec::new(),
            &[0xAA, 0x02, 80, 0, 75, 1, 0xAA, 0x33, 1, 8, 0, 0],
        );
        assert_eq!(
            frames.iter().map(|f| f.command).collect::<Vec<_>>(),
            [0x02, 0x33]
        );
        assert_eq!(frames[0].payload, [80, 0, 75, 1]);
    }

    #[test]
    fn baseus_decoder_extracts_legacy_mf1_gesture_frames() {
        let mut buffer = Vec::new();
        let frames = decode_baseus_stream(&mut buffer, &[0xAA, 0x21, 0, 1, 2, 0xAA, 0x22, 1]);
        assert_eq!(frames.iter().map(|frame| frame.command).collect::<Vec<_>>(), [0x21, 0x22]);
        assert_eq!(frames[0].payload, [0, 1, 2]);
        assert_eq!(frames[1].payload, [1]);
    }

    #[test]
    fn blade2_power_codec_matches_crc_and_decodes_streamed_status() {
        assert_eq!(crc16(b"123456789"), 0x4B37);
        assert_eq!(
            encode_power_read(0x0020),
            [0x9A, 0xAA, 0x03, 0, 0x20, 0, 1, 0x09, 0xBD]
        );
        let frame = [0x9A, 0xAA, 0x03, 0, 0x20, 0, 1, 0, 2, 0x97, 0x30];
        let mut buffer = Vec::new();
        assert!(decode_power_stream(&mut buffer, &frame[..4]).is_empty());
        let decoded = decode_power_stream(&mut buffer, &frame[4..]);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].command, 0x0020);
        assert_eq!(decoded[0].payload, [1, 0, 2]);
    }
}
