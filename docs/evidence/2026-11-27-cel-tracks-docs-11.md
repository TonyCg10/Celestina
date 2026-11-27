# Evidence: the menu drop design and plans (CEL-TRACKS-DOCS-11)

- **Date:** 2026-11-27
- **Scope:** `CEL-TRACKS-DOCS-11` covers:
  - the design `docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md`;
  - the plans `docs/superpowers/plans/2026-11-27-celestina-cel-d-013-plan.md`, `-cel-d-014-plan.md`, `-cel-d-015-plan.md` and `-cel-d-016-plan.md`;
  - two pointer lines in `docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md` (§4.2 and §4.3);
  - the TRK-1 row and its authorization line.
- **Why:** after recording celestina 1.35.0, the author asked for app menus that come out of their app, an instant switch between menus, a close that does not linger over the pill, the open piece back in its slot, and menus that open and close as a drop. On 2026-11-27 the author approved version 3 of the menu drop preview (drop G1, the grey card, the open piece at 80 %, the travelling switch).
- **Environment:** worktree `suite-CEL-TRACKS-DOCS-11`
- **Artifact:** not applicable (no production input changes)

## Procedure

```sh
git diff --numstat origin/main... -- . ':!*.md'
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** every command exits 0, and no line changes outside documents.
- **Review:** 1 round. A builder check read the design against the code; its five questions became calls recorded in the design's §6. The controller read the four plans against the design and against each other and aligned their shared names (`Sheets::held_piece`, `PIECE_RESTORE_AT`, `Tree::paint_hiding`, the scroll thumb).

## Limits

- Documents only: no code, build or nested run for this unit. The "today" facts come from the code, read-only surveys of it, and the author's recording of 1.35.0 (1920 × 1080, 60 fps).
- How niri blurs a thin neck is not known from HTML; the first task of `CEL-D-013` is a nested check the author approves before the rest of that unit.

## Follow-up

None.

## Landing

- **Base revision:** `6ff5a0188fd1ed6a96a21507bea8521025a235f4`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
