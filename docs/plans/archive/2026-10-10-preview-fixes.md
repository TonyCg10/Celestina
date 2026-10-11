# PRV-2 — Capture preview fixes

- **Opened:** 2026-10-10
- **Plan ID:** preview-fixes
- **Closed:** 2026-10-10
- **Successor:** none; the author named no next suite checkpoint. Its one ledger row, PRV-2-A, is done; SEL-3-A and FLU-P2-A are done in their own plans
- **Status:** done
- **Authorization:** the author reported five problems after the first live
  use of PRV-1 on 2026-10-10 and approved the fixes the same day
- **Scope:** suite
- **Implementation checkpoint:** PRV-2
- **Author-validation checkpoint:** none at suite level; each application's
  `VALIDATION.md` holds its own entries

## Hypothesis

PRV-1's preview and editor work, but the first live use showed five defects
that each block daily use: the preview opened as a tiled column, its picture
was blurred, Fluorita's toolbar was cut off, the `S` key captured every
monitor at once, and the recordings folder carried an English name.

## Tangible outcome

The preview floats in the bottom-right corner, sharp; Fluorita's editor
opens wide enough for its whole toolbar; `S` captures the focused monitor;
recordings go to the videos folder's `Grabaciones`; the adoption folder rule has one
owner in `selenita-core`.

## Scope

- `PRV-2-A` — the suite documents: the niri rule's title pattern (`^Vista
  previa`, without `$`, because Qt on Wayland appends the application's
  display name to the title) in the spec and the host-hygiene record, the
  folder name `Grabaciones` in ADR 0012's PRV-1 follow-up and the spec, and
  this checkpoint.

The application units are rows of each project's own plan: `SEL-3-A` (the
preview's sharpness, the focused monitor for `S`, `Grabaciones`, the single
adoption rule) in `selenita/docs/plans/active/2026-10-10-sel-3-preview-fixes.md`,
and `FLU-P2-A` (the edit window's size, the adoption rule taken from
`selenita-core`) in `fluorita/docs/plans/active/2026-10-10-flu-p2-preview-fixes.md`.

## Exclusions

- Anything beyond the five reported defects and the adoption rule's single
  owner.
- The halted Celestina shell.

## Build order

1. `PRV-2-A`, `SEL-3-A` and `FLU-P2-A` in any order; they share no file.
2. After the landings, the author archives this plan and the two
   application plans by hand.

## Implementation exit

The three units are landed and the author's configuration carries the
corrected rule (done on 2026-10-10).

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| PRV-2-A | `suite:` | done | [inventory](../../inventories/2026-10-10-preview-fixes/PRV-2-A.numstat.tsv) | 9 files, +186/-13 | Fix the preview rule and the recordings folder name in the suite documents and open PRV-2 | [evidence](../../evidence/2026-10-10-preview-fixes.md) | None |
| PRV-2-B | `suite:` | done | [inventory](../../inventories/2026-10-10-preview-fixes/PRV-2-B.numstat.tsv) | 8 files, +164/-84 | Archive the delivered preview fixes plan through its own administrative unit and reconcile the suite roadmap, status and plan indexes | [evidence](../../evidence/2026-10-10-preview-fixes-archive.md) | None |

This plan records intent; it grants no authority.
