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

CAL-1-C adds the reading mode with Calcita as the `application/pdf` handler.
