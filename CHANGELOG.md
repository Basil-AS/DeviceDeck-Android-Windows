# Changelog

## 0.1.0-alpha.17 — 2026-11-09

- Add Baseus Bowie MF1 legacy gesture assignment controls to the shared Android and Windows/Linux DeviceDeck hub. Query layouts with BA21, write paired left/right actions with BA22, require the vendor success acknowledgement, and confirm the selected layout by reading it back. Hardware verification remains pending.
- Align the Windows/Linux package version with the shared alpha.17 release.

## 0.1.0-alpha.16 — 2026-11-09

- Add the D8 B17 bit 1 gated EDIFIER W820NB Plus prompt tone switch to the shared Android and Windows/Linux DeviceDeck hub. F9 reads state and FA writes a one-byte switch, with F9 readback after each write. Static APK evidence supports this mapping; physical-device verification remains open.

## 0.1.0-alpha.15 — 2026-11-09

- Android and Windows/Linux: add capability-gated EDIFIER W820NB Plus left/right tap and long-press assignment controls from D8 profile 18. Map play/pause, side-specific volume/track, voice assistant, game/sound mode, and 2–3 selected noise-cycle modes with F0 readback after F1 writes. Static APK mappings are implemented; physical headset verification remains open.

## 0.1.0-alpha.14 — 2026-11-09

- Android and Windows/Linux: add Edifier W820NB Plus touch-sensitivity read/write controls when D8 byte 17 bit 3 advertises support. Read FD before showing the control; accept only indexed profiles 1 (0–31) and 2 (0–15), then confirm FE writes with FD readback. This remains unverified on physical hardware.
- Keep the control in the existing shared DeviceDeck hub alongside Edifier, realme and Baseus profiles.

## 0.1.0-alpha.13 — 2026-11-09

- Android and Windows/Linux: show the W820NB Plus Bluetooth name when D8 reports query support. Rename it only when D8 also reports a write profile; enforce its UTF-8 byte limit and confirm CA writes with a C9 readback.
- Keep the Edifier controls in the same DeviceDeck device hub as realme and Baseus. Physical-device behavior and reconnect persistence remain unverified.

## 0.1.0-alpha.12 — 2026-11-09

- Android and Windows/Linux: add Edifier W820NB Plus auto power-off when audio is absent. The D8 capability report gates the control; D7 reads state, D6 writes, and D7 verifies the result. Android asks before enabling.
- Keep Edifier and Baseus controls inside the same DeviceDeck app on each platform. Device behavior remains unverified on physical hardware.

## 0.1.0-alpha.11 — 2026-11-09

- Android and Windows/Linux: add Baseus EP10 Pro gesture assignments to DeviceDeck's shared device hub. The “Cycle noise modes” action can choose Normal, ANC, and Transparency (at least two), with BA8E readback after BA8F writes.
- Keep Android, Windows, and Linux in one DeviceDeck release line. Hardware round-trip validation remains pending.

## 0.1.0-alpha.10 — 2026-11-09

- Android and Windows/Linux: add Baseus Blade 2 USB-C C1/C2 mode controls for Off, Output/Discharge, and Input/Charge. Read the fresh `0x0010` port state, use the APK-confirmed `0x0018`–`0x001B` command pairs, then confirm the requested mode with another `0x0010` read. Hardware behavior remains unverified.

## 0.1.0-alpha.9 — 2026-11-09

- Android and Windows/Linux: expose W820NB Plus game mode only when D8 byte 17 bit 6 advertises it; query with 0x08 and confirm 0x09 writes with a fresh 0x08 readback.

## 0.1.0-alpha.8 — 2026-11-09

- Android and Windows/Linux: expose the Edifier W820NB Plus sleep timer only when D8 advertises a known timer profile; read the current value with D3 and confirm D1/D2 writes with D3 readback.
- The shared accessory hub keeps timer controls inside DeviceDeck. Hardware validation on W820NB Plus remains pending.

## 0.1.0-alpha.7 — 2026-11-09

- Android and Windows/Linux: add W820NB Plus EQ presets from the Edifier D8 profile report; read the selected preset with D5 and confirm C4 changes with a fresh D5 readback. Profiles with unsupported per-side or scene layouts stay hidden.
- Protocol changes have CI coverage. Physical-device validation is still required.

## 0.1.0-alpha.6 — 2026-11-09

- Windows/Linux: add Baseus EP10 Pro battery and Off/ANC/Transparency controls over paired RFCOMM/SPP in DeviceDeck's existing cross-brand hub; add battery-only Bowie MF1 SPP profile. Noise writes require acknowledgement and mode readback.
- Standard SPP connection targets Baseus's Bluetooth Serial Port service directly. MF1 exposes battery only until more command mappings are evidenced.
- New headset paths passed Android/Desktop CI. Physical-device validation remains pending.

## 0.1.0-alpha.5

- Windows/Linux: add Baseus Blade 2 as a power accessory in DeviceDeck's existing shared device hub, with telemetry, screen theme/saver, display text, reminder and shutdown controls.
- Desktop numeric settings require readback; custom display text is reported as sent because the inspected protocol does not provide a text readback.
- Hardware behavior still needs round-trip validation on Blade 2 and Blade 2-140.

## 0.1.0-alpha.4

- Android: add Baseus EP10 Pro SPP battery and Off/ANC/Transparency controls, with acknowledgement and readback; add Bowie MF1 left/right battery readings over SPP in the same DeviceDeck device hub.
- Android: add original vector illustrations for earbuds, over-ear headphones, and Blade 2 in the shared device screen; localize pairing guidance in English and Russian.
- Windows/Linux continues to ship in the same DeviceDeck release with the existing BLE accessory hub; macOS has an independent 0.7.0-alpha.6 release line.
- New Baseus SPP features have CI coverage but require checks against named physical models and firmware.

## 0.1.0-alpha.3

- Android: add EDIFIER W820NB Plus battery and noise-mode controls, and add Baseus Bowie 30 battery/noise controls to the shared DeviceDeck device hub.
- Android: add Baseus Blade 2 and Blade 2-140 power telemetry, cell measurements on 140, three display themes, screensaver, custom display text and reminder/shutdown timers.
- Windows/Linux: add EDIFIER W820NB Plus and Baseus Bowie 30 BLE battery/noise controls to the existing DeviceDeck app.
- Keep hardware validation status explicit: these new BLE paths have CI coverage but still need round-trip checks on named devices/firmware. Baseus Blade 2 reset remains unavailable.
- macOS release remains on its independent 0.7.0-alpha.4 version line.

## 0.1.0-alpha.2

- Windows/Linux desktop known-issues page now warns that the realme Buds Air 7 Pro spatial-audio toggle may not produce a spatial effect, based on the documented device report.
- Added a regression check for the Air 7 Pro warning.
- Android and desktop artifacts are rebuilt with the same version; hardware-specific controls are incomplete and unverified.

## 0.1.0-alpha.1

- First DeviceDeck Android and Windows/Linux prerelease based on the QuickBuds protocol foundation.
- Included inherited realme model profiles; support on DeviceDeck builds remains unverified.
