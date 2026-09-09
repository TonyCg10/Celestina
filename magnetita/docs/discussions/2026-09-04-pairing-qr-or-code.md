# Pairing: QR code or typed code as the primary path

- **Opened:** 2026-09-04
- **Status:** applied
- **Question:** does the first pairing scan a QR code shown by the desktop, or
  compare a short code shown on both sides?

## Context

Pairing is the one moment trust is created; everything afterwards is a
pinned fingerprint. The KDE Connect path shows the same code on both screens
and asks the human to compare it; `MAG-R1` pairs `adb` on six digits the
author reads off the phone. The author has an open QR pairing question from
`MAG-R2`. The choice decides which Android permissions and libraries the app
needs on day one (camera, barcode scanning) and what the desktop app must
render.

## Strongest case

A QR code carries more than a human can type: the desktop's device id, its
certificate fingerprint, every address it listens on, and a 256-bit one-time
secret. The phone scans it, dials the address directly (no discovery race),
and both sides prove possession of the secret over the encrypted channel
with an HMAC over both fingerprints. There is nothing to compare, nothing to
mistype, and no window in which a device on the LAN can substitute itself,
because the fingerprint arrived out of band. It is also the pairing shape
Android's own wireless debugging uses, so the author already knows it.

## Counter-case

It needs a camera and a barcode library on the phone (CameraX + ML Kit, or
ZXing) and a QR renderer on the desktop, and it does not work for pairing
from the phone toward a headless desktop or for a second desktop application
without a window. A short typed code — six digits, turned into a shared key
with a PAKE such as SPAKE2 so an eavesdropper learns nothing — needs no
hardware at all and is symmetric.

## Alternatives

- QR primary, six-digit SPAKE2 fallback. Both paths end in the same pin.
- Six-digit SPAKE2 only. One path to test; slower for the human.
- Compare-and-confirm (KDE Connect style) without a PAKE. Simplest and the
  weakest: a wrong human answer pins an attacker.

## Falsifiers and evidence needed

`MAG-P0-E`: the QR path pairs the S25U with the desktop nest in one scan
with no other input, and the typed path pairs a headless peer simulator with
the daemon. If the author finds the camera step slower than reading six
digits, the typed path becomes primary and the QR path the fallback.

## Conclusion

**QR primary, six-digit SPAKE2 fallback**, concluded by the author on
2026-09-04. Both paths end in the same pinned fingerprint, so the fallback
costs one state machine, not a second trust model; it exists for a headless
peer and for a desktop without a window. `MAG-P0-E` verifies each path pairs
once with no other input. Applied in
[ADR 0001](../decisions/0001-own-protocol-and-android-app.md) §5 and
`MAG-P1-B`. Verified on 2026-09-08 — see
[the pairing record](../evidence/2026-09-08-qr-and-code-pairing.md): one
scan paired the S25U with the desktop in 43 ms with the desktop certificate
pinned from the QR and both sides proving the secret over the encrypted
channel; the six-digit SPAKE2 path paired two headless processes and
rejected a code off by one on both sides. The falsifier (the camera slower
than reading six digits) was not met; the author scanned once.
