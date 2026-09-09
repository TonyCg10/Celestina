# Magnetita Android status

- **Updated:** 2026-09-09
- **Implementation:** `AND-1`, the application foundation, is the active
  checkpoint since 2026-09-09, opened with Magnetita's `MAG-P3`
- **Author validation:** `VAL-MAG-11` in Magnetita's
  [VALIDATION.md](../magnetita/VALIDATION.md) covers the first pairing on
  the S25U; this project's own lane starts empty

## Current checkout truth

- The project is the author's Android Studio scaffold moved into the
  repository under the package `org.celestina.magnetita`, `minSdk 31`,
  Kotlin 2.2, Compose Material 3, AGP 9.
- Gradle runs `scripts/build-native.sh` before compiling: it cross-compiles
  `magnetita-mobile` for arm64 with `cargo-ndk` and generates the Kotlin
  bindings with the crate's own `uniffi-bindgen`, both into `app/build/`.
- The theme carries the One UI tokens of [DESIGN.md](DESIGN.md) in Samsung
  blue; the first screen shows the device's identity read from the Rust core.

## Blockers

None. The SDK and NDK are the author's, outside the repository.

## Records

- [Implementation roadmap](ROADMAP.md)
- [Active plan AND-1](docs/plans/active/2026-09-09-foundation.md)
- [Registry entry](../docs/projects.toml)
