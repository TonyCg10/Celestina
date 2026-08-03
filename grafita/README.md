# Grafita

Celestina's general text editor: one loss-free document core with a complete
standalone application and a bounded editor embedded in Siderita.

## User contract

- Open textual content by inspecting bytes and encoding, regardless of name,
  extension or desktop MIME classification.
- Preserve supported encodings, original line terminators and reproducible file
  metadata; refuse a save that cannot meet the loss-free contract.
- In Siderita, `Space` edits text in place and double-click/`Enter` launches the
  standalone app. The canonical mapping is the
  [content-activation contract](../docs/contracts/content-activation.md).
- Grafita is an editor, not an IDE: it has no project tree, build runner,
  debugger, terminal, LSP or plugin platform.

Supported editable encodings are UTF-8, UTF-8 with BOM and UTF-16 LE/BE with
BOM. Unknown or malformed byte streams are reported honestly instead of being
silently reinterpreted.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/grafita-core` | Text probing, byte/newline-preserving document, edit history, search/highlight, tabs/session outcomes and safe file IO; no Qt |
| `src/` | Standalone CXX-Qt adapter, activation, bounded workers and desktop integration |
| `qml/` | Standalone window, document tabs and editor presentation |
| `../siderita/src/editor.rs` | Thin Siderita adapter over the same core |
| `../siderita/qml/dialogs/` | Bounded embedded editing surface |
| `../celestina-style` | Canonical visual tokens, controls and assets |
| `org.celestina.Grafita.desktop` | Desktop discovery and `Abrir con`; never text classification |

The two hosts do not import each other's QML. Qt text is a projection of the
core document, so untouched CRLF/mixed-newline content does not get normalized
by a widget. Open and save results are generation/revision stamped before they
are applied on the GUI thread.

## Build and use

Grafita needs Rust and a compatible Qt 6 development environment visible to
CXX-Qt. The canonical production workflow is:

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
scripts/complete-production.sh # canonical agent completion; updates ~/.local
```

Build creates the release artifact once; verify tests that exact artifact
without replacing the installed binary; status reports whether the verification
seal still matches the current inputs; deploy installs the already verified
binary, desktop entry and icons without recompiling. `scripts/run.sh` remains a
human convenience, not the canonical agent verification entry.

After completion, launch `grafita [PATH]` or use the desktop
entry.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
- [Roadmap history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
