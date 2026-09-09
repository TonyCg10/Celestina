# `magnetita-proto`: pairing by QR and by six-digit code, both roles — MAG-P1-B

- **Date:** 2026-09-09
- **Scope:** `MAG-P1-B` of
  [`../plans/archive/2026-09-09-protocol-core.md`](../plans/archive/2026-09-09-protocol-core.md):
  `celestina-rs/crates/magnetita-proto/src/pair.rs`, the `PAIRING`
  capability id, and the crate's `ring`, `spake2` and `rand_core`
  dependencies
- **Environment:** as in
  [the envelope record](2026-09-09-protocol-envelope-and-hello.md);
  `spake2 0.4.0` without its `getrandom` feature — the caller supplies the
  RNG, tests supply a deterministic one — and `ring 0.17` for HMAC-SHA256
  and SHA-256, the backend `magnetita-net` already pins
- **Artifact:** not applicable

## Procedure

```sh
cd celestina-rs
cargo fmt -p magnetita-proto -- --check
cargo clippy -p magnetita-proto --all-targets -- -D warnings
cargo test -p magnetita-proto
cargo check --workspace
cd .. && scripts/check-documentation-contract.sh && python3 scripts/check-language-contract.py && sh scripts/check-architecture-contract.sh
```

## Result

- **Exit:** 0 for every command
- **Observed:** `test result: ok. 33 passed; 0 failed` (13 for pairing);
  Clippy clean; workspace check clean; the three contracts `OK`. What the
  tests pin:
  - the QR payload round-trips through its `magnetita://pair?` URI and
    refuses a foreign scheme, a future version, a short fingerprint, a
    missing secret and an id with anything but alphanumerics;
  - QR pairing pins both fingerprints when the phone saw the QR and
    presents the certificate the desktop saw; an impostor with the QR but
    another certificate, a phone that reached another desktop, and the
    right phone with the wrong secret are each refused with `WrongProof`;
    every state machine is single-use and refuses the other role's steps;
  - a proof of the wrong size is refused at decode, before any HMAC;
  - code pairing pins both ways with the same six digits, refuses a code
    off by one on both sides, refuses a confirmation from a peer that
    believes it is talking to another certificate, is single-use and
    ordered, refuses anything but six ASCII digits (full-width digits
    included), and refuses a garbage exchange;
  - `fingerprint` is SHA-256 of the DER.
- **What the spikes proved is now the rule:** the wire shape and the
  proof construction are the ones `MAG-P0-E` paired the S25U with, with
  two hardenings the spike skipped — the proofs and confirmations bind
  both fingerprints under a domain label, and every state machine ends on
  its first failure.

## Limits

- SPAKE2 messages are random per session, so there is no golden vector
  for them; the tests run both roles in-process with a deterministic RNG
  and pin the outcome, not the bytes. The `{0: bytes}` framing is pinned
  through the bound tests.
- Attempt limiting across connections (how many wrong codes a desktop
  tolerates before it stops showing one) is the link's policy, `MAG-P2`.
- No consumer yet; the first is `magnetita-link`.

## Follow-up

`MAG-P1-C`, the daily-set catalog and the wire document.
