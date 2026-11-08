# Changelog

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
