# CAL-2 — Editing: annotations and forms

- **Opened:** 2026-10-11
- **Plan ID:** cal-2-editing
- **Status:** active
- **Authorization:** the author approved the design in brainstorming on
  2026-10-11 (all four sections) and asked for the program to be
  implemented straight through; the design is
  [the spec](../../specs/2026-10-11-editing-design.md) and the task
  breakdown is [the plan](../../specs/2026-10-11-editing-plan.md)
- **Scope:** calcita
- **Implementation checkpoint:** CAL-2
- **Author-validation checkpoint:** VAL-CAL-ANNOT in
  [VALIDATION.md](../../../VALIDATION.md); CAL-2-B adds VAL-CAL-EDIT and
  CAL-2-C adds VAL-CAL-FORM
- **Predecessor:** CAL-1, the
  [foundation](../archive/2026-10-09-cal-1-foundation.md), closed at 1.0.1

## Hypothesis

Light editing can live in the viewer without rewriting a document: text
boxes, highlights and freehand drawings as standard PDF annotations, and
the forms a PDF already carries, saved as incremental updates that leave
every original byte in place and stay editable in any other program.

## Tangible outcome

A release binary that shows every annotation of every program, adds and
edits the three kinds and fills text fields and checkboxes, saving through
the suite's two outcomes («Guardar ambas», «Guardar solo la editada»).

## Scope

- `CAL-2-A` — every annotation shown (a C++ page provider rendering with
  QtPdf's annotations flag in place of `PdfPageImage`), the annotation model
  and the form fields read with `lopdf` in `calcita-core`, the coordinate
  conversion, the fixtures `annotated.pdf` and `rotated.pdf`.
- `CAL-2-B` — the edit mode, the three tools, undo and redo, saving as an
  incremental update with the two outcomes and «Descartar» on close.
- `CAL-2-C` — forms: shown from a stamped in-memory copy, text fields and
  checkboxes filled and saved.

## Exclusions

- Everything the spec's §1 lists as out of scope: editing the document's
  own text or images, pages added or deleted, certificate signatures,
  embedded fonts, rich text, replies, a flattened export and encrypted
  documents (refused with a notice).
- Any reference to the Celestina shell.

## Build order

1. `CAL-2-A`, then `CAL-2-B` (it renders from the provider and edits the
   model), then `CAL-2-C` (its field edits join `CAL-2-B`'s edit set and
   save).

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary shows
another program's annotations, adds and edits the three kinds, fills a
form and saves both outcomes, each saved file reopening in another viewer.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CAL-2-A | `calcita:` | done | [inventory](../../inventories/2026-10-11-cal-2-editing/CAL-2-A.numstat.tsv) | 40 files, +4202/-74 | Make every annotation visible: a C++ `QQuickAsyncImageProvider` renders each page with `RenderFlag::Annotations` on one render thread that owns its own `QPdfDocument`s, in place of `PdfPageImage` (paper, hits, selection, links and the reading layer unchanged); `calcita-core` reads the annotation model and the form fields with `lopdf =0.45.0` and converts PDF user space to the displayed page; the window reads the model on a reader of its own, one read at a time, the newest waiting; review round 1: `/Rotate` read as PDFium does (whole quarter turns, real numbers truncated), the `/CropBox` clipped to the `/MediaBox`, an inline annotation at its true place in `/Annots`, text strings without their mark and decoded lossily, one inflation budget per read, the smoke's grab after the page image is ready | [evidence](../../evidence/2026-10-11-annotations-shown.md) | VAL-CAL-ANNOT |
| CAL-2-B | `calcita:` | planned | `celestina-rs/crates/calcita-core/src/annot/{write,appearance,edit}.rs`, `calcita/src/editor.rs`, `calcita/qml/edit/`, the page provider (an in-memory copy with the editable annotations hidden; a revision in the page address so the image cache renders again after a save, and after the file is replaced outside Calcita, since today a page already in the image cache keeps the file version it was rendered from) | — | The edit mode, the three tools, undo and redo, saving as an incremental update with the two outcomes | `cargo test`, QML tests, smoke | VAL-CAL-EDIT |
| CAL-2-C | `calcita:` | planned | `celestina-rs/crates/calcita-core/src/annot/form.rs`, `calcita/qml/edit/FormLayer.qml`, `calcita/tests/fixtures/form.pdf`, the page provider (the stamped copy) | — | Forms shown from a stamped copy; text fields and checkboxes filled and saved | `cargo test`, QML tests, smoke | VAL-CAL-FORM |
