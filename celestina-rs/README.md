# celestina-rs

The suite's shared Rust workspace: interface-neutral domain cores that carry no
Qt, QML or Niri types — plus, as the one deliberate exception, Magnetita's
network transport and headless daemon. Presentation lives in each app.

- **Role:** shared domain cores + the phone-link backend (part of the [Celestina suite](../ROADMAP.md))
- **Toolchain:** Rust 2021, pinned 1.97.1 (MSRV floor `rust-version = "1.85"`) · `unsafe_code = "forbid"` (workspace lint)
- **Dependencies:** the pure cores carry no third-party crates; the magnetita
  crates earn theirs (serde/serde_json, rustls + ring, rcgen, zbus), each
  justified inline in its `Cargo.toml`

## Crates

| Crate | Responsibility |
|---|---|
| `celestina-core` | shared generations, cooperative cancellation, percent-encoding, XDG paths |
| `siderita-core` | read side: `EntryId` identity, snapshots, bounded scan executor, view projection, watch state |
| `siderita-ops` | write side: loss-free create/rename/copy/move/trash/restore/purge |
| `siderita-qt` | stable opaque view tokens — the contract toward Qt/QML |
| `celestina-dotfiles-core` | plan-only dotfiles change planning (records conflicts, never mutates) |
| `magnetita-core` | KDE Connect protocol domain: packets, identity, pairing state machine, plugin bodies — pure, offline-tested |
| `magnetita-net` | phone-link transport: UDP discovery, TCP+TLS link (TOFU pinning), payload transfer, trust store |
| `magnetitad` | the headless phone-link daemon (binary): device links, sshfs mount, bounded runtime-only MPRIS artwork cache, `org.celestina.Devices1` |

## Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Apps use `path` dependencies during development; a release consumes pinned
versions of these crates.

See [ROADMAP.md](ROADMAP.md) for status and checkpoints.
