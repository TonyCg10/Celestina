# Siderita

Celestina's local and removable-file manager, including the desktop file chooser
and bounded in-place Grafita/Fluorita actions.

## User contract

- Navigate, search, arrange and operate on local/removable files without silent
  data loss; integrate through freedesktop Trash, MIME, desktop handlers,
  `FileManager1` and the file-chooser portal.
- `Space` performs a bounded in-place task: edit text with embedded Grafita or
  view/play media with embedded Fluorita. Double-click/`Enter` opens the owning
  standalone application. The canonical mapping is
  [the content-activation contract](../docs/contracts/content-activation.md).
- Phone storage exposed by Magnetita is ordinary mounted filesystem content;
  D-Bus contributes device state, identity and actions.
- Siderita is not an IDE, media library, shell, global indexer, archive VFS or
  cloud client. It consumes those product domains instead of absorbing them.

## Architecture

| Area | Responsibility |
|---|---|
| `src/controller/`, `src/controller.rs` | Qt-facing file-manager state, desktop adaptation and bounded workers |
| `src/editor.rs` | Siderita's Qt adapter over `grafita-core` |
| `src/media.rs` | Siderita's minimal-player adapter over Fluorita contracts |
| `src/portal.rs` | `org.freedesktop.impl.portal.FileChooser` backend and request lifecycle |
| `cpp/` | CXX-Qt gaps such as the native model, clipboard and thumbnail provider |
| `qml/Main.qml`, `qml/PickerWindow.qml`, `qml/views/` | Window and view coordinators |
| `qml/components/`, `qml/dialogs/`, `qml/menus/` | Local presentation regions and modal/menu composition |
| `../celestina-rs/crates/siderita-*` | Pure read models, loss-free operations and opaque view tokens |
| `../celestina-rs/crates/grafita-core` | Shared document acceptance, editing and safe-save truth |
| `../celestina-rs/crates/fluorita-*` | Shared media/playback engine and render seam |
| `../celestina-style` | Canonical visual tokens, controls and assets |

## Build and use

Siderita needs Rust and a compatible Qt 6 development environment visible to
CXX-Qt. The canonical production workflow is:

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
scripts/complete-production.sh # canonical agent completion; updates ~/.local
```

Build creates the release artifact once; verify tests that exact artifact
without touching `~/.local`, D-Bus activation or portal configuration; status
reports whether the verification seal still matches the current inputs; deploy
installs the already verified binary, desktop entry, icons and portal files
without recompiling. `scripts/run.sh` remains a human convenience, not the
canonical agent verification entry.

After completion, launch `siderita [PATH]` or use the desktop entry. An already
running process must be reopened. Portal routing remains an explicit
desktop-session choice.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
- [Roadmap history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
