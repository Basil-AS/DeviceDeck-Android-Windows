# DeviceDeck Android and desktop architecture

This repository builds the Android, Windows and Linux versions of the single DeviceDeck app. It inherits the OnePlus/OPPO/realme implementation from QuickBuds and adds Edifier, Baseus, and other device profiles to the same app and release channels. Protocol modules and transports are separated internally by vendor/model; they are not separate user-facing apps. A catalog row alone does not establish working device support. See the umbrella repository's support matrix and protocol research.

## Platform layers

| Layer | Location | Role |
|---|---|---|
| Android app | `app/` | Kotlin Bluetooth link, protocol implementation, device catalog, UI, widgets, tiles and notifications. |
| Sans-I/O core | `core/` | Rust frames, typed messages, commands and feature-sync logic. |
| Desktop app | `desktop-tauri/`, `desktop/` | Tauri UI and native Windows/Linux Bluetooth/control code. |
| Alternate UI | `desktop-egui/` | Experimental UI port; not the release binary. |

The Android protocol implementation and Rust core are separate implementations. Protocol changes that affect both platforms need corresponding changes in both and in `docs/PROTOCOL.md`.

## Device data and capability gating

`app/src/main/assets/models.json` holds inherited model profiles. Profiles are evidence inputs, not hardware verification. A model-specific control should only appear when its protocol and capability information supports it. Unknown or unsupported writes must remain disabled. DeviceDeck target-model status is recorded in the umbrella support matrix.

## Control flow

1. The user changes a control. Android routes the request through its connection manager; desktop sends a typed command to the link thread.
2. The implementation sends only the command permitted by the model profile/capabilities.
3. Replies, notifications and acknowledgements update state. Controls should reflect device read-back rather than optimistic UI state.
4. A timeout, refusal or mismatched read-back leaves the reported state unchanged and surfaces an error.

The device is the source of truth; DeviceDeck is a local control interface.

## Connections and protocols

The current primary Android and desktop device-control screens implement the inherited OPPO/realme protocol. Android's `app/.../protocol/` contains the Kotlin codec and feature-sync engine. The same Android and desktop app also provides a shared cross-brand DeviceDeck hub for BLE devices (including Edifier W820NB Plus, Baseus Bowie 30, and Blade 2) and Baseus RFCOMM/SPP headsets (EP10 Pro, plus battery reading for Bowie MF1). Each vendor/model uses its own transport session and codec; these are internal modules within DeviceDeck, not separate apps. A catalog entry alone does not imply runtime support. DeviceDeck does not route Edifier or Baseus commands through OPPO framing. The Baseus reference checkout is research material only; all user-facing controls and releases belong to the existing unified DeviceDeck apps.

## Reports, privacy and updates

DeviceDeck does not send usage telemetry. The Android feedback form opens a prefilled GitHub issue in the browser; the user reviews and submits it. Diagnostic exports stay local until the user shares them. Desktop release checks use the public DeviceDeck GitHub Releases API directly. There is no active DeviceDeck report or update Worker dependency; `report-worker/` is retained as inherited source material only.

## Release process

Tag-based Android and desktop releases run only after the tagged commit is merged to `main`. Android release APKs must be signed with the persistent DeviceDeck release key. Windows/Linux archives include checksums. See `docs/RELEASING.md`.

## Maintenance rules

- Keep protocol changes and their evidence in the same reviewable change.
- Keep the Android and Rust implementations aligned where they share behavior.
- Add user-visible strings to every supported locale and run `scripts/check-locales.py` in CI.
- Do not deploy the inherited `report-worker/` as part of DeviceDeck releases.
