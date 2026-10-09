# Calcita status

- **Updated:** 2026-10-09
- **Implementation:** CAL-1's three units are implemented and checked
  offscreen: CAL-1-A (opening, continuous pages, zoom and navigation),
  CAL-1-B (search, the outline, selection and copy, links) and CAL-1-C (the
  dark reading mode, the recents card's menu, the `application/pdf` entry,
  the accessibility pass); the CAL-1-C landing releases 1.0.0 and closes the
  foundation
- **Author validation:** VAL-CAL-OPEN, VAL-CAL-SEARCH and VAL-CAL-DARK
  pending

## Current checkout truth

- Version 0.2.0 in the checkout (the CAL-1-C landing sets 1.0.0), not
  installed. The empty window «Sin documento» offers «Abrir…» (the
  FileChooser portal, filtered to PDF), the recent documents and a drop
  area. A click, Enter or Space on a recent row opens it; its menu (a right
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
  scrolling). Search: Ctrl+F or the bar's button opens a card
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
- The desktop entry declares `MimeType=application/pdf;`; the author pins
  Calcita as the PDF handler by hand after VAL-CAL-DARK.
- Every control of the bar, the page field, the search card, the outline
  rows and the recent rows carries a Spanish accessible name.

## Deferred

- The bar's menu of the design's §4.3 (the recents and «copy path»); the
  recents live on the empty window's card.

## Blockers

None recorded.
