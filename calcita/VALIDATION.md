# Author validation — Calcita

This queue contains no implementation work and never blocks `ROADMAP.md`.

## VAL-CAL-OPEN — Open a real PDF from Siderita and from a drop

- **Status:** pending
- **Related implementation:** CAL-1-A
- **Requires:** the Calcita release binary of CAL-1-A on the real session
  (run from the shared target or installed by `complete-production.sh`)
- **Procedure:** start Calcita; open a real multi-page PDF with «Abrir…»
  (the portal's chooser, Siderita's); drag a second PDF from Siderita onto
  the window, then a non-PDF file; with Calcita running, launch
  `calcita <third.pdf>` from a terminal; in a document window scroll, type a
  page in the field, use the zoom buttons and the keys of STATUS.md; close
  the windows and start Calcita again
- **Pass condition:** the chooser is the portal's and lists PDFs; every PDF
  opens in a window of its own with its pages, the field reads «n / N» while
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
