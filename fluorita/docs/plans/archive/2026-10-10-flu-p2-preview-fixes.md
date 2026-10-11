# FLU-P2 — Fixes to the floating editor after its first live use

- **Opened:** 2026-10-10
- **Plan ID:** flu-p2-preview-fixes
- **Closed:** 2026-10-10
- **Successor:** none; the author named no next Fluorita checkpoint. Its one delivery row (FLU-P2-A) is done
- **Status:** done
- **Authorization:** the author used the floating editor of the capture
  preview live on 2026-10-10 and approved two fixes: the edit window opens
  wide enough for its whole toolbar and never lets it be cut, and Fluorita
  takes the adoption rule from Selenita instead of keeping its own copy. The
  design is [the capture preview spec](../../../../docs/superpowers/specs/2026-10-10-capture-preview-design.md)
  (§5 is Fluorita's); the delivered editor is
  [FLU-P1](../archive/2026-10-10-flu-p1-preview.md)
- **Scope:** fluorita
- **Implementation checkpoint:** FLU-P2
- **Author-validation checkpoint:** `VAL-FLU-EDIT-WINDOW` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The edit window opened 960 px wide, narrower than the toolbar it carries,
so the toolbar's ends were cut off on the author's screen. Opening at the
toolbar's own width (never less than 1280 × 800, never more than nine tenths
of the screen) and refusing a narrower window keeps every tool in reach; on
a screen too narrow for the whole toolbar, the toolbar scrolls inside its
pill rather than clip. Separately, `fluorita/src/adopt.rs` kept its own copy
of Selenita's adoption rule; taking `selenita_core::history::adopted` and the
folder names of `selenita_core::names` leaves one owner, so a rename of
Selenita's recordings folder reaches Fluorita without a change here.

## Tangible outcome

The editor opened from Selenita's preview shows its whole bottom toolbar,
and the trim's controls under a film likewise; a window cannot be made
narrower than either. A copy saved with «Guardar ambas» joins Selenita's
history by Selenita's own rule whether Selenita runs or not.

## Scope

- `FLU-P2-A` — the edit window's size against its bar (`EditWindow.qml`,
  `EditToolbar.qml`, `EditSurface.qml`, `TrimSurface.qml` and their QML
  tests), and the adoption rule taken from `selenita-core`
  (`fluorita/src/adopt.rs` and its tests).

## Exclusions

- `selenita-core` and `selenita/`: the rule and the folder names are read,
  never changed; the recordings folder's rename is Selenita's own unit.
- The niri window rule, and anything else the capture preview spec leaves
  out of scope.

## Build order

1. `FLU-P2-A`.

## Implementation exit

The row's `Automated evidence` names its exit; the unit ends with
Fluorita's `scripts/complete-production.sh` at landing.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| FLU-P2-A | `fluorita:` | done | [inventory](../../inventories/2026-10-10-flu-p2-preview-fixes/FLU-P2-A.numstat.tsv) | 18 files, +1117/-390 | Fix the edit window's size and take the adoption rule from Selenita: the window opens at its toolbar's whole width (at least 1280 × 800, at most nine tenths of the screen) with that width as its minimum, the toolbar scrolls rather than clip on a narrower screen, the trim's controls follow the same rule, and `adopt.rs` applies `selenita_core::history::adopted` over the folders `selenita_core::names` names | [evidence](../../evidence/2026-10-10-editor-fixes.md) | `VAL-FLU-EDIT-WINDOW` |
| FLU-P2-B | `fluorita:` | done | [inventory](../../inventories/2026-10-10-flu-p2-preview-fixes/FLU-P2-B.numstat.tsv) | 7 files, +158/-87 | Archive the delivered FLU-P2 preview fixes plan through its own administrative unit and reconcile the Fluorita roadmap, status and plan index | [evidence](../../evidence/2026-10-10-flu-p2-preview-fixes-archive.md) | None |

This plan records intent; it grants no authority.
