# `magnetita-link`: endpoint, pins, handshake deadline, streams, on loopback — MAG-P2-A

- **Date:** 2026-09-09
- **Scope:** `MAG-P2-A` of
  [`../plans/archive/2026-09-09-link.md`](../plans/archive/2026-09-09-link.md):
  `celestina-rs/crates/magnetita-link` (`Cargo.toml`, `src/lib.rs`,
  `src/tls.rs`, `src/endpoint.rs`, `src/session.rs`, `src/trust.rs`,
  `src/backoff.rs`, `src/error.rs`), the workspace manifest and lock, the
  crate's registration, and the opening of `MAG-P2` with `MAG-P1` archived
- **Environment:** `celestina-rs` on the pinned `1.97.1` toolchain;
  `quinn 0.11` (`rustls-ring`, `runtime-tokio`), `rustls 0.23`, `tokio 1`;
  the device certificate and trust store from `magnetita-net`; loopback only
- **Artifact:** not applicable; the daemon links the crate in `MAG-P2-C`

## Procedure

```sh
cd celestina-rs
cargo fmt -p magnetita-link -- --check
cargo clippy -p magnetita-link --all-targets -- -D warnings
cargo test -p magnetita-link   # run three times for flakiness
cargo check --workspace
cd .. && scripts/check-documentation-contract.sh && python3 scripts/check-language-contract.py && sh scripts/check-architecture-contract.sh
```

## Result

- **Exit:** 0 for every command
- **Observed:** `test result: ok. 9 passed; 0 failed` three times in a row;
  Clippy clean; workspace check clean; the three contracts `OK`. What the
  tests pin, all on loopback with real certificates and real QUIC:
  - two pinned peers connect, exchange hellos both ways, send envelopes on
    the control stream in both directions, a datagram, and a transfer
    stream named by its id;
  - an unpinned certificate is closed by the server before any envelope,
    and the dial fails on the client;
  - a pairing admits exactly the certificate the QR names, runs the
    `magnetita-proto` QR state machines over the link, and both sides pin
    each other — the spike of `MAG-P0-E` as product code;
  - the client rebinds to a new UDP socket mid-session and the server keeps
    the session, now seeing the new address — connection migration;
  - a control-stream frame whose header lies about its size is refused by
    the header, before a byte is read;
  - the handshake deadline is absolute: a step started after the budget is
    spent times out at once.
- **Owner and boundary:** the certificate and the trust store are
  `magnetita-net`'s, imported, not copied — the same file, the same
  colon-hex fingerprints, so a phone paired on either wire is one entry;
  they move to this crate when `MAG-P7` removes the KDE Connect wire. The
  TLS verifiers accept any certificate and the endpoint's gate is the only
  door to a session, so a session cannot exist for an unchecked
  certificate. Transport: initial MTU 1200 without probing, keep-alive 1 s,
  idle 30 s, the three numbers `MAG-P0` measured.

## Limits

- Loopback: no Wi-Fi, no Avahi, no daemon. `MAG-P2-B` to `-D` add those.
- The reconnection loop is not here, only its schedule (`Backoff`); the
  daemon runs it on its one owned thread.
- The six-digit code path is not exercised over the link in this unit;
  the proto crate proves it and the wire it uses is the same control
  stream as the QR path.

## Follow-up

`MAG-P2-B`, discovery through Avahi.
