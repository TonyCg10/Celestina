# Siderita

The suite's file manager: modern, minimal and coherent with the glassmorphic
language, but installable and usable outside the Celestina session. It navigates,
organizes and retrieves local and removable files, and integrates with the rest
of the desktop through freedesktop standards — no editor, viewer, player, panel
or dotfiles manager inside it.

- **Role:** file manager (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** Rust host · Qt Quick/QML via CXX-Qt (minimal) · GPL-3.0-or-later
- **Consumes:** [celestina-rs](../celestina-rs/) domain cores · [celestina-style](../celestina-style/) tokens + glass

## Build / run

Needs Rust and a development Qt visible to CXX-Qt.

```sh
scripts/run-i1.sh                                    # build + run the I1 host on Wayland
cargo build --release --locked                       # with a shared Qt visible to cxx-qt
cargo build --release --locked --features qt-minimal # Qt bootstrap (CI / no system Qt)
scripts/measure-i1.sh target/release/siderita-i1     # ELF inventory + process metrics
desktop-file-validate siderita-adma.desktop
```

## Layout

| Path | Responsibility |
|---|---|
| `src/main.rs`, `src/controller.rs` | Rust host and the CXX-Qt QObject |
| `qml/i1/MainI1.qml` | the Iteration-1 UI (Siderita-specific) |
| `../celestina-style/` | shared theme, glass and icons (consumed) |
| `../celestina-rs/crates/siderita-core` | read-only Rust domain |
| `../celestina-rs/crates/siderita-qt` | stable view contract for QML |
| `scripts/` | run and measurement scripts |

See [ROADMAP.md](ROADMAP.md) for status, checkpoints, the implemented cut,
measured budget and the design decisions.
