# Magnetita Android status

- **Updated:** 2026-09-09
- **Implementation:** `AND-2`, the daily set, is the active checkpoint
  since 2026-09-09, paired with Magnetita's `MAG-P4`; its seven units are
  done and its exit is met; `VAL-MAG-12` closes it. `AND-1`, the foundation, has its four units done and waits for
  `VAL-MAG-11`
- **Author validation:** `VAL-MAG-11` in Magnetita's
  [VALIDATION.md](../magnetita/VALIDATION.md) covers the first pairing on
  the S25U; this project's own lane starts empty

## Current checkout truth

- With the phone grants, contacts go to the desktop as vCards, the SMS
  list and threads on request, incoming SMS as they arrive, sends from the
  desktop through the SMS manager, and calls with mute, answer and hang
  up from the desktop (`AND-2-E`, `-F`, `-G`).
- The phone's active player goes to the desktop and its buttons come
  back; the desktop's player shows on the device screen with previous,
  play/pause and next (`AND-2-D`).
- Files shared to Magnetita from any app reach the desktop on their own
  streams; files the desktop sends land in `Downloads/Magnetita` with a
  notification (`AND-2-C`).
- The phone's notifications reach the desktop through the notification
  listener, with their buttons and reply; the desktop's dismissals,
  presses and replies come back (`AND-2-B`). Notification access is
  granted on the system page the device screen opens.
- The clipboard travels both ways: the desktop's text becomes the phone's;
  the phone's goes out in front, from the quick-settings tile and from the
  share target (`AND-2-A`).
- The project is the author's Android Studio scaffold moved into the
  repository under the package `org.celestina.magnetita`, `minSdk 31`,
  Kotlin 2.2, Compose Material 3, AGP 9.
- Gradle runs `scripts/build-native.sh` before compiling: it cross-compiles
  `magnetita-mobile` for arm64 with `cargo-ndk` and generates the Kotlin
  bindings with the crate's own `uniffi-bindgen`, both into `app/build/`.
- The theme carries the One UI tokens of [DESIGN.md](DESIGN.md) in Samsung
  blue; the first screen shows the device's identity read from the Rust core
  and the state of the link.
- A foreground service of type `connectedDevice` holds the session: it
  browses `_magnetita._udp` through `NsdManager`, dials only pinned
  desktops, reports the battery, and reconnects on a schedule. A
  `magnetita://pair` link pairs. On the S25U the session survived screen
  off, an app switch and a Wi-Fi toggle against the live daemon.
- The device screen shows the desktop, the link, the address and the
  battery sent; the scan screen reads the desktop's QR with CameraX and
  ML Kit; the phone rings on find until stopped; forget drops the pin.
  The production scripts run through the suite's runner. The six-digit
  code screen waits for the wire.
- The artifact is the release APK, signed with the author's key when
  `keystore.properties` names it; lint runs in the verification; the
  suite reads the version from Gradle, so the project is versioned at
  `0.1.0`. `AND-1`'s four units are done; `VAL-MAG-11` closes it.

## Blockers

None. The SDK and NDK are the author's, outside the repository.

## Records

- [Implementation roadmap](ROADMAP.md)
- [Active plan AND-2](docs/plans/active/2026-09-09-daily-set.md)
- [Archived plan AND-1](docs/plans/archive/2026-09-09-foundation.md)
- [Registry entry](../docs/projects.toml)
