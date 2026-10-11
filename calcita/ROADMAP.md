# Calcita implementation roadmap

- **Status:** idle
- **Active implementation checkpoint:** none
- **Related author validation:** VAL-CAL-OPEN, VAL-CAL-SEARCH and
  VAL-CAL-DARK pending in [VALIDATION.md](VALIDATION.md) (they do not block)

## Hypothesis and tangible outcome

A PDF viewer built on QtPdf and the suite's conventions can take reading out
of the web browser: one window per document, opened from Siderita, a drop or
a second launch, with the reading tools a person reaches for every day.

## Scope

| Unit | Outcome |
|---|---|
| EXT-1-A (`suite`) | Registration, with the skeleton, the icon and the document set |
| CAL-1-A | Open a document: pages, zoom, go to, the suite joins (drop, `Open`) |
| CAL-1-B | Search, the outline, text selection and links |
| CAL-1-C | Reading mode, recents, the `application/pdf` entry and 1.0.0 |
| CAL-1-D | Bug: every page on opaque paper over the glass canvas, 1.0.1 |

## Exclusions

- Annotation, signing, forms, printing, OCR, and any shell integration.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| CAL-1-A | done | EXT-1-A | open, pages, zoom, navigation, drop and activation | [evidence](docs/evidence/2026-10-09-open-and-pages.md) |
| CAL-1-B | done | CAL-1-A | search, outline, selection and copy, links | [evidence](docs/evidence/2026-10-09-search-outline-links.md) |
| CAL-1-C | done | CAL-1-B | reading mode, recents, handler entry, 1.0.0 | [evidence](docs/evidence/2026-10-09-exit.md) |
| CAL-1-D | done | CAL-1-C | the pages on opaque paper, 1.0.1 | [evidence](docs/evidence/2026-10-10-page-contrast.md) |

CAL-1 closed on 2026-10-11 at 1.0.1, after the bug unit CAL-1-D. Its build
order, exclusions, exit and ledger are in the
[archived plan](docs/plans/archive/2026-10-09-cal-1-foundation.md).

## Later

- The bar's menu of the design's §4.3 (the recents and «copy path») is
  deferred: CAL-1 ships the recents card on the empty window only.

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary opens and
reads a PDF with every tool of the scope.

## Closed evidence

- [The skeleton](docs/evidence/2026-10-09-skeleton.md) (EXT-1-A).
