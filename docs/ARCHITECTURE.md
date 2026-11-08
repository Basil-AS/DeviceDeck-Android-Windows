# DeviceDeck Android and desktop architecture

This repository is the Android, Windows and Linux application fork. It currently inherits the OnePlus/OPPO/realme implementation from QuickBuds; DeviceDeck-specific support for Edifier and Baseus is tracked separately and must not be inferred from a UI row or catalog entry. See the umbrella repository's support matrix and protocol research.

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

The current primary Android and desktop device-control screens implement the inherited OPPO/realme protocol. Android's `app/.../protocol/` contains the Kotlin codec and feature-sync engine. The Android app also has a shared GATT link and catalogued Edifier/Baseus endpoint profiles. The in-app **More devices** hub is a shared cross-brand DeviceDeck surface: it connects to Baseus Bowie 30 (battery and noise modes) and EDIFIER W820NB Plus (battery and Off/ANC/Transparency). Edifier controls use its own frame codec/session and are gated on state readback; both device paths still need physical hardware verification. Blade 2 has transport/reference protocol material but no live DeviceDeck session or controls yet. The desktop uses `core/` and platform link code. Other vendors require their own protocol session and controls; do not route an Edifier or Baseus device through OPPO framing merely because an app database lists it. Baseus protocol research may be sourced from the separate reference checkout, but all user-facing controls and product releases belong to DeviceDeck's existing platform apps.

## Reports, privacy and updates

DeviceDeck does not send usage telemetry. The Android feedback form opens a prefilled GitHub issue in the browser; the user reviews and submits it. Diagnostic exports stay local until the user shares them. Desktop release checks use the public DeviceDeck GitHub Releases API directly. There is no active DeviceDeck report or update Worker dependency; `report-worker/` is retained as inherited source material only.

## Release process

Tag-based Android and desktop releases run only after the tagged commit is merged to `main`. Android release APKs must be signed with the persistent DeviceDeck release key. Windows/Linux archives include checksums. See `docs/RELEASING.md`.

## Maintenance rules

- Keep protocol changes and their evidence in the same reviewable change.
- Keep the Android and Rust implementations aligned where they share behavior.
- Add user-visible strings to every supported locale and run `scripts/check-locales.py` in CI.
- Do not deploy the inherited `report-worker/` as part of DeviceDeck releases.
