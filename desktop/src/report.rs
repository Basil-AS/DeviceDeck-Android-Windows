//! App settings › Feedback: the phone's `ProblemReport`, posted with `curl` (as the updater does) to the same
//! report Worker. Nothing goes out without Send; See what is sent shows the exact text first.

use crate::types::*;
use crate::session::{Line, LOG};
use crate::{with_app, App, Community, MainWindow, Report, Tr};
use slint::{ComponentHandle, Model, VecModel};
use std::io::Write;
use std::process::Stdio;
use std::sync::Mutex;

/// The Category options, in the Worker's own words; it drops any other value (report-worker/src/index.js).
const CATEGORIES: [&str; 6] = ["UI", "Lag", "Connection", "Feature not working", "Battery", "Other"];
/// Their labels, the phone's `problem_cat_*` strings.
pub const CATEGORY_KEYS: [&str; 6] = ["problem_cat_ui", "problem_cat_lag", "problem_cat_connection",
    "problem_cat_feature", "problem_cat_battery", "problem_cat_other"];

/// The first line of a sent or exported log: app, computer, system, buds model and firmware.
pub fn header(a: &App) -> String {
    let s = &a.snap;
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

/// Maker and model from the BIOS keys (`reg` ships with Windows).
#[cfg(windows)]
fn computer() -> String {
    let out = crate::update::quiet("reg").args(["query", r"HKLM\HARDWARE\DESCRIPTION\System\BIOS"]).output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let value = |k: &str| text.lines().find(|l| l.trim_start().starts_with(k))
        .and_then(|l| l.split("REG_SZ").nth(1)).map(|v| v.trim().to_string()).unwrap_or_default();
    format!("{} {}", value("SystemManufacturer"), value("SystemProductName")).trim().to_string()
}

#[cfg(windows)]
fn system() -> String {
    let out = crate::update::quiet("cmd").args(["/c", "ver"]).output();
    out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_else(|_| "Windows".into())
}

/// This computer's own name, which the Bluetooth adapter and the buds' device list carry.
fn host() -> Option<String> {
    #[cfg(windows)]
    let name = std::env::var("COMPUTERNAME").ok();
    #[cfg(not(windows))]
    let name = std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string());
    name.filter(|n| n.len() >= 3)
}

/// Device list and device manager packets (PROTOCOL.md section 19) hold addresses and names: only the header stays.
/// The `0x0204` push carries them in sub `06` (device list) and sub `F2` (connected-devices info).
fn device_bytes(l: &Line) -> bool {
    if l.bytes.len() < 10 || l.bytes[0] != 0xAA { return false; }
    let pl = payload_of(&l.bytes);
    matches!(cmd_of(&l.bytes), 0x8112 | 0x0429 | 0x8132)
        || (cmd_of(&l.bytes) == EVT_PUSH && matches!(pl.first(), Some(&0x06) | Some(&0xF2)))
}

/// The log as sent: header, every line, the newest stderr, then the last crash report; without Bluetooth addresses,
/// device data or this computer's name.
pub(crate) fn log_text(a: &App) -> String {
    scrub(header(a) + "\n" + &lines(&LOG.lock().unwrap()) + &stderr_tail() + CRASH.lock().unwrap().as_str())
}

/// The log as Dev tools › Export saves it: header and packet lines, scrubbed like a sent report. The in-memory
/// log stays raw.
pub(crate) fn export_text(a: &App) -> String {
    scrub(header(a) + "\n" + &lines(&LOG.lock().unwrap()))
}

/// `stderr.log` next to `settings.json`.
fn stderr_path() -> Option<std::path::PathBuf> {
    crate::settings_path().map(|p| p.with_file_name("stderr.log"))
}

/// Second thing in `main`: stderr goes to [stderr_path], emptied per launch. GTK, Slint, BlueZ and Rust warnings land
/// there; Windows shows no console and a launcher drops them. From a terminal stderr stays on the terminal.
pub fn capture_stderr() {
    use std::io::IsTerminal;
    if std::io::stderr().is_terminal() { return; }
    let Some(path) = stderr_path() else { return };
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let Ok(file) = std::fs::File::create(&path) else { return };
    #[cfg(not(windows))]
    unsafe {
        use std::os::fd::IntoRawFd;
        libc::dup2(file.into_raw_fd(), 2);
    }
    #[cfg(windows)]
    unsafe {
        use std::os::windows::io::IntoRawHandle;
        windows_sys::Win32::System::Console::SetStdHandle(windows_sys::Win32::System::Console::STD_ERROR_HANDLE,
            file.into_raw_handle() as _);
    }
}

/// The last 100 stderr lines, or "" when there are none. The tray library's deprecation notice comes every launch.
fn stderr_tail() -> String {
    let text = stderr_path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty() && !l.contains("libayatana-appindicator is deprecated")).collect();
    if lines.is_empty() { return String::new(); }
    format!("\nSTDERR (warnings and errors):\n{}\n", lines[lines.len().saturating_sub(100)..].join("\n"))
}

fn lines(log: &[Line]) -> String {
    log.iter().map(|l| if device_bytes(l) {
        format!("{} {} {} [device data removed]\n", l.at, l.dir, hex(&l.bytes[..l.bytes.len() - payload_of(&l.bytes).len()]))
    } else {
        crate::devlog::raw(l) + "\n"
    }).collect()
}

fn scrub(text: String) -> String {
    let text = scrub_macs(&text);
    match host() { Some(h) => text.replace(&h, "[this computer]"), None => text }
}

/// Downloads/DeviceDeck: exported logs and crash reports, as on the phone.
pub fn folder() -> std::path::PathBuf {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    std::path::PathBuf::from(home.unwrap_or_default()).join("Downloads").join("DeviceDeck")
}

/// The crash report the Report page sends after a crash; "" otherwise.
static CRASH: Mutex<String> = Mutex::new(String::new());

/// The crash report opened the Report page this start: no other launch prompt shows (`modeltest.rs`).
static CRASH_SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn crash_shown() -> bool { CRASH_SHOWN.load(std::sync::atomic::Ordering::Relaxed) }

/// First thing in `main`: a panic writes `crash-<seconds>.txt` to [folder], with the newest log lines. The release
/// build aborts on a panic and Windows shows no console, so this file is the only record.
pub fn catch_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // A panic while the log is locked skips the log rather than deadlock.
        let log = LOG.try_lock().map(|l| lines(&l[l.len().saturating_sub(300)..])).unwrap_or_default();
        let text = format!("DeviceDeck crash report\nversion: {}\ncomputer: {}\nsystem: {}\nthread: {}\n\n{info}\n\n{log}{}",
            env!("CARGO_PKG_VERSION"), computer(), system(), std::thread::current().name().unwrap_or("?"), stderr_tail());
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let _ = std::fs::create_dir_all(folder()).and_then(|_| std::fs::write(folder().join(format!("crash-{secs}.txt")), scrub(text)));
        previous(info);
    }));
}

/// A crash report newer than the last one shown opens the Report page, filled in with it. Once per report.
pub fn show_crash(a: &mut App) {
    let newest = std::fs::read_dir(folder()).into_iter().flatten().flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("crash-") && n.ends_with(".txt"))
        .max_by_key(|n| n[6..n.len() - 4].parse::<u64>().unwrap_or(0));
    let Some(name) = newest else { return };
    if crate::load_settings()["crash_shown"].as_str() == Some(name.as_str()) { return; }
    crate::save_setting("crash_shown", name.clone().into());
    let Ok(text) = std::fs::read_to_string(folder().join(&name)) else { return };
    *CRASH.lock().unwrap() = format!("\n{text}");
    CRASH_SHOWN.store(true, std::sync::atomic::Ordering::Relaxed);
    open(a);
    let r = a.main.global::<Report>();
    r.get_picked().set_row_data(5, true);
    r.set_description("App crash".into());
    r.set_note(crate::t(&a.tr, "crash_title").into());
    a.main.set_page(9);
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

/// One report; [Draft::log] is None when the person turned it off.
pub(crate) struct Draft {
    pub model: String,
    pub categories: Vec<&'static str>,
    pub description: String,
    pub log: Option<String>,
    pub crash: bool,
}

/// What the person reads before Send: one block per field, the log last.
pub(crate) fn preview(d: &Draft) -> String {
    let mut blocks = vec![format!("Model:\n{}", d.model)];
    if !d.categories.is_empty() { blocks.push(format!("Category:\n{}", d.categories.join(", "))); }
    blocks.push(format!("Description:\n{}", d.description));
    if let Some(log) = &d.log { blocks.push(format!("Log:\n{log}")); }
    blocks.join("\n\n")
}

/// Posts [body] to [url] through `curl`'s standard input. The answer code (0: no connection) and text.
fn post(url: &str, body: &str) -> (u16, String) {
    let child = crate::update::quiet("curl")
        .args(["-sS", "--max-time", "30", "-w", "\n%{http_code}", "-H", "Content-Type: application/json; charset=UTF-8", "--data-binary", "@-", url])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
    let Ok(mut child) = child else { return (0, String::new()) };
    if child.stdin.take().map(|mut i| i.write_all(body.as_bytes())).is_none() { return (0, String::new()); }
    let out = child.wait_with_output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let (text, code) = out.rsplit_once('\n').unwrap_or(("", out.as_str()));
    (code.trim().parse().unwrap_or(0), text.to_string())
}

/// A random version 4 UUID. `RandomState` is seeded by the system's random source: two of them give 128 random bits, with no extra crate.
fn new_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let r = |_: u8| std::collections::hash_map::RandomState::new().build_hasher().finish();
    let (a, b) = (r(0), r(1));
    format!("{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}", (a >> 32) as u32, (a >> 16) as u16, (a & 0xfff) as u16,
        0x8000 | (b >> 48) as u16 & 0x3fff, b & 0xffff_ffff_ffff)
}

/// The install ID: a random UUID made on the first start and kept in `settings.json` ("app_id"). It ties to no device,
/// account or address. Sent with the daily ping and with a problem report. Delete the key to get a new one.
pub(crate) fn app_id() -> String {
    if let Some(id) = crate::load_settings()["app_id"].as_str() { return id.to_string(); }
    let id = new_id();
    crate::save_setting("app_id", id.clone().into());
    id
}

/// Counts this install once a day: platform, version and the install ID (README, Privacy). Network thread only.
pub(crate) fn ping(_weak: slint::Weak<MainWindow>) {}

const DISCORD: &str = "https://discord.gg/pvsnEf9TrZ";

/// The high-quality codec tokens, best first (`ModelCatalog.bestHiResCodec`).
const HIRES_CODECS: [&str; 6] = ["lhdc5", "lhdc", "ldac", "aptx-adaptive", "aptx-hd", "aptx"];

/// The issues of the current earbuds ([known issues](crate) as on the phone): the model's `knownIssues`, Hi-Res with 3D audio
/// when both are offered and are not one switch each, and Hi-Res on a model whose `codecs` names no high-quality codec.
fn known_issues(a: &App) -> (bool, bool, bool) {
    let listed = |key: &str| a.snap.model.is_some_and(|m| m["knownIssues"].as_array().is_some_and(|k| k.iter().any(|x| x.as_str() == Some(key))));
    let silent = listed("spatial_silent");
    let offered = |id: u8| a.snap.features.iter().any(|f| f.0 == id);
    let both = offered(FEATURE_HIRES) && offered(FEATURE_SPATIAL) && a.snap.model.map_or(true, |m| m["spatialToggle"] != 1);
    let hires_row = offered(FEATURE_HIRES) || crate::earbuds::codec_picker(&a.snap);
    let no_codec = hires_row && a.snap.model.is_some_and(|m| m["codecs"].as_array().is_some_and(|c| {
        !c.iter().any(|x| x.as_str().is_some_and(|t| HIRES_CODECS.contains(&t)))
    }));
    (silent, both, listed("hires_no_codec") || no_codec)
}

/// Posts [d] to the Worker.
/// Returns (sent, the Worker's answer text when it refused the report).
pub(crate) fn send(_d: Draft) -> (bool, Option<String>) {
    (false, Some("Open a DeviceDeck GitHub issue to share feedback.".into()))
}

/// The current form; None (with a note) without a description, when the model is not in the list (a crash
/// report sends with any model), or when the log has no RX frame (the buds never connected). A person who cannot
/// connect at all has no log, only words to reproduce it from: Send without the log still works.
fn current(a: &App) -> Option<Draft> {
    let r = a.main.global::<Report>();
    let tr = a.main.global::<Tr>();
    let picked: Vec<&str> = (0..6).filter(|&i| r.get_picked().row_data(i).unwrap_or(false)).map(|i| CATEGORIES[i]).collect();
    let description = r.get_description().trim().to_string();
    let crash = !CRASH.lock().unwrap().is_empty();
    if !crash && !known(r.get_model().trim()) {
        r.set_note(format!("{}\n{}: {}#supported-earbuds", tr.get_problem_unsupported(), tr.get_problem_supported_list(),
            crate::update::PAGE.trim_end_matches("/releases/latest")).into());
        return None;
    }
    if description.is_empty() {
        r.set_note("Describe the problem: what you did and what happened.".into());
        return None;
    }
    let no_frames = !crash && !LOG.lock().unwrap().iter().any(|l| l.dir == "RX");
    if r.get_with_log() && no_frames {
        r.set_note(tr.get_problem_log_empty());
        return None;
    }
    r.set_note("".into());
    let log = r.get_with_log().then(|| log_text(a));
    Some(Draft { model: r.get_model().trim().to_string(), categories: picked, description, log, crash })
}

fn open(a: &mut App) {
    let r = a.main.global::<Report>();
    if r.get_model().is_empty() {
        r.set_model(a.snap.model.and_then(|m| m["name"].as_str()).unwrap_or(a.snap.name.as_str()).into());
    }
    r.set_note("".into());
    r.set_preview("".into());
}

pub fn setup(main: &MainWindow) {
    let r = main.global::<Report>();
    r.set_picked(VecModel::from_slice(&[false; 6]));
    r.on_open(|| with_app(open));
    let c = main.global::<Community>();
    c.on_refresh(|| with_app(|a| {
        let (silent, both, no_codec) = known_issues(a);
        let c = a.main.global::<Community>();
        c.set_spatial_silent(silent);
        c.set_hires_spatial(both);
        c.set_hires_no_codec(no_codec);
    }));
    c.on_open_discord(|| crate::update::open_url(DISCORD));
    r.on_toggle(|i| with_app(|a| {
        let p = a.main.global::<Report>().get_picked();
        let i = i as usize;
        p.set_row_data(i, !p.row_data(i).unwrap_or(false));
    }));
    r.on_show_preview(|| with_app(|a| {
        if let Some(d) = current(a) {
            a.main.global::<Report>().set_preview(preview(&d).into());
        }
    }));
    r.on_send(|| with_app(|a| {
        let Some(d) = current(a) else { return };
        let r = a.main.global::<Report>();
        r.set_busy(true);
        let weak = a.main.as_weak();
        std::thread::spawn(move || {
            let (ok, refused) = send(d);
            let _ = weak.upgrade_in_event_loop(move |m| {
                let r = m.global::<Report>();
                r.set_busy(false);
                if ok {
                    r.set_note("Feedback sent. Thank you!".into());
                    r.set_description("".into());
                    r.set_preview("".into());
                    r.set_picked(VecModel::from_slice(&[false; 6]));
                    CRASH.lock().unwrap().clear();
                } else {
                    r.set_note(refused.unwrap_or_else(|| "Could not send the feedback. Check the internet connection and try again.".into()).into());
                }
            });
        });
    }));
}

/// A model name from the list, any case.
fn known(model: &str) -> bool {
    models().iter().any(|m| m["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(model)))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!out.contains("11 22 33 44"), "{out}");
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
        let l = rx(EVT_PUSH, &f2_payload());
        let out = lines(&[l]);
        assert!(out.contains("[device data removed]"), "{out}");
        assert!(out.contains(&hex(&build_packet(EVT_PUSH, 0x11, &f2_payload())[..9])), "{out}");
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
        assert!(a.bytes().all(|c| c == b'-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn known_models() {
        assert!(known("OnePlus Buds 4"));
        assert!(known("oneplus buds 4"));
        assert!(!known("Xiaomi Buds 5"));
        assert!(!known(""));
    }
}
