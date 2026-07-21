# celestina-rs

Interface-neutral Rust domain cores for the Celestina suite: reusable logic that
carries no Qt, QML or Niri types. Presentation lives in each app.

- **Role:** shared domain cores (part of the [Celestina suite](../ROADMAP.md))
- **Toolchain:** Rust 2021, pinned 1.85.1 · `#![forbid(unsafe_code)]` · no third-party deps

## Crates

| Crate | Responsibility |
|---|---|
| `celestina-core` | shared generations and cooperative cancellation |
| `siderita-core` | identity, snapshots, bounded scan executor, view projection, watch state |
| `siderita-qt` | stable opaque view tokens — the contract toward Qt/QML |
| `celestina-dotfiles-core` | read-only dotfiles change planning (no mutation yet) |

## Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Apps use `path` dependencies during development; a release consumes pinned
versions of these crates.

See [ROADMAP.md](ROADMAP.md) for status and checkpoints.
