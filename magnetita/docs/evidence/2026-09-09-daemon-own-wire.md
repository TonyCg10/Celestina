# The daemon hosts the own wire next to KDE Connect — MAG-P2-C

- **Date:** 2026-09-09
- **Scope:** `MAG-P2-C` of
  [`../plans/archive/2026-09-09-link.md`](../plans/archive/2026-09-09-link.md):
  `celestina-rs/crates/magnetitad/src/link_wire/mod.rs`, the
  `StartPairing` method and `serve` in `devices.rs`, the four lines that
  wire it in `main.rs`, the daemon's manifest, and the architecture
  baseline row for `main.rs`
- **Environment:** as in [the link record](2026-09-09-link-endpoint.md);
  the daemon's own test suite, 86 tests
- **Artifact:** `celestina-rs/target/release/magnetitad` built once from
  this tree; not deployed — the plan reserves the canonical
  `scripts/complete-production.sh` exit for the checkpoint

## Procedure

```sh
cd celestina-rs
cargo clippy -p magnetitad --all-targets -- -D warnings
cargo test -p magnetitad
cargo build --release -p magnetitad
cd .. && sh scripts/check-architecture-contract.sh
```

## Result

- **Exit:** 0
- **Observed:** `test result: ok. 86 passed` (85 before, plus the wire's
  own), twice in a row; Clippy clean; release build clean; the
  architecture contract passes with `main.rs` at 922 lines, five fewer
  than its baseline, which this unit lowers.
  - **One runtime thread.** `link_wire::install` starts one thread that
    owns one tokio runtime; it binds the QUIC endpoint on `0.0.0.0:1760`,
    advertises, accepts, dials what Avahi shows, pairs and runs sessions.
    Nothing async leaks into the rest of the daemon, which stays
    thread-based.
  - **The same registry, unchanged contract.** An own-wire phone appears as
    a `DeviceEntry` with `paired = true` and its fingerprint, is announced
    by the same `Changed` signal, disappears through the same
    `SessionRegistration` drop, and receives the app's `Ring` as a
    `find` envelope. `org.celestina.Devices1` keeps every method; it gains
    `StartPairing`, which arms a two-minute window and returns the QR text
    — additive.
  - **The same Forget.** Pins live in the shared `TrustStore`, so the
    app's `Forget` forgets an own-wire phone exactly as a KDE Connect one;
    the session thread sees the revocation generation on its next tick,
    closes the session and acknowledges, and the barrier is honoured.
  - **The gate is the daemon's too.** An unpinned certificate is admitted
    only while a window is armed, only under the certificate it presents,
    and only until it proves the secret; the test drives a real phone
    endpoint through pairing, a battery report, a ring, a Forget, and the
    refusal that follows it, all on loopback with real QUIC.
- **The old path is untouched:** the KDE Connect threads, ports and
  behaviour are the same bytes; the two wires share the trust store, the
  registry and the revocation barrier and nothing else.

## Limits

- Not run against the live `magnetitad`: the installed daemon still
  carries no own wire until the author requests the production exit. UDP
  1760 was not exercised on the LAN in this unit.
- Only `battery` and `find` cross the wire; the daily set follows in
  `MAG-P4`.
- The foreign one-line change in `main.rs` (announce while connected
  rather than paired) is not this unit's and was left in the worktree,
  outside the commit.

## Follow-up

`MAG-P2-D` provides the peer that exercises this from a shell.
