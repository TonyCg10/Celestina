# Calcita — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
Calcita's constraints; it cannot relax the root or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md), and
  [VALIDATION.md](VALIDATION.md)
- [The design](../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md),
  §1–§4
- [Production artifacts](../docs/contracts/production-artifacts.md)
- [Architecture](../docs/standards/architecture.md)
- [Rust, C++, Qt, and QML](../docs/standards/rust-cpp-qt-qml.md)
- [Verification](../docs/standards/verification.md)
- [Visual design](../celestina-style/DESIGN.md) for visual changes

## Local boundary

- `celestina-rs/crates/calcita-core` is the only owner of the domain: recent
  documents, the zoom ladder, reading positions, the page field's grammar,
  the order a search walks its hits in, the annotation model and form
  fields (read with `lopdf`), the one conversion between PDF user space
  and the displayed page (`annot::coords`), what an edit may change with
  its undo and redo (`annot::edit`), the appearance streams and the
  incremental update that write the edits (`annot::{appearance, write}`),
  and the form: its fields, the copy that stamps the widgets QtPdf cannot
  draw, and what filling a field writes (`annot::form`).
  It has no Qt.
- `src/` owns the Qt adaptation: the controller (open documents, recents,
  the clipboard and `xdg-open` for a confirmed external link),
  the per-window `CalcitaDocument` and `CalcitaEditor`, the landing of a
  saved edit (`saving.rs`, over `atomic_file` and `siderita_ops`), the
  activation adapter and the appearance follower; `cpp/` holds the
  one-function clipboard shim cxx-qt-lib lacks and the page image provider
  cxx-qt cannot subclass. `qml/` presents. QtPdf renders through the page
  provider, which opens its own documents (and the in-memory copy an edit
  is drawn over) on its one render thread; the controller never touches
  page rendering, and QML never decides what a path, a recent entry or an
  edit means.
- One document per window. Activation goes through
  `celestina_core::activation` (`Open` with paths byte-exact); the appearance
  comes from `celestina-settings`. No private copy of either.
- No blocking IO on the Qt thread: file and store work runs on a worker and
  reports back through cxx-qt `Threading` queues.
- Calcita writes a document only when the person saves an edit: one
  incremental update (the original's bytes a prefix), landed as a copy
  beside it or replacing it with the original sent to the Trash, never in
  place. It needs no privilege.
- The Celestina shell is halted by the author's order: never read, reuse or
  reference `celestina/` or `celestina-shell-core`.

## Local verification

- `calcita/scripts/build-production.sh`
- `calcita/scripts/verify-production.sh`
- `calcita/scripts/status-production.sh`

Verification exercises the canonical release artifact without touching the
installed binary. Closing a bug or milestone runs
`calcita/scripts/complete-production.sh`. Reading real documents on the
session belongs in `VALIDATION.md`.
