# The scaffold builds the Rust core from Gradle; the theme and the identity screen — AND-1-A

- **Date:** 2026-09-09
- **Scope:** `AND-1-A` of
  [`../plans/archive/2026-09-09-foundation.md`](../plans/archive/2026-09-09-foundation.md):
  the whole of `magnetita-android/` as first committed — Gradle files,
  `scripts/`, `app/src/main/`, `app/src/test/`, and the document set
- **Environment:** the author's Android Studio Flatpak SDK (platform 36,
  build-tools 36, NDK 30.0.16248370) named by an uncommitted
  `local.properties`; Gradle 9.5, AGP 9.3.2, Kotlin 2.2.10, Compose BOM
  2026.02; `jdk-openjdk 26` on the host with the foojay toolchain resolver
- **Artifact:** `app/build/outputs/apk/debug/app-debug.apk` (40 MB, debug,
  unsigned for release), not committed

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q assembleDebug testDebugUnitTest
unzip -l app/build/outputs/apk/debug/app-debug.apk | grep lib/arm64-v8a/
scripts/verify-production.sh && scripts/status-production.sh
```

## Result

- **Exit:** 0
- **Observed:**
  - `preBuild` runs `scripts/build-native.sh`, which cross-compiles
    `magnetita-mobile` with `cargo ndk` and generates the Kotlin bindings
    with the crate's own `uniffi-bindgen`, both under `app/build/native/`;
    the variant API registers those two directories as Kotlin and
    `jniLibs` sources. The APK carries `libmagnetita_mobile.so` and
    JNA's `libjnidispatch.so` for arm64 — the JNA dependency must be the
    `aar` artifact, or the phone has no dispatcher.
  - The package is `org.celestina.magnetita`, `minSdk 31`; product copy is
    in `res/values/strings.xml`, Spanish.
  - The theme carries [DESIGN.md](../../DESIGN.md)'s tokens: pure black
    canvas, blue-tinted near-black surfaces, Samsung blue accent, mint for
    a live link, the One UI shape scale (26/32/18/12) and type scale
    (32 sp medium title collapsing to 20 sp bold). Two unit tests pin the
    canvas, the tint and the accent.
  - The One UI components exist as composables — the glow canvas, the
    collapsing header, the group and its rows, the state chip — and the
    identity screen composes them to show the device id, the fingerprint
    and the pinned desktops read from the Rust core off the UI thread.
  - `verify-production.sh` runs the core's tests and the app's unit tests
    and checks the artifact; `status-production.sh` reports it current.

### On the S25U, 14:27

Installed with `adb install -r` and launched: the activity started
(`Status: ok`), no runtime error in `logcat`, and the identity screen
rendered as designed — the black canvas with the blue glow from the
top-left, the large title (the "this phone" copy) with the model `SM-S938U` in
the accent as subtitle, the group card tinted toward blue with its
hairlines, the mint "OK" chip once the Rust core answered, the device id
`4eb6f9984054dd25` and the colon-hex fingerprint read through UniFFI, the
empty paired-desktops group and the pairing hint. The core
therefore opens under the app's files directory, generates and persists a
certificate, and derives the id from it, all on the phone.

## Limits
- No foreground service, discovery, pairing screen or session yet:
  `AND-1-B` and `-C`.
- `lintDebug` is not part of the verify script yet; it needs the SDK's
  lint jars and a baseline, `AND-1-D`.

## Follow-up

Install on the S25U, screenshot the identity screen, and append it here.
