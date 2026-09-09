# QR pairing from the S25U and six-digit SPAKE2 pairing between headless peers — MAG-P0-E

- **Date:** 2026-09-08
- **Scope:** `MAG-P0-E` of
  [`../plans/active/2026-09-07-own-protocol-spikes.md`](../plans/active/2026-09-07-own-protocol-spikes.md);
  verifies the [pairing discussion](../discussions/2026-09-04-pairing-qr-or-code.md)
- **Environment:** the phone and toolchain of
  [the QUIC phone record](2026-09-08-quic-on-the-phone.md); scratch desktop
  binary `pairspike` (`quinn 0.11`, `rustls`/`ring`, `rcgen`, `qrcode 0.14`,
  `spake2 0.4`) and a throwaway `ScanActivity` (CameraX 1.4.2, ML Kit
  barcode 17.3.0) calling `pair_with_qr` in the `magspike` core through
  UniFFI; UDP 1762 and TCP 1763, inside the range the author's `ufw` admits
- **Artifact:** not applicable

## Procedure

**QR.** The desktop generated a self-signed certificate, a 32-byte random
secret and one URI —
`magnetita://pair?v=1&id=…&fp=<sha256 of its cert>&secret=<hex>&addr=10.0.0.134:1762` —
rendered as a PNG the author scanned off a screen with the phone's camera.
The phone parsed it, dialled the address over QUIC with mutual TLS,
presenting its own fresh self-signed certificate and **accepting only the
server certificate whose SHA-256 the QR carried**; then on one stream it
sent `HMAC(secret, phone_fp ‖ desktop_fp)` and expected
`HMAC(secret, desktop_fp ‖ phone_fp)` back. Each side logged the peer
fingerprint it pinned.

**Code.** The desktop drew a six-digit code and listened on TCP; a second
process on the desktop, standing in for a phone with no camera, joined with
the code typed on its command line. Both ran SPAKE2 (Ed25519 group,
identities `desktop`/`phone`), derived the shared key and exchanged HMAC
confirmations. Run twice: with the right code, and with the code plus one.

```sh
./pairspike qr          # prints the URI, writes pair-qr.png, waits on UDP 1762
adb shell am start -n com.example.magnetita/.ScanActivity
./pairspike code &      # prints the six digits
./pairspike join 282191
./pairspike join 935811 # against a desktop showing 935810
```

## Result

- **Exit:** 0 for every process
- **Observed:**

```
desktop: PAIRED phone fp 041e0b7a0d1f2f7a22fefe4b9a24eb413b6a40d32adeb13168e55cd279dcdbfe from 10.0.0.16:35483 in 10 ms after the handshake began; pinned
phone:   QR scanned: magnetita://pair?v=1&id=0eb21b28ae74d54c&fp=07fe2727c1ed715f…
phone:   PAIRED with desktop 0eb21b28ae74d54c fp 07fe2727c1ed715f… in 43 ms; my fp 041e0b7a0d1f2f7a…

right code: phone: PAIRED by code, shared key e8170d8e7e05b4cc…   desktop: PAIRED by code, shared key e8170d8e7e05b4cc…
wrong code: phone: REJECTED — wrong code                           desktop: REJECTED — wrong code
```

  - One scan, no other input on either side: the phone paired in 43 ms
    from the scan, the desktop pinned the phone's certificate 10 ms after
    the handshake began, and the fingerprints each side logged are the
    other's. The secret never crossed the network; only HMACs bound to both
    certificates did.
  - The typed path pairs two headless processes with the same six digits
    and rejects a code off by one on both sides, with nothing an
    eavesdropper could replay.
  - The author saw no change on the phone because the throwaway screen
    only updated one line of small text above the camera preview; the
    product screen (`MAG-P3`) closes the camera and shows the paired
    device.

## Limits

- The QR was scanned from a screen the author held the phone to; scanning
  from the desktop application's own window is `MAG-P3` work.
- The code path ran between two desktop processes over plain TCP, as the
  discussion asked; in the product it runs inside the QUIC/TLS session and
  the phone types the code. Six decimal digits give one guess in a million
  per attempt, so `MAG-P1` limits attempts.
- The trust store is not persisted by the spike; pinning was observed, not
  stored.

## Follow-up

None for `MAG-P0-E`; the pairing discussion cites this record. Remaining
in `MAG-P0`: `-B`, `-C` and the nest half of `-D`.
