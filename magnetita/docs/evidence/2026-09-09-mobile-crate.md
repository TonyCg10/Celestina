# `magnetita-mobile`: the phone side as one crate, UniFFI-exposed; the peer delegates — MAG-P3-A

- **Date:** 2026-09-09
- **Scope:** `MAG-P3-A` of
  [`../plans/archive/2026-09-09-android-foundation.md`](../plans/archive/2026-09-09-android-foundation.md):
  `celestina-rs/crates/magnetita-mobile` (`Cargo.toml`, `src/lib.rs`,
  `src/phone.rs`, `src/mobile.rs`, `src/bin/uniffi-bindgen.rs`),
  `magnetita-peer` now a shell over it, the workspace manifests, and the
  opening of `MAG-P3` with `MAG-P2` archived
- **Environment:** the pinned `1.97.1` toolchain with the
  `aarch64-linux-android` target added to it, `cargo-ndk 4.1.2`, NDK
  30.0.16248370, `uniffi 0.29`
- **Artifact:** `libmagnetita_mobile.so` for arm64 (4.9 MB), built by
  `magnetita-android/scripts/build-native.sh` into that project's build
  directory; not committed

## Procedure

```sh
cd celestina-rs
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p magnetita-mobile -p magnetita-peer -p magnetita-link
cargo ndk -t arm64-v8a -o /tmp/jni build --release -p magnetita-mobile
cargo run -p magnetita-mobile --bin uniffi-bindgen -- generate --library target/release/libmagnetita_mobile.so --language kotlin --out-dir /tmp/kotlin
```

## Result

- **Exit:** 0 for every command
- **Observed:** format and Clippy clean across the workspace; the peer's
  loopback pairing test passes over the moved logic; the link's 11 tests
  unchanged; the arm64 library builds; the bindings generate
  `uniffi.magnetita_mobile` with `MobilePhone` (`open`, `identity`,
  `pinned`, `forget`, `pair`, `connect`), `MobileSession`
  (`desktopId`, `desktopName`, `reportBattery`, `next`, `close`) and the
  records `Identity`, `PinnedDesktop`, `Event`.
- **One owner:** `Phone` and `PhoneSession` moved from the peer into
  `magnetita-mobile::phone` unchanged in behaviour; the peer keeps only
  the Avahi browse, the default directory and the binary. The UniFFI face
  in `mobile.rs` holds the runtime so Kotlin never sees tokio, and every
  call blocks — the application calls them from its service, never from
  the UI thread.

## Limits

- The bindings are exercised by the Android build and the identity
  screen of `AND-1-A`, not by a Kotlin test of their own.
- Sessions are not yet held by a service on the phone: `AND-1-B`.

## Follow-up

`AND-1-A` consumes this crate; `MAG-P3-B` shows the QR on the desktop.
