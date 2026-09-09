# Magnetita Android — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It adds
constraints for `magnetita-android/`; it cannot relax the root or grant
authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md),
  [VALIDATION.md](VALIDATION.md) and [DESIGN.md](DESIGN.md)
- [The Magnetita wire](../magnetita/docs/protocol.md) and
  [ADR 0001](../magnetita/docs/decisions/0001-own-protocol-and-android-app.md)
- [Architecture](../docs/standards/architecture.md) and
  [Verification](../docs/standards/verification.md)

## Local boundary

- Kotlin is UI, Android platform adapters and lifecycle. Every protocol rule,
  bound, pairing step and message lives in `magnetita-mobile`
  (`celestina-rs/crates/magnetita-mobile`), reached through its generated
  UniFFI bindings. A rule implemented in Kotlin is a defect.
- Product copy is Spanish, in `res/values/strings.xml`; identifiers,
  comments and logs are English. Kotlin and XML are outside the language
  guard's scan, so this is discipline, not enforcement.
- The session lives in a foreground service; an activity never owns a
  connection.
- No store, no analytics, no third-party SDK beyond AndroidX, Material and
  the barcode scanner. The app is installed by hand.
- Generated bindings and native libraries are build outputs under
  `app/build/`, never committed.

## Build and verification

- `scripts/build-native.sh` cross-compiles `magnetita-mobile` for arm64 and
  generates the Kotlin bindings; Gradle calls it before compiling.
- `./gradlew assembleDebug` builds; `./gradlew testDebugUnitTest` and
  `./gradlew lintDebug` verify. The SDK and NDK come from `local.properties`
  (`sdk.dir`) or `ANDROID_HOME`; neither is committed.
- Installing on the author's phone is `adb install -r` on request only.
