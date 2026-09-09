# Magnetita Android implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** AND-1
- **Related author validation:** `VAL-MAG-11` in Magnetita's
  [VALIDATION.md](../magnetita/VALIDATION.md); does not block

This roadmap is the application half of Magnetita's own-protocol program;
the Rust half and the desktop are in
[Magnetita's roadmap](../magnetita/ROADMAP.md) (`MAG-P3` onwards). Each
`AND-` checkpoint pairs with a `MAG-P` one.

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
| AND-1-C | planned | AND-1-B | Pairing screens (QR and code), device screen, battery, ping, find | JVM tests, lint |
| AND-1-D | planned | AND-1-C | Verify and deploy scripts; version reading; signed build | documentation contract |

## Implementation exit

Close `AND-1` when the debug build passes lint and unit tests, the app pairs
with `magnetita-peer` on an emulator or with the daemon on the LAN, and a
session survives screen off, app switch and Wi-Fi toggle in an instrumented
test. `VAL-MAG-11` carries the author's first pairing on the S25U.

## Closed evidence

- `AND-1-A`: [scaffold](docs/evidence/2026-09-09-foundation-scaffold.md)
- `AND-1-B`: [link service](docs/evidence/2026-09-09-foundation-link.md)
