# Calcita editing — design

- **Date:** 2026-10-11
- **Status:** approved by the author in brainstorming (all four sections,
  2026-10-11), who asked for the program to be implemented straight through
- **Product:** Calcita, the suite's PDF viewer (1.0.1)
- **Identifiers:** checkpoint `CAL-2`, units `CAL-2-A`, `CAL-2-B`, `CAL-2-C`,
  ledger `calcita/docs/plans/active/2026-10-11-cal-2-editing.md`

## 1. Goal and scope

Light editing of a PDF in Calcita: text boxes, highlights, freehand drawing
and filling the forms a PDF already carries. Every edit is a standard PDF
annotation (or a form field value), stays editable afterwards in Calcita or
any other program, and never rewrites the document's own page content.

Out of scope: editing the document's own text or images, adding or deleting
pages, signatures with certificates, embedded fonts, rich text, comments
with replies, a flattened export, and encrypted documents (refused with a
notice).

## 2. Facts the design rests on (spike, 2026-10-11, throwaway)

| Fact | Evidence |
|---|---|
| `lopdf` 0.45.0 (`default-features = false`) saves an incremental update that leaves every original byte untouched (`cmp` of the prefix) and adds FreeText (with `/AP` using Helvetica), Highlight (`/QuadPoints`, `/AP` with `/BM /Multiply`) and Ink (`/InkList`, `/AP` path); a second edit appends again | Ghostscript strict mode, poppler and an xref/`/Prev` chain checker clean on every output, including a page inside an `/ObjStm` |
| `lopdf` reads annotations back (`/Subtype`, `/Rect`, `/Contents`, `/NM`, `/QuadPoints`, `/InkList`, `/DA`, `/AP`) from Calcita and from other programs, and the form fields (`/FT`, `/T`, `/V`, `/AS`) | the spike's `dump` |
| `lopdf` refuses an incremental save of an encrypted PDF | `check_incremental_save_supported` |
| QtPdf renders annotations only with `QPdfDocumentRenderOptions::RenderFlag::Annotations`; QML `PdfPageImage` exposes only `document` and never renders them, so today's Calcita hides every annotation of other programs | pixel measurements on both fixtures |
| QtPdf never renders form widgets (`/Widget`), with or without the flag | 0 changed pixels; poppler renders the same file |
| Stamping each widget's `/AP /N` into the page content of an in-memory copy makes QtPdf show filled fields | 165 text and 76 checkbox pixels |
| QtPdf selection geometry is in the displayed page's points (top-left, rotated); the PDF user-space conversion for `/Rotate` 0/90/180/270 and a `/MediaBox` offset is exact; values are whole points; rectangles on rotated pages need `normalized()`; PDFium uses `/CropBox` when present | six measured variants |

## 3. Showing pages (CAL-2-A)

- A C++ `QQuickImageProvider` (beside Calcita's clipboard shim) renders a page
  with `QPdfDocument::render` and `RenderFlag::Annotations`, replacing
  `PdfPageImage` in `PageView.qml`. Every annotation of every program shows;
  the reading mode, search hits, selection, links and the paper keep working
  over the new image.
- `calcita-core` gains the annotation model read with `lopdf` off the Qt
  thread: per page, the FreeText, Highlight and Ink annotations (any author)
  as Calcita's own types, every other annotation as "other", and the form
  fields.
- Coordinates: one conversion module (PDF user space ⇄ displayed points)
  using the `/CropBox` (else `/MediaBox`) and `/Rotate`, with the spike's
  formula and tests for the four rotations and an offset box.

## 4. Editing (CAL-2-B)

- An edit mode (`Ctrl+E` and a bar button «Editar»). While editing, the page
  is rendered from an in-memory copy in which the editable annotations
  (FreeText, Highlight, Ink of any author) carry `/F` Hidden, so QtPdf keeps
  drawing the others; Calcita draws the editable ones as QML items.
- A bottom toolbar in the grammar of Fluorita's editor: «Seleccionar»,
  «Texto», «Resaltar», «Dibujar», six theme colours, a size (text S/M/L,
  stroke thin/medium/thick), undo/redo (`Ctrl+Z`, `Ctrl+Shift+Z`), delete
  (`Supr`).
- Text: click to place, type; move by dragging, resize with handles,
  double-click to edit; Helvetica (a standard font, nothing embedded) in
  WinAnsi, which covers Spanish; a character outside it is refused with a
  notice. Highlight: drag over text, using QtPdf's selection boxes. Ink: a
  stroke from press to release.
- Annotations of other programs of the three types can be moved, recoloured
  and deleted; a FreeText's text is rewritten only if it is plain.
- The reading mode is off while editing.

## 5. Forms (CAL-2-C)

- Viewing: a document with an AcroForm is shown from an in-memory copy in
  which each widget's current appearance is stamped into the page content.
- Filling (in edit mode, or by clicking a field): text fields and
  checkboxes become QML controls over their widgets; radio buttons and
  choice lists are shown read-only with a notice (not filled).
- Saving writes `/V`, a regenerated `/AP` for text fields and `/AS` for
  checkboxes (never `NeedAppearances` alone).

## 6. Saving (CAL-2-B, used by C)

- Saving appends one incremental update with `lopdf` to a hidden file
  beside the original, then publishes it with the suite's two outcomes:
  «Guardar ambas» (a copy beside, the default) and «Guardar solo la
  editada» (replace; the original to the Trash through `siderita_ops`, never
  deleted). Closing with changes asks those two plus «Descartar».
- Encrypted documents and documents that `lopdf` cannot parse open as today
  but refuse edit mode with a notice.
- Recents, the reading position and the reading mode survive a save.

## 7. Verification

Crate tests (model, coordinates, writing each annotation type, reading
back, form fill, encrypted refusal) over fixtures; adapter tests; QML tests
of the toolbar, the items and the save question; smoke opening a fixture
with an annotation and asserting it is rendered; ffmpeg-free. The author's
live checks: VAL-CAL-EDIT (each tool on a real PDF, both save outcomes, the
file reopened in another viewer) and VAL-CAL-FORM.

## 8. Decisions

- Annotations, not burned content; editable afterwards everywhere.
- `lopdf` is the one new dependency, pure Rust, compiled in; no system
  package.
- QtPdf stays the renderer; a C++ image provider replaces `PdfPageImage`.
- Forms are shown from a stamped in-memory copy because QtPdf cannot draw
  widgets.
