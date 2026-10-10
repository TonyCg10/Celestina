# CAL-1 — Foundation, reading and 1.0

- **Opened:** 2026-10-09
- **Plan ID:** cal-1-foundation
- **Status:** active
- **Authorization:** the author approved the design in brainstorming on
  2026-10-09 and asked for the program to be implemented; the design is
  [the spec](../../../../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md)
  and the task breakdown is
  [the plan](../../../../docs/superpowers/plans/2026-10-09-reading-and-capture.md)
- **Scope:** calcita
- **Implementation checkpoint:** CAL-1
- **Author-validation checkpoint:** none yet; each unit adds its entry to
  [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

A PDF viewer built on QtPdf and the suite's conventions can take reading out
of the web browser: one window per document, opened from Siderita, a drop or
a second launch, with the reading tools a person reaches for every day.

## Tangible outcome

A release binary installed under the author's prefix, at 1.0.0, that opens a
PDF with continuous pages, zoom, search, the outline, selection, links, a
reading mode and recents.

## Scope

The skeleton is not a row of this ledger: it lands with the registration, in
suite unit EXT-1-A of
[the suite plan](../../../../docs/plans/archive/2026-10-09-reading-and-capture.md).
It is the transparent window over the backdrop, the appearance follower, the
claim-first activation adapter, the `CalcitaController` singleton, the empty
state «Sin documento» with «Abrir…», the scripts, the QML test harness and
the document set; its record is
[the skeleton evidence](../../evidence/2026-10-09-skeleton.md).

- `CAL-1-A` — open a document: pages, zoom, go to, the drop and `Open`.
- `CAL-1-B` — search, the outline, text selection and links.
- `CAL-1-C` — reading mode, recents, the `application/pdf` entry, 1.0.0.
- `CAL-1-D` — bug: the pages had no paper, so their ink sat on the glass.

## Exclusions

- Everything the design's §1 lists as out of scope for Calcita.
- Any reference to the Celestina shell.

## Build order

1. `CAL-1-A`, then `CAL-1-B`, then `CAL-1-C`.
2. `CAL-1-D` after the author's report on the deployed 1.0.0.

## Implementation exit

`scripts/complete-production.sh` succeeds at 1.0.0 and the installed binary
reads a PDF with every tool of the scope.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CAL-1-A | `calcita:` | done | [inventory](../../inventories/2026-10-09-cal-1-foundation/CAL-1-A.numstat.tsv) | 43 files, +3385/-108 | Open a document with its pages, zoom and navigation; the drop and `Open` reach it (deferred: the page view keeps QtPdf's own scroll bar, not the suite's scroller) | [evidence](../../evidence/2026-10-09-open-and-pages.md) | VAL-CAL-OPEN |
| CAL-1-B | `calcita:` | done | [inventory](../../inventories/2026-10-09-cal-1-foundation/CAL-1-B.numstat.tsv) | 36 files, +3531/-104 | Search with the hits marked, the outline, text selection and copy, links (an external one after a confirmation), on Calcita's own continuous page view, because QtPdf's `PdfMultiPageView` link delegate opens a URL at once and paints hits and selection in non-token colours (fix round 1: computed page offsets and a near range, hits that only move the view on a new search or a step, a drag that always selects) | [evidence](../../evidence/2026-10-09-search-outline-links.md) | VAL-CAL-SEARCH |
| CAL-1-C | `calcita:` | done | [inventory](../../inventories/2026-10-09-cal-1-foundation/CAL-1-C.numstat.tsv) | 29 files, +960/-57 | The dark reading mode (a layer effect with a local fragment shader baked by `build.rs`, Ctrl+I and a bar button, remembered per document in `Reading`), the recents card's «Quitar de recientes» menu, the `application/pdf` entry, the accessibility pass; 1.0.0 at the landing (`--kind release`), plan archived after it | [evidence](../../evidence/2026-10-09-exit.md) | VAL-CAL-DARK |
| CAL-1-D | `calcita:` | done | [inventory](../../inventories/2026-10-09-cal-1-foundation/CAL-1-D.numstat.tsv) | 10 files, +260/-5 | Bug: QtPdf renders a page with a clear background, so the ink was drawn straight on the window's glass; every page now lies on an opaque paper rectangle under its image (`PageView.paper`, the suite's `iconSheet`; square, no shadow, as content is), under the hits, the selection and the links, and inside the reading mode's layer so it reads dark there while the gaps stay glass | [evidence](../../evidence/2026-10-10-page-contrast.md) | VAL-CAL-OPEN |
