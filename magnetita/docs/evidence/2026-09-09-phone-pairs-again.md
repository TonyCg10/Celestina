# The real phone pairs and holds a session — MAG-P3-C

- **Date:** 2026-09-09
- **Scope:** `MAG-P3-C` of
  [`../plans/archive/2026-09-09-android-foundation.md`](../plans/archive/2026-09-09-android-foundation.md):
  `celestina-rs/crates/magnetitad/src/link_wire/mod.rs`, this record
- **Environment:** the live `magnetitad` on the desktop (`10.0.0.134`),
  the S25U at `10.0.0.16` running the Android application of `AND-1-B`
  and `AND-1-C`
- **Artifact:** the daemon, deployed through
  `magnetita/scripts/complete-production.sh` after the change below

## Procedure

The phone side is the application's; its records carry the commands:
[`AND-1-B`](../../../magnetita-android/docs/evidence/2026-09-09-foundation-link.md)
for the link and
[`AND-1-C`](../../../magnetita-android/docs/evidence/2026-09-09-foundation-screens.md)
for the screens. On the desktop:

```sh
busctl --user call org.celestina.Magnetita /org/celestina/Devices1 org.celestina.Devices1 StartPairing
busctl --user --json=short call … ListDevices
busctl --user call … Ring s 4eb6f9984054dd25
journalctl --user -u magnetitad -o cat -f
cd celestina-rs && cargo test -p magnetitad link_wire
```

## Result

- **Pairing, both ways in:** the phone paired through the
  `magnetita://pair` link handed to it by `adb` (`[paired] SM-S938U at
  10.0.0.16:56951 on the own wire`), and later by scanning the QR of a
  fresh `StartPairing` with its own camera. `ListDevices` lists it with
  `paired true`, `connected true` and its battery.
- **Session survival:** the same session stayed connected through 70 s
  with the phone's screen off, 20 s with another application in front, and
  a Wi-Fi off/on of 8 s; no second pairing, no close on the own wire.
- **Find:** `Ring` reached the phone, which rang at alarm volume until its
  stop button; a second `Ring` while ringing changed nothing.
- **Defect found and corrected:** a phone that had forgotten this desktop
  while the desktop still pinned it was admitted as trusted, so its QR
  proof arrived inside a normal session and was logged as unhandled; the
  phone's pairing timed out after the handshake budget. `run_session` now
  answers a `QR_PROOF` from the armed window (`prove_again`): the proof is
  verified against the armed secret and the peer's fingerprint, the reply
  goes back, the session continues. The loopback test
  `a_phone_that_forgot_the_desktop_pairs_again_while_still_pinned` pins
  it. After deployment the S25U scanned a new QR and paired in one step.
- **Tests:** `cargo test -p magnetitad link_wire`: 3 passed. Clippy clean.

## Limits

- A phone that reconnects while its previous session is still open on
  the desktop is dropped as a duplicate until the idle timeout (30 s)
  clears the old one; the application recovers by itself. Preferring the
  newer, freshly authenticated session is a later refinement.
- The six-digit code path is not on the wire yet: the daemon arms QR
  pairing only. It stays for the desktop half, with `MAG-P3-B`.
- `VAL-MAG-11`, the author's own first pairing, is theirs to record.
