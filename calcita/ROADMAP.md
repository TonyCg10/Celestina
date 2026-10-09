# Calcita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** CAL-1
- **Related author validation:** none yet; the CAL-1 units add their entries
  to [VALIDATION.md](VALIDATION.md) (they do not block)

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

## Exclusions

- Annotation, signing, forms, printing, OCR, and any shell integration.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| CAL-1-A | planned | EXT-1-A | open, pages, zoom, navigation, drop and activation | `scripts/verify-production.sh` |
| CAL-1-B | planned | CAL-1-A | search, outline, selection, links | `scripts/verify-production.sh` |
| CAL-1-C | planned | CAL-1-B | reading mode, recents, handler entry, 1.0.0 | `scripts/complete-production.sh` |

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary opens and
reads a PDF with every tool of the scope.

## Closed evidence

- [The skeleton](docs/evidence/2026-10-09-skeleton.md) (EXT-1-A).
