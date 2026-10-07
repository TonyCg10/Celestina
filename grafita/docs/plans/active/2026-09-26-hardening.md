# GRA-H1 — Hardening after the monorepo audit

- **Opened:** 2026-09-26
- **Plan ID:** hardening
- **Status:** active
- **Authorization:** after the 2026-09-26 monorepo audit the author asked
  for its whole program to be done; the findings and rulings are in
  [the audit evidence](../../../../docs/evidence/2026-09-26-monorepo-audit.md)
  and the Grafita findings in
  [the Hematita and Grafita record](../../../../docs/evidence/2026-09-26-monorepo-audit-hematita-grafita.md)
- **Scope:** grafita
- **Implementation checkpoint:** GRA-H1
- **Author-validation checkpoint:** none

## Hypothesis

Bounding nesting and decompressed size, and replacing unchecked slices with
typed errors, makes every hostile document the importer receives a refusal
instead of a process abort, in Grafita and in Siderita's embedded preview
alike.

## Tangible outcome

A 20 KB nested PDF, a 1 GiB gzip bomb, a lying ZIP header and a huge `/N` are
each refused with a typed error by `grafita-core`, under tests that also pass
as root; the highlighter no longer does quadratic work on a long line; and
the recent list and preferences stop touching the disk on the GUI thread.

## Scope

- `GRA-H1-A` (P-5) — harden the document importer against hostile input.
- `GRA-H1-B` (P-19) — UTF-16 highlight runs, recent and preference IO off
  the GUI thread, a guarded highlighter target, the shared `file_uri` owner,
  and STATUS truth.

## Exclusions

- GRA-6 (a whole-document round trip per keystroke), a later milestone.
- Production builds and deployment, which run at the author's landing.

## Build order

1. `GRA-H1-A` from `main`.
2. `GRA-H1-B`, stacked on `GRA-H1-A`, after `RS-H1-A` (P-6) landed.

## Implementation exit

Each row's `Automated evidence` names its exit; every unit ends with
Grafita's and Siderita's `scripts/complete-production.sh` at landing, because
Siderita links `grafita-core`. Under ruling R-A2 only Grafita's version
moves.

## Change and commit ledger

Paths are repository-relative. Each row's `Intended change` ends with the
program id and the audit findings it closes.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| GRA-H1-A | `grafita:` | done | [inventory](../../inventories/2026-09-26-hardening/GRA-H1-A.numstat.tsv) | 24 files, +3928/-340 | Bound PDF nesting. Replace unchecked slices with typed errors and use `checked_add`. Cap every decoder with `take(limit+1)` and a total across members and filters. Stop trusting `uncompressed_size`/`/N`. Memoise object streams. Add a negative-input table. (P-5: GRA-1, GRA-2, GRA-3, GRA-7) | [evidence](../../evidence/2026-09-26-importer-hardening.md) | None |
| GRA-H1-B | `grafita:` | done | [inventory](../../inventories/2026-09-26-hardening/GRA-H1-B.numstat.tsv) | 24 files, +2066/-223 | Return UTF-16 runs from Rust in one pass and coalesce the palette rehighlight. Recent-list and `existing()` as worker jobs, and debounced preference writes. `QPointer` target. Adopt `file_uri` (non-UTF-8 byte-exact). STATUS. (P-19: GRA-4, GRA-5, GRA-8, GRA-9; RS-1 (grafita)) | [evidence](../../evidence/2026-09-26-app-responsiveness.md) | None |
| GRA-H1-C | `grafita:` | done | [inventory](../../inventories/2026-09-26-hardening/GRA-H1-C.numstat.tsv) | 11 files, +100/-11 | Give the window Siderita's glass canvas: transparent window with `CelestinaBackdrop` (STYLE-G7-Q recipe), the tab strip and find bar in the Haze `pillFill` with no outline, and the document page an opaque `card` box; lowers the glass-canvas ratchet row to 0. Author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-glass-canvas.md) | None |
| GRA-H1-D | `grafita:` | done | [inventory](../../inventories/2026-09-26-hardening/GRA-H1-D.numstat.tsv) | 10 files, +75/-2 | Attach the shared wheel scroller to the document view and the encoding list so the wheel scrolls without Qt's decelerating flick; Ctrl+wheel zoom is untouched. Author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-wheel.md) | None |

This plan records intent; it grants no authority.
