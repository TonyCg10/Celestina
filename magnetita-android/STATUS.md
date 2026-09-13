# Magnetita Android status

- **Updated:** 2026-09-13
- **Implementation:** `AND-6`, the application's design, is the one open
  checkpoint since 2026-09-13, paired with Magnetita's `MAG-D1`: the
  author closed the own-protocol program with `VAL-MAG-15` passed. `AND-5`,
  the phone's files over the link, is archived with its three units done. `AND-4`, the mirror over
  the link, is implemented and archived; `VAL-MAG-14` carries the author's
  observation. `AND-3`, remote control,
  met its exit the same day and is archived; `VAL-MAG-13` carries the
  author's hand. `AND-2`, the daily set, met its exit the same day and is
  archived; `VAL-MAG-12` closes it. `AND-1`, the foundation, has its four units done and waits for
  `VAL-MAG-11`
- **Author validation:** `VAL-MAG-11` in Magnetita's
  [VALIDATION.md](../magnetita/VALIDATION.md) covers the first pairing on
  the S25U; this project's own lane starts empty

## Current checkout truth

- The whole phone (with the all-files grant) or the folder picked in the
  "Archivos" row is the shared root: the
  desktop's listings, reads, writes, renames and deletions run through
  the documents contract on one storage thread, and the link says whether
  a root is shared on every connect (`AND-5-A`).
- The desktop's Mirror asks for the screen; the phone consents through
  the system's dialog (opened at once in front, from a notification
  otherwise), captures into an HEVC encoder and streams it on the link's
  video stream; the desktop's touches, back, home and recents land through
  the accessibility service the device screen's row enables (`AND-4-A`,
  `-B`).
- The control screen turns the phone into the desktop's trackpad,
  keyboard and command deck; motion takes a fast path to the held
  session (`AND-3-A`).
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
- [Active plan AND-6](docs/plans/active/2026-09-13-app-design.md)
- [Archived plan AND-5](docs/plans/archive/2026-09-10-storage.md)
- [Archived plan AND-4](docs/plans/archive/2026-09-09-link-mirror.md)
- [Archived plan AND-3](docs/plans/archive/2026-09-09-remote-control.md)
- [Archived plan AND-2](docs/plans/archive/2026-09-09-daily-set.md)
- [Archived plan AND-1](docs/plans/archive/2026-09-09-foundation.md)
- [Registry entry](../docs/projects.toml)
