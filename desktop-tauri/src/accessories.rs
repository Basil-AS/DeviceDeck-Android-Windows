//! Cross-brand BLE/GATT device hub for the existing DeviceDeck desktop app.
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
const EDIFIER_WRITE: &str = "48093803-1a48-11e9-ab14-d663bd873d93";
const EDIFIER_NOTIFY: &str = "48093802-1a48-11e9-ab14-d663bd873d93";
const BASEUS_WRITE: &str = "ee684b1a-1e9b-ed3e-ee55-f894667e92ac";
const BASEUS_NOTIFY: &str = "654b749c-e37f-ae1f-ebab-40ca133e3690";
const MAX_RX_BUFFER: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Model {
    EdifierW820NbPlus,
    BaseusBowie30,
    BaseusBlade2,
}

impl Model {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "edifier-w820nb-plus" => Ok(Self::EdifierW820NbPlus),
            "baseus-bowie-30" => Ok(Self::BaseusBowie30),
            "baseus-blade-2" => Ok(Self::BaseusBlade2),
            _ => Err("Unsupported DeviceDeck BLE profile".into()),
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::EdifierW820NbPlus => "edifier-w820nb-plus",
            Self::BaseusBowie30 => "baseus-bowie-30",
            Self::BaseusBlade2 => "baseus-blade-2",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::EdifierW820NbPlus => "EDIFIER W820NB Plus",
            Self::BaseusBowie30 => "Baseus Bowie 30",
            Self::BaseusBlade2 => "Baseus Blade 2",
        }
    }
    fn write_uuid(self) -> Uuid {
        Uuid::parse_str(match self {
            Self::EdifierW820NbPlus => EDIFIER_WRITE,
            Self::BaseusBowie30 | Self::BaseusBlade2 => BASEUS_WRITE,
        })
        .unwrap()
    }
    fn notify_uuid(self) -> Uuid {
        Uuid::parse_str(match self {
            Self::EdifierW820NbPlus => EDIFIER_NOTIFY,
            Self::BaseusBowie30 | Self::BaseusBlade2 => BASEUS_NOTIFY,
        })
        .unwrap()
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
        }
    }
}

pub struct AccessoryHub {
    found: Mutex<HashMap<String, (Peripheral, Model, String)>>,
    session: tokio::sync::Mutex<Option<AccessorySession>>,
}

impl AccessoryHub {
    fn new() -> Self {
        Self {
            found: Mutex::new(HashMap::new()),
            session: tokio::sync::Mutex::new(None),
        }
    }
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
    battery: Option<u8>,
    noise: Option<String>,
    power: HashMap<u16, PowerStatus>,
    device_name: String,
}

enum Decoder {
    Edifier(Vec<u8>),
    Baseus(Vec<u8>),
    BaseusPower(Vec<u8>),
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
    async fn write(&self, bytes: &[u8]) -> Result<(), String> {
        self.peripheral
            .write(&self.write, bytes, WriteType::WithResponse)
            .await
            .map_err(|e| e.to_string())
    }
    fn feed(&mut self, bytes: &[u8]) -> Vec<Frame> {
        match (&mut self.decoder, self.model) {
            (Decoder::Edifier(buffer), Model::EdifierW820NbPlus) => {
                decode_edifier_stream(buffer, bytes)
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
                // D8 is a capability probe; the current desktop profile does not expose
                // optional commands until its W820-specific flags have been validated.
                let d8 = [0xD8];
                let _ = self.request(&encode_edifier(&d8), 0xD8).await;
                let d0 = [0xD0];
                let battery = self.request(&encode_edifier(&d0), 0xD0).await?;
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
                let noise = self.request(&encode_edifier(&cc), 0xCC).await?;
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
        }
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
                let packet = encode_edifier(&[0xC1, wire]);
                let ack = self.request(&packet, 0xC1).await?;
                if ack.header != 0xCC {
                    return Err("EDIFIER did not acknowledge the mode".into());
                }
                let cc = [0xCC];
                let state = self.request(&encode_edifier(&cc), 0xCC).await?;
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
        }
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
        json!({ "status": status, "model": self.model.id(), "name": self.device_name, "battery": self.battery, "noise": self.noise, "power": power })
    }
}

fn encode_edifier(payload: &[u8]) -> Vec<u8> {
    encode_edifier_header(0xAA, payload)
}

fn encode_edifier_header(header: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![header, payload.len() as u8];
    out.extend_from_slice(payload);
    let sum = out.iter().fold(0x2019u16, |s, b| s.wrapping_add(*b as u16));
    out.extend_from_slice(&sum.to_be_bytes());
    out
}

fn decode_edifier_stream(buffer: &mut Vec<u8>, input: &[u8]) -> Vec<Frame> {
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
        let size = buffer[1] as usize + 4;
        if buffer.len() < size {
            break;
        }
        let raw = &buffer[..size];
        let sum = raw[..size - 2]
            .iter()
            .fold(0x2019u16, |s, b| s.wrapping_add(*b as u16));
        if u16::from_be_bytes([raw[size - 2], raw[size - 1]]) == sum && raw[1] >= 1 {
            frames.push(Frame {
                header: raw[0],
                command: raw[2] as u16,
                payload: raw[3..size - 2].to_vec(),
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
            0x02 | 0x33 => 6,
            0x34 | 0x35 => 3,
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
    emit(
        &app,
        json!({ "status": "scanning", "model": model.id(), "devices": [] }),
    );
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
    let write = chars
        .iter()
        .find(|c| c.uuid == model.write_uuid())
        .cloned()
        .ok_or("Device control write characteristic was not found")?;
    let notify = chars
        .iter()
        .find(|c| c.uuid == model.notify_uuid())
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
    let mut session = AccessorySession {
        model,
        peripheral,
        write,
        rx,
        decoder: if model == Model::EdifierW820NbPlus {
            Decoder::Edifier(Vec::new())
        } else if model == Model::BaseusBlade2 {
            Decoder::BaseusPower(Vec::new())
        } else {
            Decoder::Baseus(Vec::new())
        },
        battery: None,
        noise: None,
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
    let mut active = hub.session.lock().await;
    let session = active.as_mut().ok_or("No BLE device is connected")?;
    session.set_noise(&mode).await?;
    let result = session.snapshot("connected");
    emit(&app, result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn accessory_set_power(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    code: u16,
    value: u16,
) -> Result<Value, String> {
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
pub async fn accessory_set_power_text(
    app: AppHandle,
    hub: State<'_, AccessoryHub>,
    text: String,
) -> Result<Value, String> {
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
    fn edifier_decoder_handles_concatenated_valid_frames() {
        let a = encode_edifier_header(0xBB, &[0xD0]);
        let b = encode_edifier_header(0xCC, &[0xCC, 2]);
        let input = [a, b].concat();
        let frames = decode_edifier_stream(&mut Vec::new(), &input);
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
