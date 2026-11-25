# Evidence: DEV-1-CLOSE — archiving the development speed plan

- **Date:** 2026-11-25
- **Scope:** light unit; `DEV-1-CLOSE`: the DEV-1 plan moved to `docs/plans/archive/` with `Closed`, `Successor` and `Status`, its scope, build order and ledger rows; the root roadmap idle with no active checkpoint; the suite status; the active-plans index; two links retargeted to the archive (ADR 0013, the archived RSH-1 plan)
- **Environment:** worktree `suite-DEV-1-CLOSE`, branch `unit/suite/DEV-1-CLOSE` on `629d434b`
- **Artifact:** not applicable

## Procedure

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-staged-units.py --root . docs/inventories/2026-11-24-dev-speed/DEV-1-CLOSE.numstat.tsv
```

## Result

- **Exit:** each command exits 0
- **Exit condition of the plan:** every row is `done`; the `now` budget test passes in the documentation guard; after the landings of 2026-11-25 the session target was 0 B against the 40 GiB cap (`worktree.sh close` printed it)
- **Review:** 1 round (Sonnet): nothing important; mentions of the plan's old path in backticks in older evidence and task plans stay, as records of where it lived

## Limits

- Documents only, plus this unit's inventory. Sealed by hand with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and `sealed_diffstat`, as `DEV-1-OPEN` was: `land-unit.py` refuses a unit that moves its own plan. The commit must reach `main` by fast-forward with parent `629d434b`.

## Follow-up

None.
