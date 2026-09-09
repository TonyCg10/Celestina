# MAG-P3 — The Android application foundation, Rust side

- **Opened:** 2026-09-09
- **Plan ID:** android-foundation
- **Status:** active
- **Authorization:** the author said "abre MAG-P3 y empieza" on 2026-09-09,
  asking for the design philosophy of their MilaHub project — One UI
  tokens and components — in Samsung blue, adapted rather than copied
- **Scope:** magnetita, magnetita-mobile, magnetita-peer
- **Implementation checkpoint:** MAG-P3
- **Author-validation checkpoint:** `VAL-MAG-11` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The phone side of the protocol is one Rust crate, `magnetita-mobile`, that the
headless peer and the Android application both link; UniFFI exposes it to
Kotlin without a second implementation of any rule.

## Tangible outcome

`magnetita-mobile` builds for `aarch64-linux-android` and for the host,
`magnetita-peer` is a thin shell over it, and the `magnetita-android` project
(checkpoint `AND-1`) links it through generated Kotlin bindings.

## Scope

- `MAG-P3-A` — `magnetita-mobile`: the phone-side session logic moved out of
  the peer (identity from the certificate, pins in a directory, pair by QR,
  connect, report, receive), exposed through UniFFI objects with a
  per-crate `uniffi-bindgen`; the peer delegates to it.
- `MAG-P3-B` — the desktop app shows the QR from `StartPairing`; the daemon
  side of the phone's daily capabilities as the application grows.
- `MAG-P3-C` — the roadmap's exit against the real phone (`VAL-MAG-11`).

## Exclusions

- Every Kotlin file: `magnetita-android`'s `AND-1`.
- The daily set: `MAG-P4`.

## Build order

1. `MAG-P3-A`, so `AND-1-A` has a library to link.
2. `AND-1` units on the application side, interleaved.
3. `MAG-P3-B`, then the phone run.

## Implementation exit

`cargo test -p magnetita-mobile -p magnetita-peer` pass, the crate builds
for Android with `cargo ndk`, and `AND-1-A`'s Gradle build consumes it.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| MAG-P3-A | `magnetita:` | done | [inventory](../../inventories/2026-09-09-android-foundation/MAG-P3-A.numstat.tsv) | 24 files, +1267/-385 | The phone-side crate, UniFFI-exposed; the peer delegates; `MAG-P2` archived and `MAG-P3` opened | [record](../../evidence/2026-09-09-mobile-crate.md) | None |
| MAG-P3-B | `magnetita:` | done | [inventory](../../inventories/2026-09-09-android-foundation/MAG-P3-B.numstat.tsv) | 16 files, +496/-10 | The desktop app shows the pairing QR and knows when the phone arrived | [record](../../evidence/2026-09-09-desktop-qr.md) | `VAL-MAG-11` |
| MAG-P3-C | `magnetita:` | done | [inventory](../../inventories/2026-09-09-android-foundation/MAG-P3-C.numstat.tsv) | 6 files, +212/-4 | The real phone pairs and holds a session; the daemon answers a QR proof from a phone it still pins | [record](../../evidence/2026-09-09-phone-pairs-again.md) | `VAL-MAG-11` |
