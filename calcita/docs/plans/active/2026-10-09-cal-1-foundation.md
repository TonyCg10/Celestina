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
[the suite plan](../../../../docs/plans/active/2026-10-09-reading-and-capture.md).
It is the transparent window over the backdrop, the appearance follower, the
claim-first activation adapter, the `CalcitaController` singleton, the empty
state «Sin documento» with «Abrir…», the scripts, the QML test harness and
the document set; its record is
[the skeleton evidence](../../evidence/2026-10-09-skeleton.md).

- `CAL-1-A` — open a document: pages, zoom, go to, the drop and `Open`.
- `CAL-1-B` — search, the outline, text selection and links.
- `CAL-1-C` — reading mode, recents, the `application/pdf` entry, 1.0.0.

## Exclusions

- Everything the design's §1 lists as out of scope for Calcita.
- Any reference to the Celestina shell.

## Build order

1. `CAL-1-A`, then `CAL-1-B`, then `CAL-1-C`.

## Implementation exit

`scripts/complete-production.sh` succeeds at 1.0.0 and the installed binary
reads a PDF with every tool of the scope.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CAL-1-A | `calcita:` | planned | `calcita/`, `celestina-rs/crates/calcita-core/` | — | Open a document with its pages, zoom and navigation; the drop and `Open` reach it | `scripts/verify-production.sh` | VAL-CAL-OPEN |
| CAL-1-B | `calcita:` | planned | `calcita/src/`, `calcita/qml/` | — | Search with the hits marked, the outline, text selection and copy, links | `scripts/verify-production.sh` | None yet |
| CAL-1-C | `calcita:` | planned | `calcita/`, `docs/version-history.tsv` | — | Reading mode, recents, the `application/pdf` entry, 1.0.0, plan archived | `scripts/complete-production.sh` | None yet |
