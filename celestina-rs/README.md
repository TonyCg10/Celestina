# celestina-rs

The suite's shared Rust workspace: interface-neutral domain cores that carry no
Qt, QML or Niri types — plus, as the one deliberate exception, Magnetita's
network transport and headless daemon. Presentation lives in each app.

- **Role:** shared domain cores + the phone-link backend (part of the [Celestina suite](../ROADMAP.md))
- **Toolchain:** Rust 2021, pinned 1.97.1 (MSRV floor `rust-version = "1.85"`) · `unsafe_code = "forbid"` (workspace lint)
- **Dependencies:** the five non-Magnetita cores carry no third-party crates;
  `magnetita-core` remains UI/IO-free but uses serde for its wire domain, and the
  transport/daemon earn rustls + ring, rcgen and zbus. Each dependency is
  justified inline in its `Cargo.toml`

## Crates

| Crate | Responsibility |
|---|---|
| `celestina-core` | shared generations, cooperative cancellation, atomic durable state-file replacement, percent-encoding, XDG paths |
| `siderita-core` | read side: `EntryId` identity, snapshots, bounded scan executor, view projection, watch state |
| `siderita-ops` | write side: loss-free create/rename/copy/move/trash/restore/purge |
| `siderita-qt` | stable opaque view tokens — the contract toward Qt/QML |
| `celestina-dotfiles-core` | plan-only dotfiles change planning (records conflicts, never mutates) |
| `magnetita-core` | KDE Connect protocol domain: packets, identity, v8 timestamp-aware pairing with typed invalidity, plugin bodies, typed MPRIS actions and playback-progress classification — pure, offline-tested |
| `magnetita-net` | phone-link transport: UDP discovery, identity-bound TCP+TLS, stable TOFU certificate pin versus temporary SPKI+timestamp code, bounded certificate-bound payload transfer, atomic trust store |
| `magnetitad` | headless phone-link daemon: explicit local pairing acceptance and durable revocation, admitted/expiring unknown links, sshfs mount, lossless incoming files, a bounded/cancelable joined desktop-MPRIS worker, bounded payload workers whose stale results cannot publish after `Forget`, temporary `verificationKey`, `org.celestina.Devices1` |

Workspace-specific dependency, safety and verification rules are recorded in
[`AGENTS.md`](AGENTS.md), layered on the monorepo contract.

## Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Apps use `path` dependencies during development; a release consumes pinned
versions of these crates.

See [ROADMAP.md](ROADMAP.md) for status and checkpoints.
