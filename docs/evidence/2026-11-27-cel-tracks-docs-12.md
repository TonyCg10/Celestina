# Evidence: drop D1 and the menu placement rule (CEL-TRACKS-DOCS-12)

- **Date:** 2026-11-27
- **Scope:** `CEL-TRACKS-DOCS-12` covers:
  - the design `docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md` (§4, §5.1 to §5.3, the new §5.8, §6 and §9);
  - the amendment appended to `docs/superpowers/plans/2026-11-27-celestina-cel-d-013-plan.md`;
  - a note at the head of `docs/superpowers/plans/2026-11-27-celestina-cel-d-016-plan.md`;
  - the new plan `docs/superpowers/plans/2026-11-27-celestina-cel-d-017-plan.md`;
  - the TRK-1 row and its authorization line.
- **Why:** at the nested check of `CEL-D-013` the author turned down drop G1 and chose drop D1; `CEL-D-013` landed with D1 (`be89dd0a`), so the design and the plans still said G1. After trying the menus, the author asked for each menu to open closer to its icon and chose rule B of the menu placement preview on 2026-11-27.
- **Environment:** worktree `suite-CEL-TRACKS-DOCS-12`
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
- **Review:** 1 round. The controller checked the design's D1 numbers against `celestina/src/sheet/drop.rs` on `main` and the `CEL-D-017` formulas against the plan's test values.

## Limits

- Documents only: no code, build or nested run for this unit.

## Follow-up

None.

## Landing

- **Base revision:** `be89dd0ac2798b600a6c3776d05fc20ece780463`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
