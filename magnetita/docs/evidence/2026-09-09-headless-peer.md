# `magnetita-peer`: the own protocol from a shell — MAG-P2-D

- **Date:** 2026-09-09
- **Scope:** `MAG-P2-D` of
  [`../plans/active/2026-09-09-link.md`](../plans/active/2026-09-09-link.md):
  `celestina-rs/crates/magnetita-peer` (`Cargo.toml`, `src/lib.rs`,
  `src/main.rs`) and the crate's registration
- **Environment:** as in [the link record](2026-09-09-link-endpoint.md)
- **Artifact:** `celestina-rs/target/release/magnetita-peer`, built once;
  not installed anywhere

## Procedure

```sh
cd celestina-rs
cargo clippy -p magnetita-peer --all-targets -- -D warnings
cargo test -p magnetita-peer
cargo build --release -p magnetita-peer
```

## Result

- **Exit:** 0
- **Observed:** `test result: ok. 1 passed` in about three seconds; Clippy
  clean. The peer is a library with a thin binary:
  - `Peer::open(dir, name)` loads or creates a certificate under its own
    directory and derives its device id from the certificate's fingerprint,
    so identity and pin are one fact; pins persist in that directory's
    `trust.json`.
  - `pair(uri)` parses the QR text, tries each address with a three-second
    budget, dials with the desktop's fingerprint pinned from the QR, runs
    the phone half of the QR state machine and persists the pin.
  - `connect(addr)`, `report_battery`, `next` (with timeout) and `browse`
    do the rest; `describe` prints one line per envelope.
  - The test pairs the peer with an in-process desktop endpoint whose QR
    lists an unreachable address first, reports a battery the desktop
    decodes, receives a ring, and reopens the directory to find the same
    id and the pin.
  - The binary: `identity`, `pair URI`, `connect IP:PORT [--battery N]
    [--hold SECONDS]`, `browse`; `MAGNETITA_PEER_DIR` overrides the
    directory.

## Limits

- Run only against in-process endpoints; the LAN run from another host
  that the checkpoint exit names needs the daemon deployed with its own
  wire, which the author has not yet requested.
- `browse` shells out to `avahi-browse` unbounded; the peer is a test
  tool, not a daemon.

## Follow-up

The checkpoint's exit: pair the peer with the deployed daemon over the
LAN from another host, Forget it, and watch a migration keep the session.
