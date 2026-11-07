//! The report path for Feedback and Test my earbuds: the header, the packet log with the phone's scrub, the
//! preview text and the Worker post. The logic and the words are the ones of `desktop/src/report.rs`; the
//! Worker route is the same one. Nothing leaves the machine until Send.

use crate::session::{Line, LOG};
use crate::types::{cmd_of, models, payload_of, EVT_PUSH};
use eframe::egui;
use std::io::Write;
use std::process::Stdio;
use std::sync::mpsc::{channel, Receiver};


/// One report; [Draft::log] is None when the person turned it off.
pub struct Draft {
    pub model: String,
    pub categories: Vec<&'static str>,
    pub description: String,
    pub log: Option<String>,
    pub crash: bool,
}

/// The first line of a sent log: app, computer, system, buds model and firmware.
pub fn header(app: &crate::App) -> String {
    let s = &app.snap;
    let model = s.model.and_then(|m| m["name"].as_str()).unwrap_or(if s.name.is_empty() { "unknown buds" } else { s.name.as_str() });
    let id = s.model.and_then(|m| m["id"].as_str()).unwrap_or("?");
    format!("DeviceDeck {} | {} | {} | {model} ({id}) firmware {}", env!("CARGO_PKG_VERSION"), computer(), system(),
        s.firmware.as_deref().unwrap_or("?"))
}

#[cfg(not(windows))]
fn computer() -> String {
    let read = |f: &str| std::fs::read_to_string(format!("/sys/class/dmi/id/{f}")).map(|s| s.trim().to_string()).unwrap_or_default();
    format!("{} {}", read("sys_vendor"), read("product_name")).trim().to_string()
}

#[cfg(not(windows))]
fn system() -> String {
    let os = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let name = os.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")).unwrap_or("Linux").trim_matches('"').to_string();
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    format!("{name} (kernel {})", kernel.trim())
}

#[cfg(windows)]
fn computer() -> String {
    let out = std::process::Command::new("reg").args(["query", r"HKLM\HARDWARE\DESCRIPTION\System\BIOS"]).output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let value = |k: &str| text.lines().find(|l| l.trim_start().starts_with(k))
        .and_then(|l| l.split("REG_SZ").nth(1)).map(|v| v.trim().to_string()).unwrap_or_default();
    format!("{} {}", value("SystemManufacturer"), value("SystemProductName")).trim().to_string()
}

#[cfg(windows)]
fn system() -> String {
    let out = std::process::Command::new("cmd").args(["/c", "ver"]).output();
    out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_else(|_| "Windows".into())
}

/// This computer's own name, which the adapter and the buds' device list carry.
fn host() -> Option<String> {
    #[cfg(windows)]
    let name = std::env::var("COMPUTERNAME").ok();
    #[cfg(not(windows))]
    let name = std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string());
    name.filter(|n| n.len() >= 3)
}

/// Device list and device manager packets hold addresses and names: only the header stays. The `0x0204` push
/// carries them in sub `06` (device list) and sub `F2` (connected-devices info).
fn device_bytes(l: &Line) -> bool {
    if l.bytes.len() < 10 || l.bytes[0] != 0xAA { return false; }
    let pl = payload_of(&l.bytes);
    matches!(cmd_of(&l.bytes), 0x8112 | 0x0429 | 0x8132)
        || (cmd_of(&l.bytes) == EVT_PUSH && matches!(pl.first(), Some(&0x06) | Some(&0xF2)))
}

fn lines(log: &[Line]) -> String {
    log.iter().map(|l| if device_bytes(l) {
        format!("{} {} {} [device data removed]\n", l.at, l.dir, hex(&l.bytes[..l.bytes.len() - payload_of(&l.bytes).len()]))
    } else {
        crate::devlog::raw(l) + "\n"
    }).collect()
}

/// The log as sent: header, every line, without addresses, device data or this computer's name.
pub fn log_text(app: &crate::App) -> String {
    scrub(header(app) + "\n" + &lines(&LOG.lock().unwrap()))
}

fn scrub(text: String) -> String {
    let text = scrub_macs(&text);
    match host() { Some(h) => text.replace(&h, "[this computer]"), None => text }
}

/// `AA:BB:CC:DD:EE:FF` (or with `-`) -> `XX:XX:XX:XX:XX:XX`.
fn scrub_macs(s: &str) -> String {
    let b = s.as_bytes();
    let is_mac = |i: usize| i + 17 <= b.len()
        && (0..6).all(|k| b[i + 3 * k].is_ascii_hexdigit() && b[i + 3 * k + 1].is_ascii_hexdigit())
        && (0..5).all(|k| b[i + 3 * k + 2] == b':' || b[i + 3 * k + 2] == b'-')
        && (i == 0 || !b[i - 1].is_ascii_alphanumeric())
        && (i + 17 == b.len() || !b[i + 17].is_ascii_alphanumeric());
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if is_mac(i) { out += "XX:XX:XX:XX:XX:XX"; i += 17; continue; }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// What the person reads before Send: one block per field, the log last.
pub fn preview(d: &Draft) -> String {
    let mut blocks = vec![format!("Model:\n{}", d.model)];
    if !d.categories.is_empty() { blocks.push(format!("Category:\n{}", d.categories.join(", "))); }
    blocks.push(format!("Description:\n{}", d.description));
    if let Some(log) = &d.log { blocks.push(format!("Log:\n{log}")); }
    blocks.join("\n\n")
}

fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

/// Posts [body] to [url] through `curl`'s standard input. The answer code (0: no connection) and text.
fn post(url: &str, body: &str) -> (u16, String) {
    let child = crate::update_core::quiet("curl")
        .args(["-sS", "--max-time", "30", "-w", "\n%{http_code}", "-H", "Content-Type: application/json; charset=UTF-8", "--data-binary", "@-", url])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
    let Ok(mut child) = child else { return (0, String::new()) };
    if child.stdin.take().map(|mut i| i.write_all(body.as_bytes())).is_none() { return (0, String::new()); }
    let out = child.wait_with_output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let (text, code) = out.rsplit_once('\n').unwrap_or(("", out.as_str()));
    (code.trim().parse().unwrap_or(0), text.to_string())
}

/// A random version 4 UUID from two `RandomState` seeds: 128 random bits, with no extra crate.
fn new_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let r = |_: u8| std::collections::hash_map::RandomState::new().build_hasher().finish();
    let (a, b) = (r(0), r(1));
    format!("{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}", (a >> 32) as u32, (a >> 16) as u16, (a & 0xfff) as u16,
        0x8000 | (b >> 48) as u16 & 0x3fff, b & 0xffff_ffff_ffff)
}

/// The install ID: a random UUID made on the first start and kept in `settings.json` ("app_id").
pub fn app_id() -> String {
    if let Some(id) = crate::load_settings()["app_id"].as_str() { return id.to_string(); }
    let id = new_id();
    crate::save_setting("app_id", id.clone().into());
    id
}

/// Posts [d] to the Worker.
/// Returns (sent, the Worker's answer text when it refused the report).
pub fn send(_d: Draft) -> (bool, Option<String>) {
    (false, Some("Open a DeviceDeck GitHub issue to share feedback.".into()))
}

/// The answer of a post, ready for the frame that polls it.
pub struct Job(Receiver<(bool, Option<String>)>);

/// Posts [d] on a network thread and repaints when the answer arrives.
pub fn send_async(d: Draft, ctx: &egui::Context) -> Job {
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(send(d));
        ctx.request_repaint();
    });
    Job(rx)
}

impl Job {
    pub fn poll(&self) -> Option<(bool, Option<String>)> { self.0.try_recv().ok() }
}

/// A model name from the list, any case.
pub fn known(model: &str) -> bool {
    models().iter().any(|m| m["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(model)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::build_packet;

    #[test]
    fn macs() {
        assert_eq!(scrub_macs("to AA:bb:01:02:03:04 ok"), "to XX:XX:XX:XX:XX:XX ok");
        assert_eq!(scrub_macs("12:34:56.789 RX AA 07"), "12:34:56.789 RX AA 07");
    }

    #[test]
    fn export_hides_device_frames() {
        let l = Line {
            at: "12:00:00.000".into(), dir: "RX",
            bytes: vec![0xAA, 0x0B, 0, 0, 0x12, 0x81, 0, 0, 0x11, 0x22, 0x33, 0x44],
            human: None,
        };
        let out = lines(&[l]);
        assert!(out.contains("[device data removed]"), "{out}");
        assert!(!out.contains("11223344"), "{out}");
    }

    /// One RX line from `build_packet`, so the header is the one the log holds: `AA <len> 00 00 <cmd LE>
    /// <seq> <payload len LE>`, payload from byte 9 for a payload of 127 bytes or fewer.
    fn rx(cmd: u16, pl: &[u8]) -> Line {
        Line { at: "12:00:00.000".into(), dir: "RX".into(), bytes: build_packet(cmd, 0x11, pl), human: None }
    }

    /// The `0x0204` push `F2` (PROTOCOL.md section 10): `<count>`, then per device the MAC (6 bytes, reversed),
    /// connect count (u16 LE), name length, name.
    fn f2_payload() -> Vec<u8> {
        let mut pl = vec![0xF2, 1];
        pl.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        pl.extend_from_slice(&[0x02, 0x00]);
        pl.push(4);
        pl.extend_from_slice(b"Buds");
        pl
    }

    #[test]
    fn f2_connected_devices_lose_their_addresses_and_name() {
        let frame = build_packet(EVT_PUSH, 0x11, &f2_payload());
        let out = lines(&[rx(EVT_PUSH, &f2_payload())]);
        assert!(out.contains("[device data removed]"), "{out}");
        assert!(out.contains(&hex(&frame[..9])), "{out}");
        assert!(!out.contains(&hex(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66])), "{out}");
        assert!(!out.contains(&hex(b"Buds")), "{out}");
    }

    #[test]
    fn sub_six_device_list_still_loses_its_bytes() {
        let out = lines(&[rx(EVT_PUSH, &[0x06, 1, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66])]);
        assert!(out.contains("[device data removed]"), "{out}");
        assert!(!out.contains(&hex(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66])), "{out}");
    }

    #[test]
    fn another_push_sub_is_left_alone() {
        let l = rx(EVT_PUSH, &[0xF4, 0x7B, 0x7D]);
        let raw = crate::devlog::raw(&l) + "\n";
        assert_eq!(lines(&[l]), raw);
    }

    #[test]
    fn battery_push_is_left_alone() {
        let l = rx(EVT_PUSH, &[0x01, 0x02, 0x01, 0x64, 0x02, 0x64]);
        let raw = crate::devlog::raw(&l) + "\n";
        assert_eq!(lines(&[l]), raw);
    }

    #[test]
    fn install_id_is_a_v4_uuid() {
        let (a, b) = (new_id(), new_id());
        assert_ne!(a, b);
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), [8, 4, 4, 4, 12]);
        assert!(parts[2].starts_with('4'));
        assert!(matches!(parts[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'));
    }

    #[test]
    fn known_models() {
        assert!(known("OnePlus Buds 4"));
        assert!(known("oneplus buds 4"));
        assert!(!known(""));
    }
}
