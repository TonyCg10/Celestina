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
scripts/run.sh                                       # build (release) + install to ~/.local
cargo build --release --locked                       # just the binary (shared Qt for cxx-qt)
cargo build --release --locked --features qt-minimal # Qt bootstrap (CI / no system Qt)
```

`scripts/run.sh` is the one script Siderita needs: it builds in release and
installs the binary, the squircle icon, the desktop entry and the file-chooser
portal into `~/.local`, so the launcher runs the current tree (`--uninstall` to
remove it).

## Layout

| Path | Responsibility |
|---|---|
| `src/main.rs`, `src/controller.rs` | Rust host and the CXX-Qt QObject |
| `qml/Main.qml`, `qml/PickerWindow.qml` | application entry surfaces: main window and portal file chooser |
| `qml/views/` | composed app views (`FolderView`, `Sidebar`) |
| `qml/components/` | Siderita presentation pieces: headers, delegates, pills, rows and sidebar info |
| `qml/dialogs/` | dialogs and overlays owned by the folder view |
| `qml/menus/` | context menus and popups |
| `qml/Celestina*.qml`, `qml/Glass*.qml` | canonical `celestina-style` sources consumed as symlinks, never copies |
| `../celestina-style/` | shared theme, glass, icons and font (consumed) |
| `../celestina-rs/crates/siderita-core` | read-only Rust domain |
| `../celestina-rs/crates/siderita-qt` | stable view contract for QML |
| `scripts/run.sh` | build in release + install to `~/.local` (binary, icon, entry, portal) |

See [ROADMAP.md](ROADMAP.md) for status, checkpoints, the implemented cut,
measured budget and the design decisions.
