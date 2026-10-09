# Calcita status

- **Updated:** 2026-10-09
- **Implementation:** CAL-1 is open; CAL-1-A (opening, continuous pages,
  zoom and navigation) and CAL-1-B (search, the outline, selection and copy,
  links) are implemented and checked offscreen, and CAL-1-C is next
- **Author validation:** VAL-CAL-OPEN and VAL-CAL-SEARCH pending

## Current checkout truth

- Version 0.1.0, not installed. The empty window «Sin documento» offers
  «Abrir…» (the FileChooser portal, filtered to PDF), the recent documents
  and a drop area.
- Each document opens in a window of its own in the same process: the bar
  (name, the page field «n / N», zoom out/in with the factor, fit width, fit
  page, open) over QtPdf's continuous pages. The page field takes a page,
  `+n`, `-n`, «inicio» or «fin». The keyboard: PageDown/PageUp,
  Space/Shift+Space, Home/End, Ctrl+G, Ctrl+Plus/Minus/0, Ctrl+1/2, Ctrl+O,
  Ctrl+F, Enter/Shift+Enter and F3/Shift+F3 in a search, F9 for the
  outline, Ctrl+C, and Escape to close the search, the outline or a link's
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
- The recent documents (50, `~/.local/share/calcita/recent`) remember the
  page and the zoom each one is reopened at.

## Blockers

None recorded.
