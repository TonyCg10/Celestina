# Calcita status

- **Updated:** 2026-10-11
- **Implementation:** CAL-2, editing (annotations and forms), is the
  active checkpoint ([plan](docs/plans/active/2026-10-11-cal-2-editing.md)).
  CAL-2-A (every annotation shown, the annotation model and the
  coordinates) landed as 1.1.0 and CAL-2-B (the edit mode, the three tools
  and saving) as 1.2.0; CAL-2-C (forms shown, text fields and checkboxes
  filled and saved) is implemented and checked offscreen in its session
  worktree, not yet landed. CAL-1 is closed and its
  [plan](docs/plans/archive/2026-10-09-cal-1-foundation.md) archived: CAL-1-A
  (opening, continuous pages, zoom and navigation) as 0.2.0, CAL-1-B
  (search, the outline, selection and copy, links) as 0.3.0, CAL-1-C (the
  dark reading mode, the recents card's menu, the `application/pdf` entry,
  the accessibility pass) as 1.0.0, and the bug unit CAL-1-D (every page
  on opaque paper over the glass) as 1.0.1
- **Author validation:** VAL-CAL-OPEN, VAL-CAL-SEARCH, VAL-CAL-DARK,
  VAL-CAL-ANNOT, VAL-CAL-EDIT and VAL-CAL-FORM pending

## Current checkout truth

- Version 1.2.0 in the checkout, installed under the author's prefix
  (`~/.local/bin/calcita`) by CAL-2-B's landing; filling forms below is in
  the CAL-2-C worktree, not installed. The empty window «Sin documento» offers
  «Abrir…» (the FileChooser portal, filtered to PDF), the recent documents
  and a drop area. A click, Enter or Space on a recent row opens it; its menu (a right
  click, the Menu key or Shift+F10) has «Quitar de recientes».
- Each document opens in a window of its own in the same process: the bar
  (name, the page field «n / N», zoom out/in with the factor, fit width, fit
  page, search, outline, the reading mode, open) over QtPdf's continuous
  pages. The page field takes a page,
  `+n`, `-n`, «inicio» or «fin». The keyboard: PageDown/PageUp,
  Space/Shift+Space, Home/End, Ctrl+G, Ctrl+Plus/Minus/0, Ctrl+1/2, Ctrl+O,
  Ctrl+F, Enter/Shift+Enter and F3/Shift+F3 in a search, F9 for the
  outline, Ctrl+I for the reading mode, Ctrl+C, and Escape to close the search, the outline or a link's
  question.
- The pages are Calcita's own continuous view over QtPdf (the suite's wheel
  scroller and scroll bar; a pointer drag selects text rather than
  scrolling). Each page lies on opaque paper (the suite's `iconSheet`) over
  the glass canvas, since QtPdf renders a page with a clear background;
  the gaps between pages stay glass. Each page's image comes from
  Calcita's page provider (`cpp/page_provider.cpp`), which has QtPdf render
  it with its annotations on one render thread of its own, so another
  program's highlights, notes and drawings show. QtPdf never draws form
  widgets, so a document with a form is shown from an in-memory copy with
  each widget's appearance stamped into its page (CAL-2-C, not landed).
  The window reads the document's annotation model and form with `lopdf`
  on a worker (`calcita_core::annot`). A
  page's address carries the revision of the file shown, renewed when the
  file changed on disk (checked when the window is activated) or was saved
  over, so the image cache never shows another version. Search: Ctrl+F or the bar's button opens a card
  under the bar; the hits are marked, the current one in the selection
  colour and scrolled into view, with «n de N»; Enter/Shift+Enter, F3/Shift+F3
  and the card's arrows walk them round the ends; Escape closes. The outline
  (F9 or the bar's button) is a side card with the bookmarks as an indented
  tree; a click or Enter goes there. A pointer drag selects text on a page
  and Ctrl+C copies it. Internal links move the view; an external link shows
  «Abrir enlace externo» with its host and opens through `xdg-open` only on
  «Abrir» (web and mail links only).
- A second launch, `Open` or a drop opens each PDF in its window, or raises
  the window already holding it; a non-PDF is refused with a notice.
- The dark reading mode (Ctrl+I or the bar's button «Lectura oscura»)
  inverts the pages and turns their hue half a circle through a layer effect
  (a fragment shader baked by `build.rs` with `qsb`); images are inverted
  too. It fades in and out over 200 ms, at once under reduced motion.
- The recent documents (50, `~/.local/share/calcita/recent`) remember the
  page, the zoom and the reading mode each one is reopened at.
- Editing (Ctrl+E or «Editar»; CAL-2-B): the pages render from
  an in-memory copy with the editable annotations hidden and the window
  draws those itself; a bottom toolbar holds «Seleccionar», «Texto»,
  «Resaltar», «Dibujar», the six accent colours, three sizes, undo and
  redo, delete and the two save outcomes. Text boxes (Helvetica in
  WinAnsi, a character outside it refused with a notice), highlights from
  the text selection and freehand strokes are added; any program's three
  kinds are moved, recoloured or deleted, a plain text box retyped. Saving
  appends one incremental update: «Guardar ambas» lands «… (editado).pdf»
  beside and the window moves to it; «Guardar solo la editada» sends the
  original to the Trash and reloads. Leaving or closing with changes asks,
  with «Descartar». The reading mode waits while editing; encrypted and
  unparseable documents refuse edit mode with a notice.
- Forms (CAL-2-C, not landed): a click on a text field or a checkbox
  starts editing on it; while editing they are controls over their
  widgets (Tab and Shift+Tab in the form's order, across pages; Space
  checks), and their values join the same undo, redo and save: a text
  field's `/V` with its appearance redrawn (Helvetica in WinAnsi, its size
  or fitted, its alignment, length and comb), a checkbox's `/V` and `/AS`
  with its own on-state. Radio buttons, lists, signatures and read-only
  fields are shown and say why they are not filled.
- The desktop entry declares `MimeType=application/pdf;`; the author pins
  Calcita as the PDF handler by hand after VAL-CAL-DARK.
- Every control of the bar, the page field, the search card, the outline
  rows and the recent rows carries a Spanish accessible name.

## Known issues

- A form widget's own rotation (`/MK /R`) is not honoured when Calcita
  redraws a filled text field: the text is drawn in the page's user
  space, so on a widget a form turns it reads turned (CAL-2-C, deferred).

## Deferred

- The bar's menu of the design's §4.3 (the recents and «copy path»); the
  recents live on the empty window's card.

## Blockers

None recorded.
