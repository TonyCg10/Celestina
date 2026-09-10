# Magnetita Android implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** AND-3
- **Related author validation:** `VAL-MAG-11`, `VAL-MAG-12` and
  `VAL-MAG-13` in Magnetita's [VALIDATION.md](../magnetita/VALIDATION.md);
  they do not block

This roadmap is the application half of Magnetita's own-protocol program;
the Rust half and the desktop are in
[Magnetita's roadmap](../magnetita/ROADMAP.md) (`MAG-P3` onwards). Each
`AND-` checkpoint pairs with a `MAG-P` one.

## AND-3 — Remote control

## Hypothesis and tangible outcome

The phone as trackpad, keyboard and command deck: one Compose page over the
core's input and command calls, with the arithmetic pure and tested.

## Scope

- The control screen: trackpad gestures, the typing field and key row, the
  registered commands by name; a fast path for input.

## Exclusions

- Presenter mode, gamepad, absolute positioning.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| AND-3-A | done | MAG-P5-B | The control screen: trackpad, keyboard, commands | JVM tests, lint |

## Implementation exit

Close `AND-3` when the screen's arithmetic and signals have JVM tests and
`lintRelease` passes; `VAL-MAG-13` carries the author's hand.

Met on 2026-09-09 as far as this project can meet it: the tests and lint
pass; the pointer's motion is the author's observation.

## Closed evidence

- `AND-3-A`: [control screen](docs/evidence/2026-09-09-control-screen.md)

## AND-2 — The daily set

## Hypothesis and tangible outcome

Each daily capability is one Kotlin adapter over the core: the platform
service that Android gives (clipboard manager, notification listener,
storage access, media session, contacts and SMS providers, telecom) and
nothing else. The tangible outcome is the author's daily set on the phone
without the stock client: clipboard, notifications, files, media,
contacts, SMS and calls.

## Scope

- Clipboard both ways: the application in front, a quick-settings tile and
  a share target for the rest, the Android limit stated on the screen.
- Notifications through `NotificationListenerService`, with actions and
  inline replies.
- File share both ways through the storage access framework.
- Media through `MediaSession` both ways.
- Contacts, SMS and telephony through their providers and `TelecomManager`.

## Exclusions

- Commands, input and mirror: `AND-3` onwards.
- Any third-party SDK.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| AND-2-A | done | MAG-P4-A | Clipboard both ways: in front, tile, share target | JVM tests, lint |
| AND-2-B | done | MAG-P4-B | Notifications with actions and replies | JVM tests |
| AND-2-C | done | MAG-P4-C | File share both ways | JVM tests |
| AND-2-D | done | MAG-P4-D | Media control both ways | JVM tests |
| AND-2-E | done | MAG-P4-E | Contacts sync | JVM tests |
| AND-2-F | done | MAG-P4-F | SMS send and receive, MMS attachments | JVM tests |
| AND-2-G | done | MAG-P4-G | Call state, mute, answer, hang up | JVM tests |

## Implementation exit

Close `AND-2` when every adapter has its JVM tests, the release build passes
lint, and `MAG-P4`'s exit is met. `VAL-MAG-12` carries the author's daily
use.

Met on 2026-09-09: seven adapters with their JVM tests, `lintRelease` clean,
`MAG-P4`'s exit met the same day. `VAL-MAG-12` closes the checkpoint.

## Closed evidence

- `AND-2-A`: [clipboard](docs/evidence/2026-09-09-daily-clipboard.md)
- `AND-2-B`: [notifications](docs/evidence/2026-09-09-daily-notifications.md)
- `AND-2-C`: [share](docs/evidence/2026-09-09-daily-share.md)
- `AND-2-D`: [media](docs/evidence/2026-09-09-daily-media.md)
- `AND-2-E`: [contacts](docs/evidence/2026-09-09-daily-contacts.md)
- `AND-2-F`: [SMS](docs/evidence/2026-09-09-daily-sms.md)
- `AND-2-G`: [telephony](docs/evidence/2026-09-09-daily-telephony.md)

## AND-1 — The application foundation

## Hypothesis and tangible outcome

A Kotlin application that links `magnetita-mobile` through UniFFI can hold a
paired session in a foreground service across Doze, app switches and Wi-Fi
changes, with all protocol truth in Rust and only platform adapters in
Kotlin. The tangible outcome is the project in the repository, building the
Rust core from Gradle, with the design tokens in place, discovery, QR
pairing, a device screen with battery, ping and find, and reconnection
without touching the phone.

## Scope

- Scaffold in the repository: Gradle KTS, version catalog, package
  `org.celestina.magnetita`, `minSdk 31`; `scripts/build-native.sh`.
- The theme and components of [DESIGN.md](DESIGN.md).
- Foreground service holding the session; multicast lock and `NsdManager`
  browse for `_magnetita._udp`; pins in the app's own directory.
- QR scan with CameraX and ML Kit; the typed six-digit path.
- Device screen: name, connection state, battery both ways, ping, find.

## Exclusions

- The daily set, commands, input, mirror, SMS, contacts, telephony.
- Any store, cloud, analytics or third-party SDK beyond AndroidX, Material
  and the barcode scanner.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| AND-1-A | done | MAG-P3-A | The scaffold builds the Rust core from Gradle; the theme and the identity screen | `./gradlew assembleDebug` |
| AND-1-B | done | AND-1-A | Foreground service holding a session; discovery; pins | JVM tests |
| AND-1-C | done | AND-1-B | Pairing screens (QR and code), device screen, battery, ping, find | JVM tests, lint |
| AND-1-D | done | AND-1-C | Verify and deploy scripts; version reading; signed build | documentation contract |

## Implementation exit

Close `AND-1` when the release build passes lint and unit tests, the app pairs
with the daemon on the LAN, and a session survives screen off, app switch and
Wi-Fi toggle. All four units are done on 2026-09-09; the survival run is the
manual one of `AND-1-B`, not an instrumented test. `VAL-MAG-11` carries the
author's first pairing on the S25U and closes the checkpoint.

## Closed evidence

- `AND-1-A`: [scaffold](docs/evidence/2026-09-09-foundation-scaffold.md)
- `AND-1-B`: [link service](docs/evidence/2026-09-09-foundation-link.md)
- `AND-1-C`: [screens](docs/evidence/2026-09-09-foundation-screens.md)
- `AND-1-D`: [release](docs/evidence/2026-09-09-foundation-release.md)
