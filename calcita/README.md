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
  selection), Escape (close the search, the outline or a link's question),
  Ctrl+E (edit), and while editing Ctrl+Z / Ctrl+Shift+Z (undo, redo),
  Supr (delete the selection) and Enter (edit a selected text box);
  on a recent row of the empty window, Enter or Space opens it and the Menu
  key or Shift+F10 opens its menu («Quitar de recientes»).
- The `application/pdf` handler once the author pins it in `mimeapps.list`.
- Every annotation a document carries is shown (another program's
  highlights, notes and drawings). Ctrl+E or «Editar» adds text boxes,
  highlights over selected text and freehand drawings, and moves,
  recolours or deletes those of any program, with undo and redo; each is a
  standard PDF annotation written as an incremental update (the original's
  bytes untouched), saved with «Guardar ambas» (a copy beside) or «Guardar
  solo la editada» (the original to the Trash). Filling forms is being
  built (CAL-2-C). No signing or printing.

Opening, continuous pages, zoom, the page field, the drop, `Open` and the
recents are in place (CAL-1-A), and so are search, the outline, selection
and copy, and links, an external one only after a confirmation (CAL-1-B),
and the dark reading mode, the recents' menu and the `application/pdf`
entry (CAL-1-C, 1.0). See the
[design](../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md).
Pages are drawn with their annotations, and the annotation model is read
(CAL-2-A); the edit mode, its three tools and saving follow (CAL-2-B), as
the [editing design](docs/specs/2026-10-11-editing-design.md) says.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/calcita-core` | Recents, zoom, reading positions, page input, search order; the annotation model and form fields read with `lopdf`, the conversion between PDF user space and the displayed page, the edits with undo and redo, their appearance streams and their writing as an incremental update (`annot`); no Qt |
| `src/` | The CXX-Qt controller (open documents, recents, clipboard, confirmed external links), the per-window `CalcitaDocument` (the annotation model on a worker, the revision of the file shown), the per-window `CalcitaEditor` (the edit session, its undo stack and its save worker), the save's landing beside or instead of the original (`saving.rs`), the activation adapter, the appearance follower, the page provider's seam (`pages.rs`) |
| `cpp/` | The clipboard shim cxx-qt-lib lacks (`QClipboard` on the Qt thread) and the page image provider (`image://calcitapage/<page>/<revision>/<key>`, QtPdf with its annotations flag, one render thread owning its own documents, and the in-memory copy an edit is drawn over) |
| `qml/` | The empty window, one `DocumentWindow` per document: its bar, its own continuous `PageView` over QtPdf, the search card, the outline card, the link confirmation, the reading mode's layer effect (`shaders/reading.frag`, baked by `build.rs` with Qt's `qsb`), and editing in `qml/edit/` (the bottom toolbar, each page's annotation layer and its items, the save question) |
| `tests/` | QML tests over stand-ins and the fixtures `three-pages.pdf`, `outline.pdf`, `many-pages.pdf`, `annotated.pdf` and `rotated.pdf` (`scripts/make-fixture-pdf.py`) |
| `docs/specs/` | The CAL-2 editing design and its task plan |
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
