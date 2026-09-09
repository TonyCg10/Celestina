# AND-1 — The application foundation

- **Opened:** 2026-09-09
- **Plan ID:** foundation
- **Status:** active
- **Authorization:** the author said "abre MAG-P3 y empieza" on 2026-09-09
  and asked for MilaHub's design philosophy in Samsung blue
- **Scope:** magnetita-android
- **Implementation checkpoint:** AND-1
- **Author-validation checkpoint:** `VAL-MAG-11` in Magnetita's
  [`../../../../magnetita/VALIDATION.md`](../../../../magnetita/VALIDATION.md)

## Hypothesis

A Kotlin and Compose application can be a thin face over `magnetita-mobile`:
every protocol truth in Rust, every platform adapter in Kotlin, the design
tokens of [DESIGN.md](../../../DESIGN.md) throughout.

## Tangible outcome

The project in the repository, building the Rust core from Gradle, with the
theme in place and the screens that pair, show the device and hold a session
in a foreground service.

## Scope

- `AND-1-A` — the scaffold: package, versions, `scripts/build-native.sh`
  hooked into Gradle, the theme tokens and components, an identity screen
  that reads the device id and fingerprint from the core.
- `AND-1-B` — foreground service holding a session; `NsdManager` browse;
  pins persisted; reconnection.
- `AND-1-C` — QR and code pairing screens, the device screen with battery,
  ping and find.
- `AND-1-D` — verify and deploy scripts, version reading, a signed build.

## Exclusions

- Anything past battery, find and ping; `MAG-P4` onwards.
- Committing the SDK location, a keystore or any build output.

## Build order

1. `AND-1-A`, then `-B`, `-C`, `-D`.

## Implementation exit

`./gradlew assembleDebug`, `testDebugUnitTest` and `lintDebug` pass with the
core built by Gradle; the app pairs with `magnetita-peer` or the daemon and
holds a session through screen off, app switch and Wi-Fi toggle.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| AND-1-A | `magnetita-android:` | done | [inventory](../../inventories/2026-09-09-foundation/AND-1-A.numstat.tsv) | 56 files, +1906/-0 | Scaffold building the core from Gradle, the theme, the identity screen, the document set | [record](../../evidence/2026-09-09-foundation-scaffold.md) | None |
| AND-1-B | `magnetita-android:` | done | [inventory](../../inventories/2026-09-09-foundation/AND-1-B.numstat.tsv) | 18 files, +799/-7 | Foreground service, discovery, pins, reconnection, the pairing link | [record](../../evidence/2026-09-09-foundation-link.md) | None |
| AND-1-C | `magnetita-android:` | done | [inventory](../../inventories/2026-09-09-foundation/AND-1-C.numstat.tsv) | 22 files, +744/-132 | Scan and device screens, find ringer, forget, the production scripts on the runner | [record](../../evidence/2026-09-09-foundation-screens.md) | None |
| AND-1-D | `magnetita-android:` | planned | `scripts/`, `app/build.gradle.kts` | — | Lint baseline, version reading, signing | documentation contract, lint | None |
