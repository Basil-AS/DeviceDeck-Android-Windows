<div align="center">

# DeviceDeck

**One cross-platform control app for headphones, earbuds and Bluetooth accessories.**

Android · Windows · Linux

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](./LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Android%208%2B%20%7C%20Windows%20%7C%20Linux-3DDC84.svg)](#install)
[![Release](https://img.shields.io/badge/Release-latest-blue.svg)](https://github.com/Basil-AS/DeviceDeck-Android-Windows/releases/latest)

<img src="docs/images/phone.png" width="230" alt="DeviceDeck on a phone"> &nbsp; <img src="docs/images/desktop.png" width="560" alt="DeviceDeck on a PC">

</div>

DeviceDeck is one application per platform. Edifier, realme, Baseus and other vendor devices appear as profiles in the same app and release channel. Device features vary by model and firmware; the umbrella [support matrix](https://github.com/Basil-AS/DeviceDeck/blob/develop/SUPPORT_MATRIX.md) tracks what is implemented and what still needs device testing.

## Supported earbuds

The Android app runs on Android 8+. The inherited profile database covers many OnePlus, OPPO and realme models;
profile presence does not prove that every listed control works on every firmware. DeviceDeck's additional target
models and the per-platform evidence are tracked in the umbrella repository's support matrix.

On OnePlus, OPPO and realme phones (OxygenOS 12 and later, ColorOS, realme UI), the system's own
earbud settings can hold the control link of the earbuds. DeviceDeck cannot connect in that case. Use
the earbud settings in Settings › Bluetooth › your earbuds, or report the case.

<details><summary><b>Inherited device profile database</b></summary>

The fork currently inherits profiles for 143 OnePlus, OPPO and realme models from its upstream. This is a protocol/profile catalog, not a list of models verified with DeviceDeck. Compatibility and working controls must be established separately for each model and platform. See the umbrella repository support matrix for evidence and target-device progress.

</details>

DeviceDeck is an early fork build. Some inherited screens and profiles have not yet been audited against this fork's implementation. Android feedback opens an issue draft in your browser; review it before submitting. For support, open a [GitHub issue](https://github.com/Basil-AS/DeviceDeck-Android-Windows/issues) or a [Discussion](https://github.com/Basil-AS/DeviceDeck-Android-Windows/discussions).

### A feature is missing or does not work? Open an issue with a snoop log

The app's log is not enough to add a missing feature. An issue for a missing feature needs a Bluetooth snoop log of the vendor app while changing that setting, plus a screen recording or numbered screenshots. See [the capture guide](./docs/PACKET-CAPTURE.md#send-a-snoop-log-for-a-missing-feature).

## Install

Android and Windows/Linux 0.1.0-alpha.27 use the EDIFIER W820NB Plus control service and characteristics extracted from EDIFIER Connect 8.4.28; connection and command round-trip still need headset validation. Alpha.26 adds original realme Buds Air7/Air7 Pro illustrations to the Android model picker. DeviceDeck remains one app per platform; realme, Edifier and Baseus devices are profiles in the same app. The shared hub includes W820NB Plus controls, Baseus EP10 Pro and Bowie MF1 headset controls, Bowie 30 noise modes, and Blade 2 telemetry/display/timer/USB-C port settings. See the [DeviceDeck support matrix](https://github.com/Basil-AS/DeviceDeck/blob/develop/SUPPORT_MATRIX.md). Pair Baseus SPP headsets in system Bluetooth settings first; DeviceDeck connects to paired devices.

| Platform | How |
| --- | --- |
| **Android 8+** | Install `DeviceDeck-Android-<version>.apk` from the [GitHub Releases](https://github.com/Basil-AS/DeviceDeck-Android-Windows/releases). Allow the Bluetooth permission when asked. |
| **Windows 10 / 11** | Unzip `DeviceDeck<version>-windows-x64.zip` and run `quickbuds.exe`. It is portable: no installer, no drivers, no background service. |
| **Linux** | Run the command below. It needs only `curl` and BlueZ. |

```sh
curl -fsSL https://raw.githubusercontent.com/Basil-AS/DeviceDeck-Android-Windows/develop/scripts/install-linux.sh | sh
```

The script needs no administrator rights, adds a menu entry, and the app updates itself. More ways to install, and how to remove it: [INSTALL-LINUX.md](./docs/INSTALL-LINUX.md).

Android release APKs are signed with the DeviceDeck release key when published. Verify the signer with
`apksigner verify --print-certs <apk>` and compare the fingerprint with the one published in the release notes.

## What it does

| Area | What you get |
| --- | --- |
| **Device controls** | Features vary by model and firmware. A profile alone does not mean a command has been tested. |
| **Battery, sound and noise control** | Implemented only for compatible protocols and tested models; see the support matrix. |

| **Hearing** | A hearing test that tunes the sound to your ears, and a fit test of the ear-tip seal. |
| **Controls** | Gestures per bud, wear detection, dual connection, find my earbuds, prompt volume, tap sensitivity, firmware version. |

| **Android** | Home screen widgets from 2×2 up: the battery first, then the noise control and switches after a double tap. A 1×1 widget steps through your noise control modes. Quick Settings tiles: noise control, low latency, wind noise reduction and open DeviceDeck. A notification with the battery rings. On Android 16, a status chip can show the earbud levels in the status bar. A ten-page setup guide. A game equalizer that follows the game in the foreground (needs Usage access). |
| **PC** | A tray icon with quick controls (Windows) or a battery menu (Linux). |
| **Looks** | Material, DeviceDeck, Pixel art and Glass styles. OLED Black, Classic Dark and White themes, wallpaper colours, your own accent and palettes. Glass has six backgrounds. The phone and its widgets have all four styles. The PC app has the first three. |
| **Feedback** | Android feedback opens a draft issue in your browser. Review it before submitting. DeviceDeck does not upload usage telemetry or logs. |
| **Languages and updates** | 27 languages, switchable in the app. Updates come from GitHub releases. The PC app updates itself. |

<div align="center">

| Widget: battery | Widget: noise control | Tray: Linux |
| :---: | :---: | :---: |
| <img src="docs/images/widget-battery.png" width="170"> | <img src="docs/images/widget-controls.png" width="170"> | <img src="docs/images/tray-linux.png" width="220"> |

</div>

## For developers

- [CONTRIBUTING.md](./docs/CONTRIBUTING.md): build, tests, translations, where things are, and the rules.
- [build-android.md](./docs/build-android.md) and [build-desktop.md](./docs/build-desktop.md): the build steps for each app.
- [RELEASING.md](./docs/RELEASING.md): versioning, signed Android assets and tag-based GitHub releases.
- [PACKET-CAPTURE.md](./docs/PACKET-CAPTURE.md): capturing the earbuds' traffic, HeyMelody's included.
- [PROTOCOL.md](./docs/PROTOCOL.md): the wire format, every claim tagged with its source.
- [ROADMAP.md](./docs/ROADMAP.md): the plan, and the features decided against.

Everything to build and release DeviceDeck is in this repository, except the signing key. If the
project stops, anyone can fork it and publish their own signed build.

## Credits

The first hints about the earbud protocol came from
[OppoPodsManager](https://github.com/Zhaoyi-ya/OppoPodsManager),
[Pods-Protocol-Reverse-Engineering](https://github.com/Star-ZER0/Pods-Protocol-Reverse-Engineering) and
[OppoPods](https://github.com/Leaf-lsgtky/OppoPods). Today every fact in [PROTOCOL.md](./docs/PROTOCOL.md) rests on a
capture, a device test, or the vendor apps. The project studied the vendor apps for interoperability only.

Built with [Slint](https://slint.dev) (GPLv3) and [tray-icon](https://github.com/tauri-apps/tray-icon) on the PC.
Fonts: [Doto](https://github.com/oliverlalan/Doto) and [Google Sans Flex](https://github.com/googlefonts/googlesans-flex),
both SIL Open Font License 1.1, licenses bundled in `app/src/main/assets/`.

## Privacy

DeviceDeck sends no usage telemetry. Bluetooth control stays on the device. Update checks contact the public GitHub Releases API. Android feedback opens a GitHub issue draft for review; DeviceDeck itself does not upload reports or logs.

## Feature ideas

These are ideas, not plans. Nothing here is promised. To propose one, open a
[feature idea](https://github.com/Basil-AS/DeviceDeck-Android-Windows/issues/new?template=feature-idea.yml).

**Launch an app with an earbud gesture.** You pick a gesture, for example a triple tap on the left bud.
You pick an installed app or action. Examples are Spotify, a playlist, or a voice assistant such as
Gemini, ChatGPT or Claude. DeviceDeck runs it when the gesture happens.

Open questions:

- Do the earbuds report a gesture over the control link when that gesture has no function set? If yes,
  the app can act on it. If no, a workaround needs its own research.
- HeyMelody already offers "voice assistant" as a triple-tap function on some models. Then the phone's
  chosen assistant app starts. That path can work for models that offer it.
- It differs from HeyMelody's firmware "Spotify Tap", which no model uses (see
  [ROADMAP.md](./docs/ROADMAP.md)).

Status: idea, no research yet.

## License

This fork preserves upstream copyrights and GPL terms. DeviceDeck-specific changes are contributed under GPL-3.0-or-later.

The upstream application and this fork are distributed under GPL-3.0-or-later. Preserve upstream copyright notices and license terms. See [LICENSE](./LICENSE) and [docs/CLA.md](./docs/CLA.md); the upstream CLA does not apply to contributions to this fork.

DeviceDeck is an independent project. It is not affiliated with OnePlus, OPPO, realme, HeyTap, Edifier, Baseus or their
affiliates. Those companies do not endorse or sponsor it. OnePlus, OPPO, realme, HeyMelody and the earbud
model names are trademarks of their owners. This text uses them only to say which devices the app works
with. The project studied the protocol only to make these earbuds work with this app.
