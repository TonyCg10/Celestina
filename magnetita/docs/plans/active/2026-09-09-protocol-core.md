# MAG-P1 — The protocol core, `magnetita-proto`

- **Opened:** 2026-09-09
- **Plan ID:** protocol-core
- **Status:** active
- **Authorization:** the author said "abre MAG-P1 y empieza" on 2026-09-09
  after `MAG-P0` closed with every choice of
  [ADR 0001](../../decisions/0001-own-protocol-and-android-app.md) verified
- **Scope:** magnetita, magnetita-proto
- **Implementation checkpoint:** MAG-P1
- **Author-validation checkpoint:** none

## Hypothesis

One pure crate can own the envelope, the message catalog, capability
negotiation, pairing state machines and every hostile-input bound, and be
the single implementation both ends link — so that interoperability between
the daemon and the phone is by construction and every wire byte is pinned
by a committed vector.

## Tangible outcome

`celestina-rs/crates/magnetita-proto`: no I/O, no time, no sockets. Golden
CBOR vectors for every message, a pairing state machine driven from both
roles, a decoder that refuses every oversized, malformed or out-of-capability
input with a typed reason, and a wire document under `magnetita/docs/`.

## Scope

- `MAG-P1-A` — the envelope `{version, capability, kind, id, body}` in CBOR
  with integer keys; hello and capability negotiation with per-capability
  versions; the bound rule (every peer-chosen string, byte string, list and
  count is limited at decode) with golden vectors and refusal tests.
- `MAG-P1-B` — pairing: the QR payload, the one-time secret, possession
  proofs over both fingerprints, and the six-digit SPAKE2 path, as state
  machines for both roles.
- `MAG-P1-C` — the daily-set catalog (`battery`, `clipboard`,
  `notifications`, `find`, `share`, `media`) and the wire document.
- `MAG-P1-D` — `commands`, `input` and `mirror` messages.
- `MAG-P1-E` — `sms`, `contacts` and `telephony` messages.

## Exclusions

- Sockets, TLS, discovery, time and threads: `magnetita-link` (`MAG-P2`).
- `storage`: `MAG-P7`.
- Any change to `magnetita-core`, `magnetita-net` or the daemon.
- The Android project; it links this crate in `MAG-P3`.

## Build order

1. `MAG-P1-A`, because every later message is an envelope body.
2. `MAG-P1-B`, which needs only the envelope.
3. `MAG-P1-C`, `-D`, `-E` in that order; each adds a capability to the
   negotiation table and vectors to the document.

## Implementation exit

`cargo fmt --check`, Clippy and `cargo test -p magnetita-proto` pass with
every message round-tripping through a committed vector; the architecture
contract records the crate as pure with no adapter dependency; the wire
document lists every capability, kind and bound.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| MAG-P1-A | `magnetita:` | done | [inventory](../../inventories/2026-09-09-protocol-core/MAG-P1-A.numstat.tsv) | 23 files, +1332/-130 | Envelope, hello, negotiation, bounds, golden vectors; `MAG-P0` archived and `MAG-P1` opened | [record](../../evidence/2026-09-09-protocol-envelope-and-hello.md) | None |
| MAG-P1-B | `magnetita:` | done | [inventory](../../inventories/2026-09-09-protocol-core/MAG-P1-B.numstat.tsv) | 4 files, +943/-0 | Pairing state machines, both roles, both paths | [record](../../evidence/2026-09-09-protocol-pairing.md) | None |
| MAG-P1-C | `magnetita:` | done | [inventory](../../inventories/2026-09-09-protocol-core/MAG-P1-C.numstat.tsv) | 15 files, +1479/-3 | Daily-set catalog, the shared codec and bounds, and the wire document | [record](../../evidence/2026-09-09-protocol-daily-catalog.md) | None |
| MAG-P1-D | `magnetita:` | done | [inventory](../../inventories/2026-09-09-protocol-core/MAG-P1-D.numstat.tsv) | 7 files, +921/-3 | `commands`, `input` and `mirror` messages | [record](../../evidence/2026-09-09-protocol-control-and-mirror.md) | None |
| MAG-P1-E | `magnetita:` | done | [inventory](../../inventories/2026-09-09-protocol-core/MAG-P1-E.numstat.tsv) | 7 files, +833/-3 | `sms`, `contacts` and `telephony` messages | [record](../../evidence/2026-09-09-protocol-phone-surface.md) | None |
