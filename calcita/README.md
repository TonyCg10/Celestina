# Calcita

Celestina's PDF viewer: one document per window, read in the suite's glass
grammar instead of the web browser.

## User contract

- Opens a PDF from the launcher, from Siderita's «Abrir en», from a drop or
  from a second launch, which reaches the running process.
- Continuous pages, zoom (fit width, fit page, free), go to a page, search
  with the hits marked, the outline, text selection and copy, internal links,
  a dark reading mode and the recent documents.
- The keyboard: PageDown/PageUp, Space/Shift+Space, Home/End, Ctrl+G (go
  to), Ctrl+Plus/Minus/0, Ctrl+1 (fit width), Ctrl+2 (fit page), Ctrl+O,
  Ctrl+F (search), Enter/Shift+Enter and F3/Shift+F3 (next and previous
  hit), F9 (outline), Ctrl+I (the dark reading mode), Ctrl+C (copy the
  selection), Escape (close the search, the outline or a link's question);
  on a recent row of the empty window, Enter or Space opens it and the Menu
  key or Shift+F10 opens its menu («Quitar de recientes»).
- The `application/pdf` handler once the author pins it in `mimeapps.list`.
- No annotation, signing, forms or printing.

Opening, continuous pages, zoom, the page field, the drop, `Open` and the
recents are in place (CAL-1-A), and so are search, the outline, selection
and copy, and links, an external one only after a confirmation (CAL-1-B),
and the dark reading mode, the recents' menu and the `application/pdf`
entry (CAL-1-C, 1.0). See the
[design](../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md).

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/calcita-core` | Recents, zoom, reading positions, page input; no Qt |
| `src/` | The CXX-Qt controller (open documents, recents, clipboard, confirmed external links), the per-window `CalcitaDocument`, the activation adapter, the appearance follower |
| `cpp/` | The clipboard shim cxx-qt-lib lacks (`QClipboard` on the Qt thread) |
| `qml/` | The empty window, one `DocumentWindow` per document: its bar, its own continuous `PageView` over QtPdf, the search card, the outline card, the link confirmation and the reading mode's layer effect (`shaders/reading.frag`, baked by `build.rs` with Qt's `qsb`) |
| `tests/` | QML tests over stand-ins and the fixtures `three-pages.pdf`, `outline.pdf` and `many-pages.pdf` (`scripts/make-fixture-pdf.py`) |
| `../celestina-style` | Canonical visual tokens, controls and assets, linked |
| `org.celestina.Calcita.desktop` | Desktop discovery |

## Build and use

Calcita needs Rust and a Qt 6 development environment visible to CXX-Qt
(with QtPdf, the `QtQuick.Pdf` module). The canonical production workflow is:

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
scripts/complete-production.sh # canonical agent completion; updates ~/.local
```

After completion, launch `calcita` or use the desktop entry.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
