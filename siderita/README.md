# Siderita

The suite's file manager: modern, minimal and coherent with the glassmorphic
language, but installable and usable outside the Celestina session. It navigates,
organizes and retrieves local and removable files, and integrates with the rest
of the desktop through freedesktop standards. It does not own editor, player,
panel or dotfiles-manager domains; bounded embedded Grafita and Fluorita
surfaces consume their shared cores without moving those domains into Siderita.

- **Role:** file manager (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** Rust host · Qt Quick/QML via CXX-Qt (minimal) · GPL-3.0-or-later
- **Consumes:** [celestina-rs](../celestina-rs/) domain cores · [celestina-style](../celestina-style/) tokens + glass

## Build / run

Needs Rust and a development Qt visible to CXX-Qt.

```sh
scripts/run.sh                                       # build (release) + install to ~/.local
cargo build --release --locked                       # just the binary (Qt 6.9+ shared for cxx-qt)
cargo build --release --locked --features qt-minimal # Qt bootstrap (CI / no system Qt)
```

`scripts/run.sh` is the one script Siderita needs: it builds in release and
installs the binary, the squircle icon, the desktop entry and the file-chooser
portal into `~/.local`, so the launcher runs the current tree (`--uninstall` to
remove it).

## Layout

| Path | Responsibility |
|---|---|
| `AGENTS.md` | local agent contract for the CXX-Qt/QML boundary, component APIs and verification |
| `src/main.rs`, `src/controller.rs` | Rust host and the CXX-Qt QObject |
| `src/editor.rs` | the embedded Grafita editor's own QObject: document state and Qt marshalling over `grafita-core`, with every read and write on its worker |
| `qml/Main.qml`, `qml/PickerWindow.qml` | application entry surfaces: main window and portal file chooser |
| `qml/views/FolderView.qml`, `qml/views/Sidebar.qml` | composed coordinators; Sidebar is below the ~800-line ceiling, while FolderView is a frozen baseline exception that may only shrink |
| `qml/components/chrome/` | app chrome: top/bottom controls, tabs, headers and Siderita's local floating-glass controls |
| `qml/components/sidebar/` | sidebar rows, saved sections, context menus and info presentation |
| `qml/components/entry/` | file/folder delegates, drag edges, entry badges and `EntryGlyph` (drawn folder vs tinted glyph, and the emblem each place gets) |
| `qml/components/folder/` | list/grid views, shortcuts, actions, operation status and floating content chrome |
| `qml/components/picker/` | portal picker controls; `PickerWindow.qml` retains browsing and selection state |
| `qml/components/` | small dialog/property rows that do not belong to a larger UI region |
| `qml/dialogs/` | dialogs and overlays owned by the folder view |
| `qml/menus/` | context menus and popups |
| `qml/Celestina*.qml`, `qml/Glass*.qml` | canonical `celestina-style` sources consumed as symlinks, never copies |
| `../celestina-style/` | shared theme, glass, icons and font (consumed) |
| `../celestina-rs/crates/siderita-core` | read-only Rust domain |
| `../celestina-rs/crates/siderita-qt` | stable view contract for QML |
| `../celestina-rs/crates/grafita-core` | shared text document/edit/save domain, its line-feed projection and its bounded worker, consumed by the embedded Grafita surface |
| `../celestina-rs/crates/fluorita-core`, `fluorita-engine` *(planned)* | shared media catalogue/playback contracts and lazy decode engine for the future minimal player |
| `scripts/run.sh` | build in release + install to `~/.local` (binary, icon, entry, portal) |
| `scripts/smoke.sh` | static `x: x` scan + an offscreen start that must survive without QML errors |
| `scripts/qml-tests.sh`, `tests/qml/` | interaction tests: `qmltestrunner` presses, moves and sweeps over the real components (what a build or a smoke cannot prove) |

## Grafita interaction

`Space` on editable textual content opens a simple Grafita modal occupying
almost all of Siderita. Images, folders, media and binaries keep the existing
quick-look path; S7 separately replaces the image/video/audio branch with
Fluorita. Text classification comes from `grafita-core` by content, never from a
hardcoded extension or MIME allowlist, and it runs on Grafita's worker so a
large file cannot stall the folder while it is classified.

The modal is a real editor — typing, selection, undo/redo, save, dirty and
conflict state, and a guarded close offering Guardar/Descartar/Cancelar. It is
**driven end to end offscreen, but not yet in a real session**: see Siderita's
S6 checkpoint for exactly what is and is not proven. Double-click and `Enter` on text still
use the default handler; routing them to standalone Grafita waits on that
application existing.

The text widget does not own the text. It shows `grafita-core`'s line-feed
projection and hands its whole content back on every change, and the core
derives the single edit that explains the difference — which is why editing a
CRLF file here does not rewrite its line endings.

## Planned Fluorita interaction

`Space` on an image, video or audio file will open a minimal Fluorita player
inside Siderita; double-click or `Enter` will start the item in the complete
Fluorita library/player. Folder rows continue to consume static thumbnails,
video posters and audio covers from the shared freedesktop cache. A short video
trailer is requested only on demand, and the decode engine remains unloaded
during ordinary browsing.

See [ROADMAP.md](ROADMAP.md) for status, checkpoints, the implemented cut,
measured budget and the design decisions.
