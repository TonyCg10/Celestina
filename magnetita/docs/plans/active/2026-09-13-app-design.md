# MAG-D1 — The design of the two applications

- **Opened:** 2026-09-13
- **Plan ID:** app-design
- **Status:** active
- **Authorization:** the author asked on 2026-09-13 to close the whole
  development and leave one checkpoint open, the applications' design
- **Scope:** magnetita
- **Implementation checkpoint:** MAG-D1
- **Author-validation checkpoint:** none; the author's eye is the measure

## Hypothesis

With the protocol, the daemon and both applications complete and in daily
use, what remains is how they look and feel; refined as the author judges
them, one change at a time, nothing behind them changes.

## Tangible outcome

A desktop application the author calls finished.

## Scope

- `MAG-D1-A` — the checkpoint opened: `MAG-M1` archived, the program's
  validations recorded, `AND-6` paired on the Android project.
- `MAG-D1-B` — the desktop application's pages reviewed against the
  suite's visual language.
- `MAG-D1-C` — the author's refinements, as they come.
- `MAG-D1-D` (P-11) — the audit's admission, identity and bounds fixes:
  the session id comes from the pinned fingerprint, handshakes leave the
  accept loop, the QR window survives until a verified proof, shares are
  capped and swept, `commands.json` is written atomically, Notify bodies are
  escaped, mounts and FIFOs move to the runtime directory, and verification
  covers the protocol crates.
- `MAG-D1-E` (P-12) — the audit's liveness, performance, accessibility and
  hygiene fixes: cancel-safe reads, send deadlines, released input, a bounded
  mirror feed, blocking adapters off the async runtime, no discovery churn, an
  accessible conversation row, the dependency trims and the documentation.

The two audit units come from the 2026-09-26 monorepo audit, whose whole
program the author asked for; they are rows of this plan because the project
has one active checkpoint (ruling R-A8 of
[the audit evidence](../../../../docs/evidence/2026-09-26-monorepo-audit.md)).
The findings are in
[the Magnetita audit record](../../../../docs/evidence/2026-09-26-monorepo-audit-magnetita.md).

## Exclusions

- The daemon, the mirror's engine, the wire, except for the recorded audit
  defects `MAG-D1-D` and `MAG-D1-E` fix.

## Build order

1. `MAG-D1-A`, `-B`, then `-C` for as long as the author asks.
2. `MAG-D1-D` after the suite plan's `AUD-1-D` (P-2) and the Rust
   workspace's `RS-H1-A` (P-6), then `MAG-D1-E` stacked on it. The suite
   plan's `AUD-1-E` (P-13) follows `MAG-D1-E`.

## Implementation exit

The author says the application looks finished. The audit units close on
their own rows' automated evidence and Magnetita's
`scripts/complete-production.sh`.

## Waiver

The audit found 37 Magnetita product fixes landed as `maintenance` between
2026-09-01 and 2026-09-26 (MAG-25), so they moved no version and the installed
version under-reports what changed. History is immutable, so those commits
and `docs/version-history.tsv` stay as they are; this paragraph records the
waiver (ruling R-A5). From 2026-09-26 on, a Magnetita product fix lands as
`magnetita-bug`.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| MAG-D1-A | `magnetita:` | done | [inventory](../../inventories/2026-09-13-app-design/MAG-D1-A.numstat.tsv) | 13 files, +239/-89 | The checkpoint opened; `MAG-M1` archived; the program's validations recorded | [record](../../evidence/2026-09-13-program-closed.md) | none |
| MAG-D1-B | `magnetita:` | planned | `qml/` | — | The pages reviewed against the suite's language | record | none |
| MAG-D1-C | `magnetita:` | planned | `qml/`, `src/` | — | The author's refinements | record | none |
| MAG-D1-D | `magnetita:` | planned | `celestina-rs/crates/magnetitad/src/link_wire/mod.rs` (session id, QR window); `celestina-rs/crates/magnetita-link/src/endpoint.rs`; `celestina-rs/crates/magnetitad/src/link_wire/share.rs`; `celestina-rs/crates/magnetitad/src/link_wire/commands.rs`; `celestina-rs/crates/magnetitad/src/notify.rs`; `celestina-rs/crates/magnetitad/src/mount.rs`; `celestina-rs/crates/magnetitad/src/link_wire/mirror.rs`; `celestina-rs/crates/magnetitad/src/link_wire/storage.rs`; `celestina-rs/crates/magnetita-net/src/cert.rs` (`write_private`); `scripts/verify-production.sh` | — | Take the session id from the pinned fingerprint and refuse duplicate ids. Handshake in the spawned task behind a semaphore. Keep the QR window open until a verified proof. Enforce `MAX_PAYLOAD_SIZE`, expire offers, sweep `.part` files. Write `commands.json` atomically. Escape Notify bodies when markup is supported. Move mounts and FIFOs to `runtime_dir`. Delete `write_private`. Map ENOTEMPTY. Add proto, link, mobile and peer to `verify-production.sh`. (P-11: MAG-1, MAG-2, MAG-3, MAG-10, MAG-16, MAG-18, MAG-29; RS-2, RS-3 (magnetita); AND-4 (daemon half)) | `magnetita/scripts/verify-production.sh` now runs `-p magnetita-proto -p magnetita-link -p magnetita-mobile -p magnetita-peer`; new loopback tests (hello id mismatch closes the session; a stalled Initial does not block a second accept; an unpinned connection does not burn the QR; oversized offer refused); Magnetita `complete-production.sh` | none |
| MAG-D1-E | `magnetita:` | planned | `celestina-rs/crates/magnetita-mobile/src/phone.rs`; `celestina-rs/crates/magnetitad/src/link_wire/mod.rs` (session loop, held input, dial loop); `celestina-rs/crates/magnetitad/src/link_wire/mirror.rs`; `celestina-rs/crates/magnetitad/src/devices.rs`; `celestina-rs/crates/magnetitad/src/link_wire/storage.rs`; `celestina-rs/crates/magnetitad/src/main.rs`; `celestina-rs/crates/magnetita-link/src/tls.rs`; `celestina-rs/crates/magnetitad/Cargo.toml` (zbus); `celestina-rs/crates/magnetita-mobile/Cargo.toml` (`uniffi/cli`); `celestina-rs/crates/magnetita-net/`; `src/devices.rs`; `src/controller.rs`; `qml/components/ConversationRow.qml`; `STATUS.md`; `AGENTS.md` | — | Give the phone one reader task. Give the daemon a writer task with deadlines, and enforce revocation, stop and supersede outside sends. Release held input and let releases bypass the governor. Bound the mirror FIFO. Move adapters to blocking threads. Make Forget async. Bound FUSE listings and caches. Handle SIGTERM and unmount. Remove the dial loop and the adb polling. Use a 10 s keep-alive. Keep one bus connection per worker and merge moves. Make `ConversationRow` accessible. Use row-level models. Put zbus on tokio or record approval. Gate `uniffi/cli`. Clean up the KDE leftovers, the docs and the STATUS versioning note; the MAG-25 waiver itself is already recorded in the Magnetita plan (ruling R-A5). (P-12: MAG-4, MAG-5, MAG-6, MAG-7, MAG-11, MAG-13, MAG-14, MAG-15, MAG-17, MAG-20, MAG-21, MAG-22, MAG-24, MAG-25, MAG-26, MAG-27, MAG-30; RS-5, RS-13) | Loopback tests (slow sender does not desync; a send past its deadline closes; keys released on drop; bounded FIFO drops to a key frame); `cargo tree -p magnetitad -i async-io` empty or approval recorded; `cargo tree -p magnetita-mobile -e normal` without `uniffi_bindgen`; `qmllint-cxxqt.sh` for the QML; Magnetita `complete-production.sh` | none |
