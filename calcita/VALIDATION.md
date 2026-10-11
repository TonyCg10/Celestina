# Author validation — Calcita

This queue contains no implementation work and never blocks `ROADMAP.md`.

## VAL-CAL-OPEN — Open a real PDF from Siderita and from a drop

- **Status:** pending
- **Related implementation:** CAL-1-A, CAL-1-D (the page's paper)
- **Requires:** the Calcita release binary of CAL-1-A on the real session
  (run from the shared target or installed by `complete-production.sh`)
- **Procedure:** start Calcita; open a real multi-page PDF with «Abrir…»
  (the portal's chooser, Siderita's); drag a second PDF from Siderita onto
  the window, then a non-PDF file; with Calcita running, launch
  `calcita <third.pdf>` from a terminal; in a document window scroll, type a
  page in the field, use the zoom buttons and the keys of STATUS.md; close
  the windows and start Calcita again
- **Pass condition:** the chooser is the portal's and lists PDFs; every PDF
  opens in a window of its own with its pages, each page opaque paper with
  black text readable over the glass (the gaps between pages glass), the
  field reads «n / N» while
  scrolling, zoom and keys behave; the non-PDF shows «Calcita solo abre
  documentos PDF.»; the second launch opens its document in the running
  process; the recents card lists the documents and reopens each at its page
- **Result:** not run
- **Evidence:** none

## VAL-CAL-SEARCH — Search, the outline, copy and links in a real PDF

- **Status:** pending
- **Related implementation:** CAL-1-B
- **Requires:** the Calcita release binary of CAL-1-B on the real session; a
  real PDF with text, an outline, internal links and a web link
- **Procedure:** open the PDF; press Ctrl+F, type a word that appears several
  times, walk the hits with Enter, Shift+Enter, F3 and Shift+F3, then press
  Escape; press F9, move through the outline with the arrows and Enter, and
  click a bookmark; drag over a line of text, press Ctrl+C and paste into
  another application; click an internal link; click a web link, answer
  «Cancelar», click it again and answer «Abrir»
- **Pass condition:** the hits are marked and the current one stands out in
  the selection colour, comes into view and the counter reads «n de N»;
  Escape closes the card; the outline shows the document's bookmarks
  indented and each one goes to its place; the pasted text is the selected
  text; the internal link goes to its page; the web link opens nothing until
  «Abrir», then opens in the default browser
- **Result:** not run
- **Evidence:** none

## VAL-CAL-DARK — The reading mode, the recents menu and the PDF handler

- **Status:** pending
- **Related implementation:** CAL-1-C
- **Requires:** the Calcita 1.0.0 binary installed by
  `calcita/scripts/complete-production.sh` on the real session; a real PDF
  with black text on white and at least one colour image
- **Procedure:** open the PDF; press Ctrl+I, scroll through a few pages,
  press Ctrl+I again, then use the bar's «Lectura oscura» button; turn
  reduced motion on in the suite's appearance settings and toggle the mode
  again; leave the document in the reading mode, close the window and
  reopen the document from the recents card; in the empty window, right
  click a recent row (and, with the keyboard, focus a row and press
  Shift+F10) and choose «Quitar de recientes»; run
  `update-desktop-database ~/.local/share/applications`, add
  `application/pdf=org.celestina.Calcita.desktop` to
  `~/.config/mimeapps.list` under `[Default Applications]`, then open a PDF
  from Siderita with a double click and with `xdg-open <file.pdf>`; in the
  reading mode, search a word with Ctrl+F and drag over a word to select it
- **Pass condition:** the pages turn dark with light text and colours keep
  their hue (the image is inverted, accepted), the gaps between pages stay
  the window's glass, the bar, the cards and the scroll bar are not
  inverted; the change fades in about 200 ms, and with reduced motion it is
  immediate; the document reopens in the reading mode; the row leaves the
  card and its file is untouched; Siderita and `xdg-open` open the PDF in
  Calcita, not in the browser; in the reading mode the search hit and the
  selection stay clearly visible
- **Result:** not run
- **Evidence:** none

## VAL-CAL-ANNOT — Another program's annotations are visible

- **Status:** pending
- **Related implementation:** CAL-2-A
- **Requires:** the Calcita release binary of CAL-2-A on the real session;
  a real PDF annotated in another program (a highlight, a sticky or text
  note, a freehand drawing; Okular or Firefox's PDF editor write them), on
  a page with `/Rotate` if one is at hand; `tests/fixtures/annotated.pdf`
- **Procedure:** open the annotated PDF and the fixture; look at each
  annotated page at fit width, then zoom in to the ladder's top (Ctrl+Plus)
  and back; scroll through a long annotated document; press Ctrl+I over an
  annotated page and again; search a highlighted word with Ctrl+F; drag
  over highlighted text and press Ctrl+C; click a link on an annotated
  page; open the same file in the other program
- **Pass condition:** every highlight, note and drawing shows where the
  other program shows it, in its colour, sharp at every zoom and on rotated
  pages; pages appear as quickly as before while scrolling; in the reading
  mode the annotations invert with the page; the search hit, the selection
  and the link still work and draw over the annotations
- **Result:** not run
- **Evidence:** none

## VAL-CAL-EDIT — Text boxes, highlights and drawings saved as annotations

- **Status:** pending
- **Related implementation:** CAL-2-B
- **Requires:** the Calcita release binary of CAL-2-B on the real session;
  a real PDF with selectable text (a copy kept elsewhere), one annotated by
  another program (a highlight, a text box, a drawing), and an encrypted
  one if at hand; another viewer (Okular, or a web browser's PDF viewer)
- **Procedure:** open the PDF; press Ctrl+E (and once «Editar» on the bar):
  the bottom toolbar appears and the reading mode, if on, waits. With
  «Texto», click on the page and type a Spanish sentence with accents, the
  euro and «guillemets», then click elsewhere; double-click it and change a
  word; with «Seleccionar» drag it, then drag a corner handle; pick another
  colour while it is selected. With «Resaltar», drag over a line of text,
  and over two lines. With «Dibujar», draw a stroke with the mouse (and a
  stylus if at hand). Pick each size once. Select another program's
  highlight and move it, recolour its drawing, delete its text box with
  Supr. Reopen a file Calcita saved in an earlier session: move one of
  its text boxes, then try to resize it (no handles), recolour or retype
  it and resize it. Undo everything with Ctrl+Z and redo it with
  Ctrl+Shift+Z. Type a
  character outside Spanish (a Greek or Chinese letter) in a text box.
  Save with «Guardar ambas»; edit again and save with «Guardar solo la
  editada»; edit once more and leave with Ctrl+E, answering «Descartar»;
  edit, then close the window and answer «Guardar ambas». Try Ctrl+E on
  the encrypted PDF. Open each saved file in the other viewer; open the
  Trash in Siderita
- **Pass condition:** every tool places its mark where the pointer was, at
  every zoom and on a rotated page; the text looks the same while typing
  and once saved (same font, same lines); the selection shows its frame
  and four handles and moves and resizes smoothly; undo and redo walk
  every step; another program's text box, and one Calcita saved in an
  earlier session, moves at its own size and is resized only once
  recoloured or retyped (then redrawn, never stretched); the outside
  character is refused with a notice and nothing
  else is lost; «Guardar ambas» leaves the original untouched and opens the
  copy «… (editado).pdf» in the same window at the same page and reading
  mode; «Guardar solo la editada» keeps the name, reloads with the edits
  and puts the original in the Trash, restorable from Siderita;
  «Descartar» leaves the file as it was; closing with changes asks first;
  the encrypted PDF refuses edit mode with a notice; in the other viewer
  every text box, highlight and drawing shows where Calcita put it, in its
  colour, and can be selected and edited there as an annotation
- **Result:** not run
- **Evidence:** none

## VAL-CAL-FORM — A real form filled and reopened elsewhere

- **Status:** pending
- **Related implementation:** CAL-2-C
- **Requires:** the Calcita release binary of CAL-2-C on the real session;
  a real fillable PDF (a public administration's form, kept as a copy
  elsewhere) with text fields, a checkbox and ideally a radio group, a
  list, a comb or multiline field and a field repeated on two pages;
  `tests/fixtures/form.pdf`; a web browser's PDF viewer and Okular
- **Procedure:** open the form: look at every field before editing (the
  values and boxes the form draws); click a text field (edit mode begins
  with the keyboard in it), type a Spanish name with accents and the euro,
  press Tab through every field to the end and round again, check a
  checkbox with Space and another with a click, press Shift+Tab; click a
  radio button and a list; fill a field repeated on two pages and look at
  both; type past a field's length; press Ctrl+Z and Ctrl+Shift+Z outside
  a field; save with «Guardar ambas»; reopen the copy in Calcita, then in
  the browser and in Okular; fill again and save with «Guardar solo la
  editada»; fill, then close the window and answer «Descartar»; zoom in and
  out while a field has the keyboard
- **Pass condition:** every field shows as the form draws it, in Calcita
  as in the other viewers, before and after filling; the click starts
  editing on the field tapped; Tab follows the form's order, onto the
  next page too, and the field in view; what is typed shows in the field's
  font and size and nothing is lost when leaving or saving; the radio
  button and the list say they are not filled; the repeated field shows
  the same value on both pages; a field refuses characters past its
  length; undo and redo walk the values; in the browser and Okular the
  saved values show in the fields and can be edited there, checkboxes
  checked with the form's own mark; «Guardar solo la editada» puts the
  original in the Trash; «Descartar» leaves the file as it was
- **Result:** not run
- **Evidence:** none
