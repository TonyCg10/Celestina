# Magnetita Android

The phone side of Magnetita's own protocol
([ADR 0001](../magnetita/docs/decisions/0001-own-protocol-and-android-app.md)):
a Kotlin and Jetpack Compose application over the same Rust core the desktop
daemon links, reached through UniFFI.

## User contract

- Pair with the Celestina desktop by scanning the QR its Magnetita app shows,
  or by typing the six-digit code; keep the desktop pinned by certificate.
- A pairing link, scanned or opened from another app, pairs only after the
  person confirms the desktop's id, fingerprint and addresses on the
  consent screen, within two minutes and while the screen stays in front.
  Every address must be on the LAN (RFC 1918 IPv4 or `fc00::/7` IPv6) and
  none may be this phone's own, so a desktop reachable only through a
  public, CGNAT or VPN address cannot pair by link; the six-digit code is
  the path meant for that case once the wire carries it.
- Hold the session in a foreground service across screen off, app switches
  and Wi-Fi changes; reconnect without being asked.
- Grow capability by capability with the program's roadmap: battery, find
  and ping first, then the daily set, remote control, the mirror, and the
  phone's SMS, contacts and telephony.
- Speak Spanish to the person; English everywhere else.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/magnetita-mobile` | The phone side of the protocol: identity, pins, pairing, sessions |
| `app/src/main/java/org/celestina/magnetita/core` | The Kotlin face of that crate: one object per Rust object |
| `app/src/main/java/org/celestina/magnetita/ui/theme` | Tokens and the theme, per [DESIGN.md](DESIGN.md) |
| `app/src/main/java/org/celestina/magnetita/ui` | Screens and One UI-style components |
| `scripts/build-native.sh` | Cross-compiles the core and generates the bindings for Gradle |

## Build

```sh
./gradlew assembleDebug          # runs scripts/build-native.sh first
adb install -r app/build/outputs/apk/release/app-release.apk
```

Requires `rustup` with the `aarch64-linux-android` target, `cargo-ndk`, and
an Android SDK with an NDK, found through `local.properties` or
`ANDROID_HOME`.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Design](DESIGN.md)
- [Local agent delta](AGENTS.md)
