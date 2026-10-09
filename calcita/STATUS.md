# Calcita status

- **Updated:** 2026-10-09
- **Implementation:** CAL-1 is open; CAL-1-A (opening, continuous pages,
  zoom and navigation) is implemented and checked offscreen, and CAL-1-B is
  next
- **Author validation:** VAL-CAL-OPEN pending

## Current checkout truth

- Version 0.1.0, not installed. The empty window «Sin documento» offers
  «Abrir…» (the FileChooser portal, filtered to PDF), the recent documents
  and a drop area.
- Each document opens in a window of its own in the same process: the bar
  (name, the page field «n / N», zoom out/in with the factor, fit width, fit
  page, open) over QtPdf's continuous pages. The page field takes a page,
  `+n`, `-n`, «inicio» or «fin». The keyboard: PageDown/PageUp,
  Space/Shift+Space, Home/End, Ctrl+G, Ctrl+Plus/Minus/0, Ctrl+1/2, Ctrl+O.
- A second launch, `Open` or a drop opens each PDF in its window, or raises
  the window already holding it; a non-PDF is refused with a notice.
- The recent documents (50, `~/.local/share/calcita/recent`) remember the
  page and the zoom each one is reopened at.

## Blockers

None recorded.
