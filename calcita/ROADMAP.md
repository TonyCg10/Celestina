# Calcita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** CAL-2
- **Related author validation:** VAL-CAL-OPEN, VAL-CAL-SEARCH,
  VAL-CAL-DARK, VAL-CAL-ANNOT and VAL-CAL-EDIT pending in
  [VALIDATION.md](VALIDATION.md) (they do not block)

## Hypothesis and tangible outcome

A PDF viewer built on QtPdf and the suite's conventions can take reading out
of the web browser: one window per document, opened from Siderita, a drop or
a second launch, with the reading tools a person reaches for every day.

CAL-2 adds light editing without rewriting a document: text boxes,
highlights and freehand drawings as standard PDF annotations and the forms a
PDF already carries, saved as incremental updates. Its ledger is the
[active plan](docs/plans/active/2026-10-11-cal-2-editing.md), from
[the spec](docs/specs/2026-10-11-editing-design.md) and
[the plan](docs/specs/2026-10-11-editing-plan.md).

## Scope

| Unit | Outcome |
|---|---|
| EXT-1-A (`suite`) | Registration, with the skeleton, the icon and the document set |
| CAL-1-A | Open a document: pages, zoom, go to, the suite joins (drop, `Open`) |
| CAL-1-B | Search, the outline, text selection and links |
| CAL-1-C | Reading mode, recents, the `application/pdf` entry and 1.0.0 |
| CAL-1-D | Bug: every page on opaque paper over the glass canvas, 1.0.1 |
| CAL-2-A | Every annotation shown; the annotation model and the coordinates |
| CAL-2-B | The edit mode, the three tools, saving with the two outcomes |
| CAL-2-C | Forms: shown, text fields and checkboxes filled and saved |

## Exclusions

- Signing, printing, OCR, and any shell integration.
- Within editing, everything the CAL-2 spec's §1 lists as out of scope
  (the document's own text and images, pages, certificate signatures,
  embedded fonts, rich text, replies, a flattened export, encrypted
  documents).

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| CAL-1-A | done | EXT-1-A | open, pages, zoom, navigation, drop and activation | [evidence](docs/evidence/2026-10-09-open-and-pages.md) |
| CAL-1-B | done | CAL-1-A | search, outline, selection and copy, links | [evidence](docs/evidence/2026-10-09-search-outline-links.md) |
| CAL-1-C | done | CAL-1-B | reading mode, recents, handler entry, 1.0.0 | [evidence](docs/evidence/2026-10-09-exit.md) |
| CAL-1-D | done | CAL-1-C | the pages on opaque paper, 1.0.1 | [evidence](docs/evidence/2026-10-10-page-contrast.md) |
| CAL-2-A | done | CAL-1-D | every annotation shown; the model and the coordinates | [evidence](docs/evidence/2026-10-11-annotations-shown.md) |
| CAL-2-B | active | CAL-2-A | edit mode, the three tools, saving | [evidence](docs/evidence/2026-10-11-editing.md) |
| CAL-2-C | planned | CAL-2-B | forms filled and saved | none yet |

CAL-1 closed on 2026-10-11 at 1.0.1, after the bug unit CAL-1-D. Its build
order, exclusions, exit and ledger are in the
[archived plan](docs/plans/archive/2026-10-09-cal-1-foundation.md).

## Later

- The bar's menu of the design's §4.3 (the recents and «copy path») is
  deferred: CAL-1 ships the recents card on the empty window only.

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary opens and
reads a PDF with every tool of the scope; for CAL-2, it also shows another
program's annotations, adds and edits the three kinds, fills a form and
saves both outcomes.

## Closed evidence

- [The skeleton](docs/evidence/2026-10-09-skeleton.md) (EXT-1-A).
