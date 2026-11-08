# Changelog

## 0.1.0-alpha.4

- Android: add Baseus EP10 Pro SPP battery and Off/ANC/Transparency controls, with acknowledgement and readback; add Bowie MF1 left/right battery readings over SPP in the same DeviceDeck device hub.
- Android: add original vector illustrations for earbuds, over-ear headphones, and Blade 2 in the shared device screen; localize pairing guidance in English and Russian.
- Windows/Linux continues to ship in the same DeviceDeck release with the existing BLE accessory hub; macOS has an independent 0.7.0-alpha.5 release line.
- New Baseus SPP features have CI coverage but require checks against named physical models and firmware.

## 0.1.0-alpha.3

- Android: add EDIFIER W820NB Plus battery and noise-mode controls, and add Baseus Bowie 30 battery/noise controls to the shared DeviceDeck device hub.
- Android: add Baseus Blade 2 and Blade 2-140 power telemetry, cell measurements on 140, three display themes, screensaver, custom display text and reminder/shutdown timers.
- Windows/Linux: add EDIFIER W820NB Plus and Baseus Bowie 30 BLE battery/noise controls to the existing DeviceDeck app.
- Keep hardware validation status explicit: these new BLE paths have CI coverage but still need round-trip checks on named devices/firmware. Baseus Blade 2 port toggles and reset remain unavailable.
- macOS release remains on its independent 0.7.0-alpha.4 version line.

## 0.1.0-alpha.2

- Windows/Linux desktop known-issues page now warns that the realme Buds Air 7 Pro spatial-audio toggle may not produce a spatial effect, based on the documented device report.
- Added a regression check for the Air 7 Pro warning.
- Android and desktop artifacts are rebuilt with the same version; hardware-specific controls are incomplete and unverified.

## 0.1.0-alpha.1

- First DeviceDeck Android and Windows/Linux prerelease based on the QuickBuds protocol foundation.
- Included inherited realme model profiles; support on DeviceDeck builds remains unverified.
