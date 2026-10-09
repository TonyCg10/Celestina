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

The later CAL-1 units add theirs: search and the outline (CAL-1-B), and the
reading mode with Calcita as the `application/pdf` handler (CAL-1-C).
